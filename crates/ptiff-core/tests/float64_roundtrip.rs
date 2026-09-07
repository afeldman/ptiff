//! P0-02 (WP-P0-02): Float64 read/write symmetry through the typed
//! pixel/serialization path.
//!
//! Regression coverage for the Float64 defect: the write side emitted
//! Float64 pixels (BitsPerSample 64 / SampleFormat 3 on disk) while the read
//! side rejected them (`resolve_pixel_type` accepted 32-bit floats only), and
//! the typed `SceneSerializer` refused Float64 altogether. These tests prove
//! the full path — `Scene` -> `SceneSerializer` -> TIFF bytes (header + IFD +
//! pixel payloads) -> read back as `Scene` and as raw pixel bytes — preserves
//! Float64 exactly:
//!
//! ```text
//! Float64 write -> TIFF/BigTIFF -> Float64 read -> same semantic pixels
//! ```
//!
//! Coverage: strip storage, tiled storage, the lossless codecs currently
//! supported by the typed path (None, LZW, Deflate), deterministic output,
//! and IEEE-754-representative sample values compared bitwise.

use ptiff_core::image::ImageDescriptorBuilder;
use ptiff_core::io::backend::tiff::TiffBackend;
use ptiff_core::io::{
    Deserializer, MemoryBinaryReader, MemoryBinaryWriter, SceneDeserializer, SceneSerializer,
    Serializer, StorageBackend, StorageModel,
};
use ptiff_core::tile::{Tile, TileExtent, TileIndex, TileRegion};
use ptiff_core::{CompressionKind, PixelType, Scene, TileId};

/// A deterministic sample set that exercises meaningful IEEE-754 behavior:
/// subnormals, the smallest/largest normal magnitudes, exact binary
/// fractions, non-terminating binary fractions, and negative values.
const F64_SAMPLES: [f64; 10] = [
    0.0,
    1.0,
    -1.0,
    1.5,
    -2.75,
    1.0 / 3.0,
    0.1 + 0.2,
    5.0e-324,                     // smallest positive subnormal
    1.797_693_134_862_315_7e308,  // f64::MAX
    2.225_073_858_507_201_4e-308, // smallest positive normal
];

/// Deterministic Float64 value at pixel (row, col).
fn sample_value(row: u32, col: u32, width: u32) -> f64 {
    let idx = (u64::from(row) * u64::from(width) + u64::from(col)) % (F64_SAMPLES.len() as u64);
    F64_SAMPLES[idx as usize]
}

/// Writes `values` through the TIFF backend for a single-image model whose
/// `pixelType` is Float64, streaming every layout tile in row-major order.
/// Returns the file bytes.
fn write_f64_model(model: &StorageModel, width: u32, height: u32) -> Vec<u8> {
    let mut writer = MemoryBinaryWriter::new();
    TiffBackend.serialize_model(model, &mut writer).unwrap();

    let mut sink = TiffBackend
        .open_image_sink(&mut writer, model)
        .expect("open single-image Float64 sink");
    let layout = *sink.layout();
    let cols = layout.columns(0);
    let rows = layout.rows(0);
    let tw = layout.tile_size.width;
    let th = layout.tile_size.height;

    for row in 0..rows {
        for col in 0..cols {
            let mut tile_bytes = Vec::with_capacity((tw * th * 8) as usize);
            for ty in 0..th {
                for tx in 0..tw {
                    let global_row = row * th + ty;
                    let global_col = col * tw + tx;
                    // Layout tiles may overhang the image on the right/bottom
                    // edge; only in-bounds pixels carry values, the rest are
                    // zero-filled (the sizes chosen in these tests divide
                    // evenly, so this is a safety net, not the main path).
                    let value = if global_row < height && global_col < width {
                        sample_value(global_row, global_col, width)
                    } else {
                        0.0
                    };
                    tile_bytes.extend_from_slice(&value.to_le_bytes());
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
    drop(sink);
    writer.take_buffer()
}

/// Reads every tile back through `open_image_source` and asserts each pixel
/// matches `sample_value(global_row, global_col)` bitwise.
fn assert_pixels_bitwise(bytes: &[u8], width: u32, height: u32) {
    let mut reader = MemoryBinaryReader::from_slice(bytes);
    let mut source = TiffBackend
        .open_image_source(&mut reader)
        .expect("open Float64 image source");
    let layout = *source.layout();
    let cols = layout.columns(0);
    let rows = layout.rows(0);
    let tw = layout.tile_size.width;
    let th = layout.tile_size.height;

    for row in 0..rows {
        for col in 0..cols {
            let t = source
                .read_tile(TileIndex::new(col, row, 0))
                .expect("read Float64 tile");
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
                    // Pixels are compared by their global image coordinate;
                    // storage order (tile-major vs row-major) is a container
                    // property, not a semantic one.
                    let start = (ty * tw + tx) as usize * 8;
                    let value = f64::from_le_bytes(t.data()[start..start + 8].try_into().unwrap());
                    assert_eq!(
                        value.to_bits(),
                        sample_value(global_row, global_col, width).to_bits(),
                        "Float64 pixel differs after round-trip at ({global_row}, {global_col})"
                    );
                }
            }
        }
    }
}

/// Serializes a single-image Scene into the child StorageModel the TIFF
/// backend consumes (mirrors `scene_tiff_roundtrip.rs`).
fn scene_image_model(scene: &Scene) -> StorageModel {
    let root: StorageModel = SceneSerializer.serialize(scene).expect("serialize scene");
    root.children()
        .first()
        .expect("scene has exactly one image")
        .clone()
}

/// Full typed round-trip: Scene -> StorageModel -> TIFF bytes (with pixel
/// payloads) -> Scene, then a bitwise pixel comparison.
fn typed_f64_scene_roundtrip(
    width: u32,
    height: u32,
    compression: Option<CompressionKind>,
    tile: Option<(u32, u32)>,
) -> (Vec<u8>, Scene) {
    let mut builder = ImageDescriptorBuilder::new(width, height)
        .pixel_type(PixelType::Float64)
        .channel_count(1)
        .compression(compression);
    if let Some((tw, th)) = tile {
        builder = builder.tile(tw, th);
    }
    let mut scene = Scene::new();
    scene.add_image(builder.build()).expect("add Float64 image");

    let model = scene_image_model(&scene);
    let bytes = write_f64_model(&model, width, height);

    let mut reader = MemoryBinaryReader::from_slice(&bytes);
    let back_model = TiffBackend
        .deserialize_model(&mut reader)
        .expect("deserialize Float64 TIFF");
    let back_scene = SceneDeserializer
        .deserialize(&back_model)
        .expect("deserialize Float64 scene");

    (bytes, back_scene)
}

#[test]
fn float64_scene_roundtrip_strips() {
    // Untiled model -> single-strip (RowsPerStrip == height) storage.
    let (width, height) = (32u32, 16u32);
    let (bytes, back) = typed_f64_scene_roundtrip(width, height, None, None);

    let img = back.image_at(0).unwrap();
    assert_eq!(img.pixel_type(), PixelType::Float64);
    assert_eq!(img.width(), width);
    assert_eq!(img.height(), height);
    assert_eq!(img.channel_count(), 1);
    assert_pixels_bitwise(&bytes, width, height);
}

#[test]
fn float64_scene_roundtrip_tiled_lzw() {
    // 32x32 image, 16x16 tiles, LZW compression (lossless).
    let (width, height) = (32u32, 32u32);
    let (bytes, back) =
        typed_f64_scene_roundtrip(width, height, Some(CompressionKind::Lzw), Some((16, 16)));

    let img = back.image_at(0).unwrap();
    assert_eq!(img.pixel_type(), PixelType::Float64);
    assert_eq!(img.width(), width);
    assert_eq!(img.height(), height);
    assert_eq!(img.channel_count(), 1);
    assert_pixels_bitwise(&bytes, width, height);
}

#[test]
fn float64_pixels_roundtrip_tiled_deflate() {
    // Tiled 64x48, Deflate compression. Pixel-level round-trip via the image
    // source; the typed Scene read-back of Deflate files is tracked
    // separately under P0-03 (codec vocabulary round-trip).
    let (width, height) = (64u32, 48u32);
    let mut scene = Scene::new();
    scene
        .add_image(
            ImageDescriptorBuilder::new(width, height)
                .pixel_type(PixelType::Float64)
                .channel_count(1)
                .compression(Some(CompressionKind::Deflate))
                .tile(16, 16)
                .build(),
        )
        .expect("add Float64 image");

    let model = scene_image_model(&scene);
    let bytes = write_f64_model(&model, width, height);
    assert_pixels_bitwise(&bytes, width, height);
}

#[test]
fn float64_pixels_roundtrip_uncompressed_tiled() {
    // Tiled 48x64, no compression.
    let (width, height) = (48u32, 64u32);
    let mut scene = Scene::new();
    scene
        .add_image(
            ImageDescriptorBuilder::new(width, height)
                .pixel_type(PixelType::Float64)
                .channel_count(1)
                .compression(None)
                .tile(16, 16)
                .build(),
        )
        .expect("add Float64 image");

    let model = scene_image_model(&scene);
    let bytes = write_f64_model(&model, width, height);
    assert_pixels_bitwise(&bytes, width, height);
}

#[test]
fn float64_write_is_deterministic() {
    // Golden-style determinism: identical logical content must produce
    // identical bytes, so Float64 fixtures stay reproducible.
    let (width, height) = (16u32, 16u32);
    let mut scene = Scene::new();
    scene
        .add_image(
            ImageDescriptorBuilder::new(width, height)
                .pixel_type(PixelType::Float64)
                .channel_count(1)
                .tile(16, 16)
                .build(),
        )
        .expect("add Float64 image");

    let model = scene_image_model(&scene);
    let first = write_f64_model(&model, width, height);
    let second = write_f64_model(&model, width, height);
    assert_eq!(first, second, "Float64 TIFF output must be deterministic");
    assert!(!first.is_empty());
}

#[test]
fn float64_grid_payload_uses_ieee754_representative_values() {
    // Sanity: the sample set really covers the magnitudes the round-trip
    // tests rely on (bit patterns must be distinct and non-NaN).
    let mut bits: Vec<u64> = F64_SAMPLES.iter().map(|v| v.to_bits()).collect();
    bits.sort_unstable();
    bits.dedup();
    assert_eq!(
        bits.len(),
        F64_SAMPLES.len(),
        "sample values must be distinct"
    );
    for v in F64_SAMPLES {
        assert!(!v.is_nan(), "sample set must not contain NaN");
    }
}
