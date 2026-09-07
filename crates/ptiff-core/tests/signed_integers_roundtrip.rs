//! P0-04 (WP-P0-04): signed integer (Int16/Int32) support.
//!
//! Proves the full typed chain for two's-complement signed samples:
//!
//! ```text
//! Rust signed value -> typed PixelType -> TIFF (BitsPerSample + SampleFormat 2)
//!   -> TIFF decoder -> typed PixelType -> same signed value
//! ```
//!
//! No conversion to unsigned, no reinterpretation, no silent overflow:
//! negative values round-trip as negative values with the exact bit pattern
//! TIFF stores for two's-complement samples. Also covers byte order
//! (little-endian through the typed path, big-endian through the backend
//! path), strips and tiles, all lossless typed codecs, determinism, explicit
//! rejection of Int8/Int64 and of JPEG with signed samples, and regression
//! checks that unsigned/float pixel types are unchanged.

use ptiff_core::image::ImageDescriptorBuilder;
use ptiff_core::io::backend::tiff::TiffBackend;
use ptiff_core::io::{
    Deserializer, MemoryBinaryReader, MemoryBinaryWriter, SceneDeserializer, SceneSerializer,
    Serializer, StorageBackend, StorageModel,
};
use ptiff_core::tile::{Tile, TileExtent, TileIndex, TileRegion};
use ptiff_core::{CompressionKind, ErrorCode, PixelType, Scene, TileId};

const LOSSLESS_CODECS: [CompressionKind; 4] = [
    CompressionKind::None,
    CompressionKind::Lzw,
    CompressionKind::PackBits,
    CompressionKind::Deflate,
];

/// Signed boundary values required by the increment brief.
const INT16_VALUES: [i16; 9] = [i16::MIN, -32767, -1001, -1, 0, 1, 1001, 32766, i16::MAX];
const INT32_VALUES: [i32; 9] = [
    i32::MIN,
    -2147483647,
    -1_000_003,
    -1,
    0,
    1,
    1_000_003,
    2147483646,
    i32::MAX,
];

/// Deterministic f64 sample set for float/unsigned regression checks.
fn f64_sample(row: u32, col: u32, width: u32) -> f64 {
    const SAMPLES: [f64; 8] = [
        0.0,
        1.0,
        -1.5,
        1.0 / 3.0,
        -2.75,
        1e-300,
        0.1 + 0.2,
        -1.0e200,
    ];
    let idx = (u64::from(row) * u64::from(width) + u64::from(col)) % (SAMPLES.len() as u64);
    SAMPLES[idx as usize]
}

/// Expected Int16 sample at (row, col): cycles INT16_VALUES.
fn expected_i16(row: u32, col: u32, width: u32) -> i16 {
    let idx = (u64::from(row) * u64::from(width) + u64::from(col)) % (INT16_VALUES.len() as u64);
    INT16_VALUES[idx as usize]
}

fn expected_i32(row: u32, col: u32, width: u32) -> i32 {
    let idx = (u64::from(row) * u64::from(width) + u64::from(col)) % (INT32_VALUES.len() as u64);
    INT32_VALUES[idx as usize]
}

/// Encodes one sample to little-endian bytes for `dtype`.
fn sample_bytes(dtype: PixelType, row: u32, col: u32, width: u32) -> Vec<u8> {
    match dtype {
        PixelType::Int16 => expected_i16(row, col, width).to_le_bytes().to_vec(),
        PixelType::Int32 => expected_i32(row, col, width).to_le_bytes().to_vec(),
        PixelType::UInt16 => {
            let v = (u64::from(row) * u64::from(width) + u64::from(col)) % 65536;
            (v as u16).to_le_bytes().to_vec()
        }
        PixelType::UInt32 => {
            let v = (u64::from(row) * u64::from(width) + u64::from(col)) % 65536;
            (v as u32).to_le_bytes().to_vec()
        }
        PixelType::Float32 => (f64_sample(row, col, width) as f32).to_le_bytes().to_vec(),
        PixelType::Float64 => f64_sample(row, col, width).to_le_bytes().to_vec(),
        PixelType::UInt8 => unreachable!("not used in this suite"),
        _ => unreachable!("dtype not handled in this suite"),
    }
}

/// Serializes the TIFF header/IFD for one typed Scene image and streams the
/// deterministic pixel payload through the Sink.
fn typed_scene_write(scene: &Scene, width: u32, height: u32, dtype: PixelType) -> Vec<u8> {
    let root: StorageModel = SceneSerializer.serialize(scene).expect("serialize scene");
    let model = root
        .children()
        .first()
        .expect("scene has exactly one image")
        .clone();

    let mut writer = MemoryBinaryWriter::new();
    TiffBackend
        .serialize_model(&model, &mut writer)
        .expect("serialize TIFF");

    {
        let mut sink = TiffBackend
            .open_image_sink(&mut writer, &model)
            .expect("open sink");
        let layout = *sink.layout();
        let cols = layout.columns(0);
        let rows = layout.rows(0);
        let tw = layout.tile_size.width;
        let th = layout.tile_size.height;
        let bytes_per_sample = dtype.bytes_per_sample();

        for row in 0..rows {
            for col in 0..cols {
                let mut tile_bytes = Vec::new();
                for ty in 0..th {
                    let global_row = row * th + ty;
                    if global_row >= height {
                        break;
                    }
                    for tx in 0..tw {
                        let global_col = col * tw + tx;
                        if global_col >= width {
                            break;
                        }
                        tile_bytes
                            .extend_from_slice(&sample_bytes(dtype, global_row, global_col, width));
                    }
                }
                assert_eq!(tile_bytes.len(), (tw * th) as usize * bytes_per_sample);
                let tile = Tile::new(
                    TileId::new(u64::from(row) * u64::from(cols) + u64::from(col)),
                    TileIndex::new(col, row, 0),
                    TileRegion::new(0, 0, TileExtent::new(tw, th)),
                    &tile_bytes,
                );
                sink.write_tile(&tile).unwrap();
            }
        }
    }
    writer.take_buffer()
}

/// Reads the file back through the image source and asserts every decoded
/// sample equals the deterministic expected semantic value.
fn assert_samples_exact(bytes: &[u8], width: u32, height: u32, dtype: PixelType) {
    let mut reader = MemoryBinaryReader::from_slice(bytes);
    let mut source = TiffBackend
        .open_image_source(&mut reader)
        .expect("open source");
    let layout = *source.layout();
    let cols = layout.columns(0);
    let rows = layout.rows(0);
    let tw = layout.tile_size.width;
    let th = layout.tile_size.height;
    let bps = dtype.bytes_per_sample();

    for row in 0..rows {
        for col in 0..cols {
            let t = source
                .read_tile(TileIndex::new(col, row, 0))
                .expect("read tile");
            for ty in 0..th {
                let global_row = row * th + ty;
                if global_row >= height {
                    break;
                }
                for tx in 0..tw {
                    let global_col = col * tw + tx;
                    if global_col >= width {
                        break;
                    }
                    let idx = (ty * tw + tx) as usize;
                    let raw = &t.data()[idx * bps..(idx + 1) * bps];
                    match dtype {
                        PixelType::Int16 => {
                            let expected = expected_i16(global_row, global_col, width);
                            let decoded = i16::from_le_bytes(raw.try_into().unwrap());
                            // Semantic equality proves no unsigned reinterpretation;
                            // bit equality is implied by from_le_bytes of the exact
                            // two's-complement LE bytes TIFF stored.
                            assert_eq!(
                                decoded, expected,
                                "Int16 differs at ({global_row},{global_col}): decoded {decoded}, expected {expected}"
                            );
                            assert_eq!(decoded.to_le_bytes(), expected.to_le_bytes());
                        }
                        PixelType::Int32 => {
                            let expected = expected_i32(global_row, global_col, width);
                            let decoded = i32::from_le_bytes(raw.try_into().unwrap());
                            assert_eq!(
                                decoded, expected,
                                "Int32 differs at ({global_row},{global_col}): decoded {decoded}, expected {expected}"
                            );
                            assert_eq!(decoded.to_le_bytes(), expected.to_le_bytes());
                        }
                        PixelType::UInt16 => {
                            let v = (u64::from(global_row) * u64::from(width)
                                + u64::from(global_col))
                                % 65536;
                            let expected = v as u16;
                            let decoded = u16::from_le_bytes(raw.try_into().unwrap());
                            assert_eq!(
                                decoded, expected,
                                "UInt16 differs at ({global_row},{global_col})"
                            );
                        }
                        PixelType::UInt32 => {
                            let v = (u64::from(global_row) * u64::from(width)
                                + u64::from(global_col))
                                % 65536;
                            let expected = v as u32;
                            let decoded = u32::from_le_bytes(raw.try_into().unwrap());
                            assert_eq!(
                                decoded, expected,
                                "UInt32 differs at ({global_row},{global_col})"
                            );
                        }
                        PixelType::Float32 => {
                            let expected = f64_sample(global_row, global_col, width) as f32;
                            let decoded = f32::from_le_bytes(raw.try_into().unwrap());
                            assert_eq!(
                                decoded.to_bits(),
                                expected.to_bits(),
                                "Float32 differs at ({global_row},{global_col})"
                            );
                        }
                        PixelType::Float64 => {
                            let expected = f64_sample(global_row, global_col, width);
                            let decoded = f64::from_le_bytes(raw.try_into().unwrap());
                            assert_eq!(
                                decoded.to_bits(),
                                expected.to_bits(),
                                "Float64 differs at ({global_row},{global_col})"
                            );
                        }
                        PixelType::UInt8 => unreachable!("not used in this suite"),
                        _ => unreachable!("dtype not handled in this suite"),
                    }
                }
            }
        }
    }
}

/// Full typed round-trip: Scene -> SceneSerializer -> TIFF bytes (with pixel
/// payload) -> SceneDeserializer -> Scene; returns (bytes, back-Scene).
fn typed_roundtrip(
    dtype: PixelType,
    codec: CompressionKind,
    width: u32,
    height: u32,
    tile: Option<(u32, u32)>,
) -> (Vec<u8>, Scene) {
    let mut builder = ImageDescriptorBuilder::new(width, height)
        .pixel_type(dtype)
        .channel_count(1)
        .compression(Some(codec));
    if let Some((tw, th)) = tile {
        builder = builder.tile(tw, th);
    }
    let mut scene = Scene::new();
    scene.add_image(builder.build()).expect("add image");

    let bytes = typed_scene_write(&scene, width, height, dtype);

    let mut reader = MemoryBinaryReader::from_slice(&bytes);
    let back_model = TiffBackend
        .deserialize_model(&mut reader)
        .expect("deserialize TIFF");
    let back = SceneDeserializer
        .deserialize(&back_model)
        .expect("deserialize Scene");
    (bytes, back)
}

/// Int16 through every lossless codec, strip and tiled. The typed Scene read
/// must report Int16 (never UInt16/Float32), and each sample must decode to
/// the exact signed value including i16::MIN.
#[test]
fn int16_round_trips_typed_scene_all_lossless_codecs_strips_and_tiles() {
    for codec in LOSSLESS_CODECS {
        for (tile, width, height) in [(None, 40u32, 24u32), (Some((16, 16)), 48u32, 32u32)] {
            let (bytes, back) = typed_roundtrip(PixelType::Int16, codec, width, height, tile);
            let img = back.image_at(0).unwrap();
            assert_eq!(
                img.pixel_type(),
                PixelType::Int16,
                "codec {codec:?} tile {tile:?}: signed type must survive as Int16"
            );
            assert_eq!(img.compression(), Some(codec));
            assert_eq!(img.width(), width);
            assert_eq!(img.height(), height);
            assert_eq!(img.channel_count(), 1);
            assert_samples_exact(&bytes, width, height, PixelType::Int16);
        }
    }
}

/// Int32 through every lossless codec, strip and tiled, with i32::MIN
/// round-tripping exactly (proves no sign/zero-extension corruption).
#[test]
fn int32_round_trips_typed_scene_all_lossless_codecs_strips_and_tiles() {
    for codec in LOSSLESS_CODECS {
        for (tile, width, height) in [(None, 40u32, 24u32), (Some((16, 16)), 48u32, 32u32)] {
            let (bytes, back) = typed_roundtrip(PixelType::Int32, codec, width, height, tile);
            let img = back.image_at(0).unwrap();
            assert_eq!(img.pixel_type(), PixelType::Int32);
            assert_eq!(img.compression(), Some(codec));
            assert_samples_exact(&bytes, width, height, PixelType::Int32);
        }
    }
}

/// Big-endian backend path: TIFF byte order must not flip signedness. The
/// typed schema cannot carry a byteOrder (deferred serialization decision), so
/// the big-endian form is exercised at the storage layer, exactly as TIFF
/// files are produced/consumed today.
#[test]
fn big_endian_signed_samples_decode_to_same_values() {
    for dtype in [PixelType::Int16, PixelType::Int32] {
        let mut model = StorageModel::new();
        model.set_field("imageWidth", "4");
        model.set_field("imageHeight", "2");
        model.set_field("samplesPerPixel", "1");
        model.set_field(
            "pixelType",
            match dtype {
                PixelType::Int16 => "Int16",
                PixelType::Int32 => "Int32",
                _ => unreachable!(),
            },
        );
        model.set_field("compression", "None");
        model.set_field("byteOrder", "Big");

        let mut writer = MemoryBinaryWriter::new();
        TiffBackend
            .serialize_model(&model, &mut writer)
            .expect("serialize TIFF");
        {
            let mut sink = TiffBackend.open_image_sink(&mut writer, &model).unwrap();
            let mut payload = Vec::new();
            for v in match dtype {
                PixelType::Int16 => vec![i16::MIN, -1, 0, 1, 32767, -32767]
                    .into_iter()
                    .map(|x: i16| x.to_be_bytes().to_vec())
                    .collect::<Vec<_>>(),
                PixelType::Int32 => vec![i32::MIN, -1, 0, 1, 2147483647, -2147483647]
                    .into_iter()
                    .map(|x: i32| x.to_be_bytes().to_vec())
                    .collect::<Vec<_>>(),
                _ => unreachable!(),
            } {
                payload.extend_from_slice(&v);
            }
            // Pad the remaining rows (the strip covers width*height samples;
            // the fixture lists only the meaningful sample values).
            let region_bytes = 4 * 2 * dtype.bytes_per_sample();
            assert!(payload.len() <= region_bytes);
            payload.resize(region_bytes, 0);
            let tile = Tile::new(
                TileId::new(0),
                TileIndex::new(0, 0, 0),
                TileRegion::new(0, 0, TileExtent::new(4, 2)),
                &payload,
            );
            sink.write_tile(&tile).unwrap();
        }

        let bytes = writer.take_buffer();
        // The directory read path must reconstruct SampleFormat 2 -> "Int16".
        let mut reader = MemoryBinaryReader::from_slice(&bytes);
        let back = TiffBackend
            .deserialize_model(&mut reader)
            .expect("deserialize TIFF");
        let child = back
            .children()
            .first()
            .expect("TIFF root has one image child");
        assert_eq!(
            child.field("pixelType").unwrap(),
            match dtype {
                PixelType::Int16 => "Int16",
                PixelType::Int32 => "Int32",
                _ => unreachable!(),
            }
        );

        let mut source_reader = MemoryBinaryReader::from_slice(&bytes);
        let mut source = TiffBackend.open_image_source(&mut source_reader).unwrap();
        let t = source.read_tile(TileIndex::new(0, 0, 0)).unwrap();
        match dtype {
            PixelType::Int16 => {
                let data = t.data();
                let mut values = Vec::with_capacity(data.len() / 2);
                let mut i = 0;
                while i + 2 <= data.len() {
                    values.push(i16::from_be_bytes([data[i], data[i + 1]]));
                    i += 2;
                }
                assert_eq!(values, vec![i16::MIN, -1, 0, 1, 32767, -32767, 0, 0]);
            }
            PixelType::Int32 => {
                let data = t.data();
                let mut values = Vec::with_capacity(data.len() / 4);
                let mut i = 0;
                while i + 4 <= data.len() {
                    values.push(i32::from_be_bytes([
                        data[i],
                        data[i + 1],
                        data[i + 2],
                        data[i + 3],
                    ]));
                    i += 4;
                }
                assert_eq!(
                    values,
                    vec![i32::MIN, -1, 0, 1, 2147483647, -2147483647, 0, 0]
                );
            }
            _ => unreachable!(),
        }
    }
}

/// Deterministic encoder output for signed images: identical logical content
/// produces identical bytes.
#[test]
fn signed_write_is_deterministic() {
    for dtype in [PixelType::Int16, PixelType::Int32] {
        for codec in [CompressionKind::Deflate, CompressionKind::Lzw] {
            let mut scene = Scene::new();
            scene
                .add_image(
                    ImageDescriptorBuilder::new(32, 16)
                        .pixel_type(dtype)
                        .channel_count(1)
                        .compression(Some(codec))
                        .build(),
                )
                .expect("add image");
            let first = typed_scene_write(&scene, 32, 16, dtype);
            let second = typed_scene_write(&scene, 32, 16, dtype);
            assert_eq!(first, second, "{dtype:?}+{codec:?} must be deterministic");
        }
    }
}

/// Regression: unsigned and float types keep their exact typed identity and
/// values after this increment.
#[test]
fn existing_unsigned_and_float_types_unchanged() {
    for dtype in [
        PixelType::UInt16,
        PixelType::UInt32,
        PixelType::Float32,
        PixelType::Float64,
    ] {
        let (bytes, back) =
            typed_roundtrip(dtype, CompressionKind::Deflate, 48, 32, Some((16, 16)));
        let img = back.image_at(0).unwrap();
        assert_eq!(img.pixel_type(), dtype);
        assert_samples_exact(&bytes, 48, 32, dtype);
    }
}

/// Unsupported signed bit depths (Int8/Int64 are not part of the supported
/// typed vocabulary) fail explicitly at the plan boundary.
#[test]
fn int8_and_int64_rejected_not_mapped_to_unsigned() {
    for bad in ["Int8", "Int64"] {
        let mut model = StorageModel::new();
        model.set_field("imageWidth", "8");
        model.set_field("imageHeight", "8");
        model.set_field("samplesPerPixel", "1");
        model.set_field("pixelType", bad);
        model.set_field("compression", "None");

        let mut writer = MemoryBinaryWriter::new();
        let err = TiffBackend
            .serialize_model(&model, &mut writer)
            .expect_err("Int8/Int64 must be rejected");
        assert_eq!(err.code(), ErrorCode::InvalidArgument);
    }
}

/// JPEG stays 8-bit-only: signed samples with JPEG are refused by the typed
/// writer and by the physical plan, never silently encoded as unsigned.
#[test]
fn jpeg_with_signed_samples_is_rejected() {
    // Typed path.
    let mut scene = Scene::new();
    scene
        .add_image(
            ImageDescriptorBuilder::new(16, 16)
                .pixel_type(PixelType::Int16)
                .channel_count(1)
                .compression(Some(CompressionKind::Jpeg))
                .build(),
        )
        .expect("add image");
    let err = SceneSerializer
        .serialize(&scene)
        .expect_err("JPEG+Int16 must fail");
    assert_eq!(err.code(), ErrorCode::InvalidArgument);
    assert!(
        err.message().contains("Jpeg compression requires UInt8"),
        "unexpected message: {}",
        err.message()
    );

    // Physical path.
    let mut model = StorageModel::new();
    model.set_field("imageWidth", "16");
    model.set_field("imageHeight", "16");
    model.set_field("samplesPerPixel", "1");
    model.set_field("pixelType", "Int32");
    model.set_field("compression", "Jpeg");
    let mut writer = MemoryBinaryWriter::new();
    let err = TiffBackend
        .serialize_model(&model, &mut writer)
        .expect_err("JPEG+Int32 must fail");
    assert_eq!(err.code(), ErrorCode::InvalidArgument);
    assert!(
        err.message().contains("Jpeg compression requires UInt8"),
        "unexpected message: {}",
        err.message()
    );
}

/// Unknown pixelType strings are rejected by the typed deserializer and never
/// silently downgraded to another type.
#[test]
fn unknown_pixel_type_rejected() {
    let mut model = StorageModel::new();
    model.set_field("imageWidth", "8");
    model.set_field("imageHeight", "8");
    model.set_field("samplesPerPixel", "1");
    model.set_field("pixelType", "NotAPixelType");
    model.set_field("compression", "None");

    let mut writer = MemoryBinaryWriter::new();
    let err = TiffBackend
        .serialize_model(&model, &mut writer)
        .expect_err("unknown pixelType must fail");
    assert_eq!(err.code(), ErrorCode::InvalidArgument);
}
