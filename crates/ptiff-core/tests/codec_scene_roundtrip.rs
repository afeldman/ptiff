//! P0-03 (WP-P0-03): typed Scene codec vocabulary round-trip.
//!
//! Regression coverage for the typed-vs-physical codec boundary: the TIFF
//! image layer could physically read/write PackBits/Deflate/JPEG, but the
//! typed Scene path only reconstructed `None`/`LZW` (SceneDeserializer) and
//! `CompressionKind` did not even name PackBits. These tests prove the full
//! typed path for the supported vocabulary:
//!
//! ```text
//! Scene -> SceneSerializer -> TIFF bytes (pixels written via the Sink)
//!      -> SceneDeserializer -> Scene
//! ```
//!
//! preserving the codec, and — for lossless codecs — the exact pixels.
//! Lossy JPEG round-trips structurally (codec identity + dimensions), never
//! with an exact-pixel assumption.

use ptiff_core::image::ImageDescriptorBuilder;
use ptiff_core::io::backend::tiff::TiffBackend;
use ptiff_core::io::{
    Deserializer, MemoryBinaryReader, MemoryBinaryWriter, SceneDeserializer, SceneSerializer,
    Serializer, StorageBackend, StorageModel,
};
use ptiff_core::tile::{Tile, TileExtent, TileIndex, TileRegion};
use ptiff_core::{CompressionKind, PixelType, Scene, TileId};

const LOSSLESS_CODECS: [CompressionKind; 4] = [
    CompressionKind::None,
    CompressionKind::Lzw,
    CompressionKind::PackBits,
    CompressionKind::Deflate,
];

/// Deterministic UInt8 sample at pixel (row, col).
fn sample_u8(row: u32, col: u32, width: u32) -> u8 {
    ((u64::from(row) * u64::from(width) + u64::from(col)) % 251) as u8
}

/// Deterministic Float64 sample at pixel (row, col) — a compact set of
/// exact and non-terminating binary fractions so round-trip is bitwise
/// meaningful.
fn sample_f64(row: u32, col: u32, width: u32) -> f64 {
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

/// Serializes a single-image Scene (header + IFD) and streams the pixel
/// payload through the Sink. The payload is generated deterministically per
/// pixel, so no separate payload buffer is needed.
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
                        match dtype {
                            PixelType::Float64 => {
                                tile_bytes.extend_from_slice(
                                    &sample_f64(global_row, global_col, width).to_le_bytes(),
                                );
                            }
                            _ => tile_bytes.push(sample_u8(global_row, global_col, width)),
                        }
                    }
                }
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

/// Reads every sample back through the image source and asserts lossless
/// equality with the deterministic generator.
fn assert_pixels_exact(
    bytes: &[u8],
    width: u32,
    height: u32,
    dtype: PixelType,
    tolerance: Option<u8>,
) {
    let mut reader = MemoryBinaryReader::from_slice(bytes);
    let mut source = TiffBackend
        .open_image_source(&mut reader)
        .expect("open source");
    let layout = *source.layout();
    let cols = layout.columns(0);
    let rows = layout.rows(0);
    let tw = layout.tile_size.width;
    let th = layout.tile_size.height;

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
                    match dtype {
                        PixelType::Float64 => {
                            let start = idx * 8;
                            let value =
                                f64::from_le_bytes(t.data()[start..start + 8].try_into().unwrap());
                            assert_eq!(
                                value.to_bits(),
                                sample_f64(global_row, global_col, width).to_bits(),
                                "pixel differs at ({global_row},{global_col})"
                            );
                        }
                        _ => {
                            let actual = t.data()[idx];
                            let expected = sample_u8(global_row, global_col, width);
                            match tolerance {
                                None => assert_eq!(
                                    actual, expected,
                                    "pixel differs at ({global_row},{global_col})"
                                ),
                                Some(max) => {
                                    let diff = i32::from(actual) - i32::from(expected);
                                    assert!(
                                        diff.abs() <= max as i32,
                                        "lossy sample off by {diff} at ({global_row},{global_col})"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Full typed round-trip for one (codec, layout, dtype) combination.
fn typed_codec_roundtrip(
    codec: CompressionKind,
    width: u32,
    height: u32,
    tile: Option<(u32, u32)>,
    dtype: PixelType,
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

#[test]
fn lossless_codecs_round_trip_strips() {
    for codec in LOSSLESS_CODECS {
        let (width, height) = (40u32, 24u32);
        let (bytes, back) = typed_codec_roundtrip(codec, width, height, None, PixelType::UInt8);
        let img = back.image_at(0).unwrap();
        assert_eq!(
            img.compression(),
            Some(codec),
            "codec {codec:?} must survive the typed Scene round-trip"
        );
        assert_eq!(img.pixel_type(), PixelType::UInt8);
        assert_eq!(img.width(), width);
        assert_eq!(img.height(), height);
        assert_eq!(img.channel_count(), 1);
        assert_pixels_exact(&bytes, width, height, PixelType::UInt8, None);
    }
}

#[test]
fn lossless_codecs_round_trip_tiled() {
    for codec in LOSSLESS_CODECS {
        let (width, height) = (48u32, 32u32);
        let (bytes, back) =
            typed_codec_roundtrip(codec, width, height, Some((16, 16)), PixelType::UInt8);
        let img = back.image_at(0).unwrap();
        assert_eq!(
            img.compression(),
            Some(codec),
            "codec {codec:?} must survive the typed Scene round-trip"
        );
        assert_pixels_exact(&bytes, width, height, PixelType::UInt8, None);
    }
}

/// Deflate + Float64 through the FULL typed path (Scene -> bytes -> Scene):
/// the P0-02 pixel symmetry combined with the P0-03 read-back vocabulary.
#[test]
fn deflate_float64_round_trips_through_typed_scene() {
    let (width, height) = (32u32, 32u32);
    let (bytes, back) = typed_codec_roundtrip(
        CompressionKind::Deflate,
        width,
        height,
        Some((16, 16)),
        PixelType::Float64,
    );
    let img = back.image_at(0).unwrap();
    assert_eq!(img.compression(), Some(CompressionKind::Deflate));
    assert_eq!(img.pixel_type(), PixelType::Float64);
    assert_pixels_exact(&bytes, width, height, PixelType::Float64, None);
}

/// JPEG is part of the typed vocabulary but lossy: codec identity, structure
/// and rough pixel fidelity round-trip; exact equality is never assumed.
#[test]
fn jpeg_round_trips_through_typed_scene_with_lossy_pixels() {
    for tile in [None, Some((16, 16))] {
        let (width, height) = if tile.is_some() { (32, 48) } else { (32, 16) };
        let (bytes, back) =
            typed_codec_roundtrip(CompressionKind::Jpeg, width, height, tile, PixelType::UInt8);
        let img = back.image_at(0).unwrap();
        assert_eq!(img.compression(), Some(CompressionKind::Jpeg));
        assert_eq!(img.pixel_type(), PixelType::UInt8);
        assert_pixels_exact(&bytes, width, height, PixelType::UInt8, Some(40));
    }
}

/// Deterministic encoder output: identical logical content must produce
/// identical bytes (per codec), so fixtures stay reproducible.
#[test]
fn codec_write_is_deterministic() {
    for codec in LOSSLESS_CODECS {
        let mut scene = Scene::new();
        scene
            .add_image(
                ImageDescriptorBuilder::new(32, 32)
                    .pixel_type(PixelType::UInt8)
                    .channel_count(1)
                    .compression(Some(codec))
                    .tile(16, 16)
                    .build(),
            )
            .expect("add image");
        let first = typed_scene_write(&scene, 32, 32, PixelType::UInt8);
        let second = typed_scene_write(&scene, 32, 32, PixelType::UInt8);
        assert_eq!(
            first, second,
            "codec {codec:?} output must be deterministic"
        );
    }
}

/// An unsupported codec string must fail explicitly at the physical layer —
/// never silently downgrade to another codec.
#[test]
fn unsupported_codec_is_rejected_not_downgraded() {
    let mut model = StorageModel::new();
    model.set_field("imageWidth", "16");
    model.set_field("imageHeight", "16");
    model.set_field("samplesPerPixel", "1");
    model.set_field("pixelType", "UInt8");
    model.set_field("compression", "Zstd");

    let mut writer = MemoryBinaryWriter::new();
    let err = TiffBackend
        .serialize_model(&model, &mut writer)
        .expect_err("unsupported codec must be rejected");
    assert_eq!(err.code(), ptiff_core::ErrorCode::InvalidArgument);
    assert!(
        err.message().contains("unsupported compression"),
        "unexpected message: {}",
        err.message()
    );
}
