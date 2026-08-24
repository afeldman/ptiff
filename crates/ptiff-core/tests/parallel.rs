//! Phase 5 integration tests: Rayon parallel tile compression/decompression
//! must produce **byte-identical** output to the sequential paths (§6.5
//! "Bestimmtheit"). These tests run only when the `parallel` feature is
//! enabled (the parallel methods don't exist otherwise).
#![cfg(feature = "parallel")]

use ptiff_core::io::backend::tiff::header::write_tiff_header;
use ptiff_core::io::backend::tiff::{
    interpret_tiff_ifd, plan_tiff_write, read_tiff_header, read_tiff_ifd, write_tiff_ifd,
    TiffImageSink, TiffImageSource,
};
use ptiff_core::io::{
    ImageSink, ImageSource, MemoryBinaryReader, MemoryBinaryWriter, StorageModel,
};
use ptiff_core::tile::{Tile, TileExtent, TileIndex, TileRegion};
use ptiff_core::TileId;

/// Builds a 16x16-tiled grayscale model.
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

/// Writes a tiled+compressed image sequentially (via `write_tile` in
/// row-major order) and returns the raw file bytes.
fn write_tiled_sequential(model: &StorageModel, payload: &[u8]) -> Vec<u8> {
    let plan = plan_tiff_write(model).unwrap();
    let mut w = MemoryBinaryWriter::new();
    let is_big = false;
    let header_size = 8;
    write_tiff_header(&mut w, header_size, is_big).unwrap();
    write_tiff_ifd(&mut w, plan.entries.clone(), is_big, 0).unwrap();
    {
        let mut sink = TiffImageSink::new(&mut w, plan.directory.clone());
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
    }
    w.take_buffer()
}

/// Writes a tiled+compressed image via the Phase 5 parallel path
/// (`write_compressed_tiles_parallel`) and returns the raw file bytes.
fn write_tiled_parallel(model: &StorageModel, payload: &[u8]) -> Vec<u8> {
    let plan = plan_tiff_write(model).unwrap();
    let mut w = MemoryBinaryWriter::new();
    let is_big = false;
    let header_size = 8;
    write_tiff_header(&mut w, header_size, is_big).unwrap();
    write_tiff_ifd(&mut w, plan.entries.clone(), is_big, 0).unwrap();
    {
        let mut sink = TiffImageSink::new(&mut w, plan.directory.clone());
        let layout = *sink.layout();
        let cols = layout.columns(0);
        let rows = layout.rows(0);
        let tile_bytes = (layout.tile_size.width * layout.tile_size.height) as usize;
        let mut tiles: Vec<Vec<u8>> = Vec::with_capacity((cols * rows) as usize);
        for linear in 0..((cols * rows) as usize) {
            let start = linear * tile_bytes;
            tiles.push(payload[start..start + tile_bytes].to_vec());
        }
        sink.write_compressed_tiles_parallel(&tiles).unwrap();
    }
    w.take_buffer()
}

/// Decodes every tile via the sequential `read_tile` path, row-major.
fn read_tile_grid_sequential(bytes: &[u8]) -> Vec<u8> {
    let mut r = MemoryBinaryReader::from_slice(bytes);
    let header = read_tiff_header(&mut r).unwrap();
    let ifd = read_tiff_ifd(&mut r, 8, header.endian, header.is_big_tiff).unwrap();
    let directory = interpret_tiff_ifd(&ifd).unwrap();
    let mut source = TiffImageSource::new(&mut r, directory);
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

/// Decodes every tile via the Phase 5 parallel path (`read_all_tiles_parallel`).
fn read_tile_grid_parallel(bytes: &[u8]) -> Vec<u8> {
    let mut r = MemoryBinaryReader::from_slice(bytes);
    let header = read_tiff_header(&mut r).unwrap();
    let ifd = read_tiff_ifd(&mut r, 8, header.endian, header.is_big_tiff).unwrap();
    let directory = interpret_tiff_ifd(&ifd).unwrap();
    let mut source = TiffImageSource::new(&mut r, directory);
    source
        .read_all_tiles_parallel()
        .unwrap()
        .into_iter()
        .flatten()
        .collect()
}

/// A non-trivial payload spanning many tiles with enough structure that
/// compression actually compresses differently per tile (deterministic input).
fn payload_for(width: u32, height: u32) -> Vec<u8> {
    (0..(width * height) as usize)
        .map(|i| {
            // A repeating banded pattern: compresses with all lossless codecs
            // while remaining distinct across tiles.
            let row = (i as u32) / width;
            let col = (i as u32) % 8;
            ((row % 7) * 16 + col) as u8
        })
        .collect()
}

/// Parallel compress == sequential compress, byte-for-byte (deterministic
/// layout, §6.5). Uses > MIN_TILES_FOR_PARALLEL tiles so the Rayon pool path
/// is genuinely exercised.
#[test]
fn parallel_compress_matches_sequential_byte_for_byte_lzw() {
    let (width, height) = (64u32, 64u32); // 4 cols x 4 rows = 16 tiles > threshold
    let payload = payload_for(width, height);
    let model = tiled_model(width, height, "LZW", "HorizontalDifferencing");
    let seq = write_tiled_sequential(&model, &payload);
    let par = write_tiled_parallel(&model, &payload);
    assert_eq!(seq, par, "parallel LZW output must byte-match sequential");
}

#[test]
fn parallel_compress_matches_sequential_byte_for_byte_deflate() {
    let (width, height) = (128u32, 32u32); // 8 cols x 2 rows = 16 tiles
    let payload = payload_for(width, height);
    let model = tiled_model(width, height, "Deflate", "None");
    let seq = write_tiled_sequential(&model, &payload);
    let par = write_tiled_parallel(&model, &payload);
    assert_eq!(
        seq, par,
        "parallel Deflate output must byte-match sequential"
    );
}

#[test]
fn parallel_compress_matches_sequential_byte_for_byte_packbits() {
    let (width, height) = (64u32, 96u32); // 4 cols x 6 rows = 24 tiles
    let payload = payload_for(width, height);
    let model = tiled_model(width, height, "PackBits", "HorizontalDifferencing");
    let seq = write_tiled_sequential(&model, &payload);
    let par = write_tiled_parallel(&model, &payload);
    assert_eq!(
        seq, par,
        "parallel PackBits output must byte-match sequential"
    );
}

/// Parallel decompress == sequential decompress.
#[test]
fn parallel_decompress_matches_sequential_pixels() {
    let (width, height) = (96u32, 64u32); // 6 cols x 4 rows = 24 tiles
    let payload = payload_for(width, height);
    let model = tiled_model(width, height, "LZW", "HorizontalDifferencing");
    let bytes = write_tiled_sequential(&model, &payload);

    let seq = read_tile_grid_sequential(&bytes);
    let par = read_tile_grid_parallel(&bytes);
    assert_eq!(
        seq, par,
        "parallel decompress must byte-match sequential pixels"
    );
    assert_eq!(seq, payload, "round-tripped pixels must equal the input");
}

/// Full dual-path determinism: what the parallel SINK writes, the parallel
/// SOURCE reads back to the exact original pixels.
#[test]
fn parallel_write_then_parallel_read_round_trips() {
    let (width, height) = (64u32, 128u32); // 4 cols x 8 rows = 32 tiles
    let payload = payload_for(width, height);
    let model = tiled_model(width, height, "Deflate", "HorizontalDifferencing");
    let par_bytes = write_tiled_parallel(&model, &payload);
    let par_read = read_tile_grid_parallel(&par_bytes);
    assert_eq!(
        par_read, payload,
        "parallel write -> parallel read round-trips"
    );
}

/// Small tile sets (below the parallelization threshold) must still write
/// identically — the sequential fallback path inside `write_compressed_tiles_parallel`.
#[test]
fn small_tile_set_falls_back_to_sequential_but_identical() {
    let (width, height) = (32u32, 32u32); // 2x2 = 4 tiles < threshold
    let payload = payload_for(width, height);
    let model = tiled_model(width, height, "LZW", "None");
    let seq = write_tiled_sequential(&model, &payload);
    let par = write_tiled_parallel(&model, &payload);
    assert_eq!(seq, par);
}
