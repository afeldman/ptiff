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

/// Builds a 16x16-tiled N-band (multispectral) storage model.
fn tiled_nband_model(width: u32, height: u32, samples: u32) -> StorageModel {
    let mut m = StorageModel::new();
    m.set_field("imageWidth", width.to_string());
    m.set_field("imageHeight", height.to_string());
    m.set_field("samplesPerPixel", samples.to_string());
    m.set_field("pixelType", "UInt8");
    m.set_field("tileWidth", "16");
    m.set_field("tileHeight", "16");
    m.set_field("compression", "None");
    m.set_field("predictor", "None");
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

/// Writes a single tiled N-band image whose tile payloads carry
/// `tile_w.tile_h.samples` bytes each, and reads the tile grid back.
fn write_and_read_tiled(model: &StorageModel, samples: u32) -> (Vec<u8>, Vec<u8>) {
    let width = model.field("imageWidth").unwrap().parse::<u32>().unwrap();
    let height = model.field("imageHeight").unwrap().parse::<u32>().unwrap();
    let tile_w: u32 = model.field("tileWidth").unwrap().parse().unwrap();
    let tile_h: u32 = model.field("tileHeight").unwrap().parse().unwrap();
    let cols = width.div_ceil(tile_w);
    let rows = height.div_ceil(tile_h);
    let tile_bytes = (tile_w * tile_h * samples) as usize;
    let payload: Vec<u8> = (0..(cols as usize * rows as usize * tile_bytes))
        .map(|i| (i % 251) as u8)
        .collect();

    let mut writer = MemoryBinaryWriter::new();
    TiffBackend.serialize_model(model, &mut writer).unwrap();
    {
        let mut sink = TiffBackend
            .open_image_sink(&mut writer, model)
            .expect("open single-image sink");
        let layout = *sink.layout();
        let cols = layout.columns(0);
        let rows = layout.rows(0);
        for row in 0..rows {
            for col in 0..cols {
                let linear = (row * cols + col) as usize;
                let start = linear * tile_bytes;
                // Each read-back tile is exactly tile_w * tile_h * samples bytes;
                // the C++ reference verifies this for a 6-band image.
                let tile = Tile::new(
                    TileId::new(linear as u64),
                    TileIndex::new(col, row, 0),
                    TileRegion::new(0, 0, TileExtent::new(tile_w, tile_h)),
                    &payload[start..start + tile_bytes],
                );
                sink.write_tile(&tile).unwrap();
            }
        }
    }
    let bytes = writer.take_buffer();

    let mut reader = MemoryBinaryReader::from_slice(&bytes);
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
    (bytes, out)
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

#[test]
fn tiled_six_band_multispectral_round_trips() {
    // Parity with the C++ reference `tiff_backend_e2e_test.cpp`
    // ("...round-trips a 6-band multispectral image byte-for-byte"). A
    // tiled 16x16, 6-band UInt8 image must survive write -> read exactly,
    // proving the ExtraSamples / samplesPerPixel>3 path end-to-end.
    let (w, h, samples) = (16u32, 16u32, 6u32);
    let model = tiled_nband_model(w, h, samples);
    let (bytes, decoded) = write_and_read_tiled(&model, samples);
    assert!(!bytes.is_empty());
    assert_eq!(decoded.len(), (w * h * samples) as usize);
    // Payload in write_and_read_tiled is `i % 251` over all tile bytes; the
    // same formula must reproduce the read-back bytes exactly (byte-for-byte).
    let expected: Vec<u8> = (0..(w * h * samples) as usize)
        .map(|i| (i % 251) as u8)
        .collect();
    assert_eq!(decoded, expected);
}
