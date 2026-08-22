//! End-to-end single-image tiled write with compression through the *public*
//! [`TiffBackend`]/[`StorageBackend`] API — the same sequence the idiomatic
//! `ptiff::Tiff::write_image_pixels` wraps. Exercises `serialize_model`
//! (header + IFD) followed by `open_image_sink` streaming the tile payloads
//! back-to-back, then reading the tiles back with our own reader.

use ptiff_core::io::backend::tiff::TiffBackend;
use ptiff_core::io::{MemoryBinaryReader, MemoryBinaryWriter, StorageBackend, StorageModel};
use ptiff_core::tile::{Tile, TileExtent, TileIndex, TileLayout, TileRegion};
use ptiff_core::{ErrorCode, TileId};

/// Builds a 16x16-tiled grayscale storage model.
fn tiled_model(width: u32, height: u32, compression: &str, predictor: &str) -> StorageModel {
    let mut m = StorageModel::new();
    m.set_field("imageWidth", width.to_string());
    m.set_field("imageHeight", height.to_string());
    m.set_field("samplesPerPixel", "1");
    m.set_field("pixelType", "UInt8");
    m.set_field("tileWidth", "16");
    m.set_field("tileHeight", "16");
    m.set_field("compression", compression);
    m.set_field("predictor", predictor);
    m
}

/// Writes a single tiled image: `serialize_model` (header + IFD) then
/// `open_image_sink`, streaming each 16x16 tile in row-major order. Returns
/// the file bytes and the tile grid layout.
fn write_tiled_image(model: &StorageModel, payload: &[u8]) -> (Vec<u8>, TileLayout) {
    let mut writer = MemoryBinaryWriter::new();
    TiffBackend.serialize_model(model, &mut writer).unwrap();

    let layout = {
        let mut sink = TiffBackend
            .open_image_sink(&mut writer, model)
            .expect("open single-image sink");
        let layout = *sink.layout();
        let cols = layout.columns(0);
        let rows = layout.rows(0);
        let tile_bytes = (layout.tile_size.width * layout.tile_size.height) as usize;
        for row in 0..rows {
            for col in 0..cols {
                let linear = (row * cols + col) as usize;
                let start = linear * tile_bytes;
                let tile = Tile::new(
                    TileId::new(linear as u64),
                    TileIndex::new(col, row, 0),
                    TileRegion::new(0, 0, TileExtent::new(16, 16)),
                    &payload[start..start + tile_bytes],
                );
                sink.write_tile(&tile).unwrap();
            }
        }
        layout
    };

    (writer.take_buffer(), layout)
}

/// Reads back every tile through the public `open_image_source` API and
/// concatenates them row-major.
fn read_tile_grid(bytes: &[u8]) -> Vec<u8> {
    let mut reader = MemoryBinaryReader::from_slice(bytes);
    let mut source = TiffBackend
        .open_image_source(&mut reader)
        .expect("open image source");
    let layout = *source.layout();
    let cols = layout.columns(0);
    let rows = layout.rows(0);
    let mut out = Vec::new();
    for row in 0..rows {
        for col in 0..cols {
            let t = source.read_tile(TileIndex::new(col, row, 0)).unwrap();
            out.extend_from_slice(t.data());
        }
    }
    out
}

#[test]
fn tiled_lzw_compressed_single_image_round_trips() {
    let (width, height) = (48u32, 64u32); // 3 cols x 4 rows of 16x16 tiles
    let payload: Vec<u8> = (0..(width * height) as usize)
        .map(|i| (((i as u32) / width) % 13 + ((i as u32) % 5)) as u8)
        .collect();
    let model = tiled_model(width, height, "LZW", "HorizontalDifferencing");
    let (bytes, layout) = write_tiled_image(&model, &payload);
    assert_eq!(layout.columns(0), 3);
    assert_eq!(layout.rows(0), 4);
    let decoded = read_tile_grid(&bytes);
    assert_eq!(decoded, payload);
}

#[test]
fn tiled_deflate_compressed_single_image_round_trips() {
    let (width, height) = (32u32, 32u32); // 2 x 2 tiles
    let payload: Vec<u8> = (0..(width * height) as usize)
        .map(|i| (i % 251) as u8)
        .collect();
    let model = tiled_model(width, height, "Deflate", "None");
    let (bytes, layout) = write_tiled_image(&model, &payload);
    assert_eq!(layout.columns(0), 2);
    assert_eq!(layout.rows(0), 2);
    assert_eq!(read_tile_grid(&bytes), payload);
}

#[test]
fn multi_image_tiled_compressed_is_rejected() {
    // Multi-image (`serialize_model_list`) keeps a static layout and cannot
    // reserve a variable-size compressed-tiled region; it must reject the plan.
    let models = vec![tiled_model(32, 32, "LZW", "None")];
    let mut writer = MemoryBinaryWriter::new();
    let err = TiffBackend
        .serialize_model_list(&models, &mut writer)
        .unwrap_err();
    assert_eq!(err.code(), ErrorCode::InvalidArgument);
}
