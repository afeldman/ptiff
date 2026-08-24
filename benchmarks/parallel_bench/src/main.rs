//! Rayon parallel tile compression/decompression scaling benchmark.
//!
//! Measures how the Phase-5 parallel paths (`TiffImageSink::write_compressed_tiles_parallel`
//! and `TiffImageSource::read_all_tiles_parallel`) scale with the number of
//! worker threads by running each measurement inside an explicit
//! `rayon::ThreadPool` sized 1/2/4/8 (num_cpus-aware). Only the CPU-bound
//! work (compression/decompression) is parallel; I/O stays sequential by
//! design (§6.2). Everything uses a deterministic, generated payload — no
//! external fixtures — so scaling curves are reproducible.
//!
//! Pairs with hypotheses H7 (parallel tile-decompression scales sub-linear to
//! linear) and H8 (parallel compression ≥ sequential at ≥2 cores) in
//! `PTIFF-1.0-RUST-CORE-PLAN.md` §12.
//!
//! Output: `--out <file.json>` (median-of-repeats per thread count, in ms),
//! plus a human-readable table on stdout.

use std::time::{Duration, Instant};

use ptiff_core::io::backend::tiff::header::write_tiff_header;
use ptiff_core::io::backend::tiff::{
    interpret_tiff_ifd, plan_tiff_write, read_tiff_header, read_tiff_ifd, write_tiff_ifd,
    TiffImageSink, TiffImageSource,
};
use ptiff_core::io::{ImageSink, MemoryBinaryReader, MemoryBinaryWriter, StorageModel};
use ptiff_core::tile::{Tile, TileExtent, TileIndex, TileRegion};
use ptiff_core::TileId;
use rayon::ThreadPoolBuilder;
use serde_json::json;

/// 16x16 pixels per tile.
const TILE: u32 = 16;
/// Number of timed samples (median taken).
const REPEATS: usize = 20;
/// Warm-up candidates processed before the timed loop.
const WARMUP: usize = 3;

/// Human name for each thread-count we measure.
const THREADS: [usize; 4] = [1, 2, 4, 8];

/// Minimal tiled+compressed storage model (grayscale UInt8).
fn tiled_model(width: u32, height: u32, compression: &str, predictor: &str) -> StorageModel {
    let mut m = StorageModel::new();
    m.set_field("imageWidth", width.to_string());
    m.set_field("imageHeight", height.to_string());
    m.set_field("samplesPerPixel", "1");
    m.set_field("pixelType", "UInt8");
    m.set_field("tileWidth", TILE.to_string());
    m.set_field("tileHeight", TILE.to_string());
    m.set_field("compression", compression.to_string());
    m.set_field("predictor", predictor.to_string());
    m
}

/// Deterministic, banded payload that compresses well with lossless codecs.
fn payload_for(width: u32, height: u32) -> Vec<u8> {
    (0..(width * height) as usize)
        .map(|i| {
            let row = (i as u32) / width;
            let col = (i as u32) % 8;
            ((row % 7) * 16 + col) as u8
        })
        .collect()
}

/// Writes a tiled+compressed TIFF sequentially (row-major) so we have a real
/// compressed file on which to benchmark parallel *decompression*. Returns the
/// file bytes and the tile grid dims.
fn build_compressed_file(model: &StorageModel, payload: &[u8]) -> (Vec<u8>, u32, u32) {
    let plan = plan_tiff_write(model).unwrap();
    let mut w = MemoryBinaryWriter::new();
    write_tiff_header(&mut w, 8, false).unwrap();
    write_tiff_ifd(&mut w, plan.entries.clone(), false, 0).unwrap();
    let (cols, rows) = {
        let mut sink = TiffImageSink::new(&mut w, plan.directory.clone());
        let layout = *sink.layout();
        let cols = layout.columns(0);
        let rows = layout.rows(0);
        let tile_bytes = (TILE * TILE) as usize;
        for row in 0..rows {
            for col in 0..cols {
                let linear = (row * cols + col) as usize;
                let start = linear * tile_bytes;
                sink.write_tile(&Tile::new(
                    TileId::new(linear as u64),
                    TileIndex::new(col, row, 0),
                    TileRegion::new(0, 0, TileExtent::new(TILE, TILE)),
                    &payload[start..start + tile_bytes],
                ))
                .unwrap();
            }
        }
        (cols, rows)
    };
    (w.take_buffer(), cols, rows)
}

/// Parallel decompression of a whole compressed file inside a Rayon pool of
/// `threads` workers.
fn bench_decompress(bytes: &[u8], threads: usize) -> Duration {
    let pool = ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .unwrap();
    pool.install(|| {
        let mut r = MemoryBinaryReader::from_slice(bytes);
        let header = read_tiff_header(&mut r).unwrap();
        let ifd = read_tiff_ifd(&mut r, 8, header.endian, header.is_big_tiff).unwrap();
        let directory = interpret_tiff_ifd(&ifd).unwrap();
        let mut source = TiffImageSource::new(&mut r, directory);
        let start = Instant::now();
        let out = source.read_all_tiles_parallel().unwrap();
        let t = start.elapsed();
        // Keep the result alive so the work is not optimized away.
        std::hint::black_box(out);
        t
    })
}

/// Parallel compression (write) of raw tiles inside a Rayon pool of `threads`
/// workers. A fresh sink is constructed per run so the running write cursor
/// always starts at 0.
fn bench_compress(model: &StorageModel, tiles: &[Vec<u8>], threads: usize) -> Duration {
    let pool = ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .unwrap();
    pool.install(|| {
        let plan = plan_tiff_write(model).unwrap();
        let mut w = MemoryBinaryWriter::new();
        write_tiff_header(&mut w, 8, false).unwrap();
        write_tiff_ifd(&mut w, plan.entries.clone(), false, 0).unwrap();
        let mut sink = TiffImageSink::new(&mut w, plan.directory.clone());
        let start = Instant::now();
        sink.write_compressed_tiles_parallel(tiles).unwrap();
        let t = start.elapsed();
        std::hint::black_box(t);
        t
    })
}

/// Median of ascending `[Duration;]` (the same methodology as the rest of the
/// benchmark suite: sort → middle).
fn median(mut samples: Vec<Duration>) -> Duration {
    samples.sort_unstable();
    samples[samples.len() / 2]
}

fn bench_axis(name: &str, f: &dyn Fn(usize) -> Duration) -> Vec<(usize, Duration)> {
    println!("  {name}:");
    let mut rows = Vec::new();
    for &threads in &THREADS {
        // Warm up.
        for _ in 0..WARMUP {
            f(threads);
        }
        let mut samples = Vec::with_capacity(REPEATS);
        for _ in 0..REPEATS {
            samples.push(f(threads));
        }
        let med = median(samples);
        println!(
            "    {threads:>2} threads: {:.2} ms",
            med.as_secs_f64() * 1e3
        );
        rows.push((threads, med));
    }
    rows
}

fn main() {
    let mut out_path: Option<String> = None;
    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--out" && i + 1 < args.len() {
            out_path = Some(args[i + 1].clone());
            i += 2;
        } else {
            i += 1;
        }
    }

    // A 4k-ish image: 256 cols x 128 rows of 16x16 tiles = 32,768 tiles.
    // This is large enough that per-tile CPU work dominates pool overhead.
    let (width, height) = (4096u32, 2048u32);

    let payload = payload_for(width, height);

    // --- Setup: a compressed LZW file and its raw tile set. ---
    let lzw_model = tiled_model(width, height, "LZW", "HorizontalDifferencing");
    println!("building compressed LZW fixture ({width}x{height})...");
    let (lzw_bytes, cols, rows) = build_compressed_file(&lzw_model, &payload);

    let mut lzw_tiles: Vec<Vec<u8>> = Vec::with_capacity((cols * rows) as usize);
    let tile_bytes = (TILE * TILE) as usize;
    for start in payload.chunks(tile_bytes) {
        lzw_tiles.push(start.to_vec());
    }

    let deflate_model = tiled_model(width, height, "Deflate", "None");
    let deflate_tiles = lzw_tiles.clone();

    println!("== LZW compress (H8) ==");
    let lzw_compress = bench_axis("lzw_compress", &|t| {
        bench_compress(&lzw_model, &lzw_tiles, t)
    });

    println!("== Deflate compress (H8) ==");
    let deflate_compress = bench_axis("deflate_compress", &|t| {
        bench_compress(&deflate_model, &deflate_tiles, t)
    });

    println!("== LZW decompress (H7) ==");
    let lzw_decompress = bench_axis("lzw_decompress", &|t| bench_decompress(&lzw_bytes, t));

    // --- JSON result ---
    // Each metric reports per-thread medians plus `speedup_vs_1_thread`, which
    // directly expresses hypotheses H7 (decompression scaling) and H8
    // (compression scaling) — e.g. 8-thread speedup ≈ 6-7× for LZW.
    let to_json = |rows: &[(usize, Duration)]| -> serde_json::Value {
        let base = rows[0].1;
        serde_json::json!(rows
            .iter()
            .map(|(threads, d)| json!({
                "threads": threads,
                "median_ms": d.as_secs_f64() * 1e3,
                "median_us": d.as_secs_f64() * 1e6,
                "speedup_vs_1_thread": base.as_secs_f64() / d.as_secs_f64(),
            }))
            .collect::<Vec<_>>())
    };

    let doc = serde_json::json!({
        "language": "rust-parallel",
        "bench_version": "0.1.0",
        "targets": ["ptiff-core tiff-backend + tiff-codecs + parallel (rayon)"],
        "image": {"width": width, "height": height, "tiles": (cols * rows)},
        "threads_tested": THREADS,
        "repeats": REPEATS,
        "warmup": WARMUP,
        "metrics": {
            "lzw_compress": to_json(&lzw_compress),
            "deflate_compress": to_json(&deflate_compress),
            "lzw_decompress": to_json(&lzw_decompress),
        },
    });

    if let Some(out) = &out_path {
        if let Some(parent) = std::path::PathBuf::from(out).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(out, serde_json::to_string_pretty(&doc).unwrap() + "\n").unwrap();
        println!("wrote {out}");
    }
}
