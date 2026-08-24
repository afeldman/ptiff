//! Rust benchmarks over the ptiff binding, mirroring the Python/Ruby/Octave/Go
//! suite (benchmarks/src/bench_*.py|rb|m|go).
//!
//! This benchmark reports:
//!
//!   * write_all_tiles_ms   -- create 128x128 UInt8 tiled TIFF, write with pixels
//!   * read_metadata_*_ms   -- `ptiff::Tiff::open` on each fixture (loads and
//!                             parses the file; per-tile pixel decode is not
//!                             exercised so the cost is open/IFD-parse only)
//!
//! Usage (from benchmarks/):
//!   cargo run --release --manifest-path rust_bench/Cargo.toml -- \
//!       --out benchmark-results/rust.json

use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use ptiff::{ImageDescriptorBuilder, PixelType, Scene, Tiff};

fn repeats() -> usize {
    env::var("BENCH_REPEATS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(20)
}

fn iters() -> usize {
    env::var("BENCH_ITERS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(50)
}

fn median_of(mut samples: Vec<f64>) -> (f64, f64, f64) {
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = samples.len();
    let med = samples[n / 2];
    (med, samples[0], samples[n - 1])
}

fn round4(v: f64) -> f64 {
    (v * 10000.0).round() / 10000.0
}

fn stat(samples: Vec<f64>, n: usize) -> serde_json::Value {
    let (med, mn, mx) = median_of(samples);
    serde_json::json!({
        "repeats": n,
        "median_ms": round4(med),
        "min_ms": round4(mn),
        "max_ms": round4(mx),
    })
}

fn write_all_tiles(out: &str, count: usize) {
    let desc = ImageDescriptorBuilder::new(128, 128)
        .pixel_type(PixelType::UInt8)
        .channel_count(1)
        .tile(64, 64)
        .build();
    let mut scene = Scene::new();
    scene.add_image(desc).expect("add_image failed");
    // 128x128 single-channel UInt8 raster filled with a constant pattern.
    let pixels = vec![7u8; 128 * 128];
    for _ in 0..count {
        let bytes = Tiff::to_bytes_with_pixels(&scene, &[&pixels])
            .expect("to_bytes_with_pixels failed");
        fs::write(out, &bytes).expect("write file failed");
    }
}

fn open_meta(path: &str) {
    Tiff::open(path).expect("Tiff::open failed");
}

fn time_repeats<F: FnMut()>(mut f: F, n: usize) -> Vec<f64> {
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let t0 = Instant::now();
        f();
        out.push(t0.elapsed().as_secs_f64() * 1000.0);
    }
    out
}

fn fixtures_dir() -> PathBuf {
    let cwd = env::current_dir().expect("cwd");
    for cand in [
        cwd.join("fixtures"),
        cwd.join("benchmarks").join("fixtures"),
        cwd.join("..").join("fixtures"),
    ] {
        if cand.is_dir() {
            return cand;
        }
    }
    panic!("cannot locate benchmarks/fixtures from {}", cwd.display());
}

fn fixture_info(dir: &PathBuf, name: &str) -> serde_json::Value {
    let p = dir.join(name);
    let size = fs::metadata(&p).map(|m| m.len() as i64).unwrap_or(-1);
    serde_json::json!({
        "file": p.display().to_string(),
        "size_bytes": size,
        "exists": size >= 0,
    })
}

fn main() {
    let mut out_path = String::new();
    let mut use_real = false;
    let mut use_nac = false;
    let args: Vec<String> = env::args().collect();
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--out" {
            i += 1;
            if i < args.len() {
                out_path = args[i].clone();
            }
        } else if args[i] == "--real" {
            use_real = true;
        } else if args[i] == "--nac" {
            use_nac = true;
        }
        i += 1;
    }
    if out_path.is_empty() {
        eprintln!("missing --out <json>");
        std::process::exit(2);
    }

    let rep = repeats();
    let it = iters();
    let fix = fixtures_dir();

    // warm-up
    let warmup_tif = env::temp_dir().join("ptiff_rust_warmup.tif");
    let warmup_tif = warmup_tif.to_str().unwrap().to_string();
    write_all_tiles(&warmup_tif, 1);
    let _ = fs::remove_file(&warmup_tif);

    let write_tif = env::temp_dir().join("ptiff_rust_write.tif");
    let write_tif = write_tif.to_str().unwrap().to_string();
    let write_samples = time_repeats(|| write_all_tiles(&write_tif, it), rep);
    let _ = fs::remove_file(&write_tif);

    let u8_128 = fix.join("uint8_128.tif");
    let u8_512 = fix.join("uint8_512.tif");
    let f32_512 = fix.join("f32_512.tif");

    let r128 = time_repeats(|| open_meta(u8_128.to_str().unwrap()), rep);
    let r512 = time_repeats(|| open_meta(u8_512.to_str().unwrap()), rep);
    let rf32 = time_repeats(|| open_meta(f32_512.to_str().unwrap()), rep);

    let mut fixtures = serde_json::json!({
        "uint8_128": fixture_info(&fix, "uint8_128.tif"),
        "uint8_512": fixture_info(&fix, "uint8_512.tif"),
        "f32_512": fixture_info(&fix, "f32_512.tif"),
    });
    let mut metrics = serde_json::json!({
        "write_all_tiles_ms": stat(write_samples, rep),
        "read_metadata_uint8_128_ms": stat(r128, rep),
        "read_metadata_uint8_512_ms": stat(r512, rep),
        "read_metadata_f32_512_ms": stat(rf32, rep),
    });

    // --real: metadata-open of a real NASA LOLA elevation crop (copied into
    // benchmarks/fixtures by run_benchmarks.sh --real). The Rust binding has no
    // per-tile pixel read, so this is a metadata-only open like the other reads.
    if use_real {
        let real_tif = fix.join("real_lola_512.tif");
        if real_tif.exists() {
            let rreal = time_repeats(|| open_meta(real_tif.to_str().unwrap()), rep);
            metrics["read_metadata_real_lola_512_ms"] = stat(rreal, rep);
            fixtures["real_lola_512"] = fixture_info(&fix, "real_lola_512.tif");
        } else {
            eprintln!("[rust] --real requested but real_lola_512.tif missing; skipping");
        }
    }

    // --nac: metadata-open of the full real NASA LRO-NAC DTM (2693x14236,
    // 616 tiles, ~55 MB). This exercises a large, real-world IFD parse -- the
    // largest file in the suite. Metadata-only, like the other Rust reads.
    if use_nac {
        let nac_tif = fix.join("nac_dtm.tif");
        if nac_tif.exists() {
            let rnac = time_repeats(|| open_meta(nac_tif.to_str().unwrap()), rep);
            metrics["read_metadata_nac_ms"] = stat(rnac, rep);
            fixtures["nac_dtm"] = fixture_info(&fix, "nac_dtm.tif");
        } else {
            eprintln!("[rust] --nac requested but nac_dtm.tif missing; skipping");
        }
    }

    let v = ptiff::VERSION_STR;
    let doc = serde_json::json!({
        "language": "rust",
        "binding_version": v,
        "repeats": rep,
        "iterations_per_sample": it,
        "read_is_metadata_only": true,
        "write_image": {"width": 128, "height": 128, "pixel_type": "uint8", "tile": 64},
        "fixtures": fixtures,
        "metrics": metrics,
    });

    if let Some(parent) = PathBuf::from(&out_path).parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::write(
        &out_path,
        serde_json::to_string_pretty(&doc).unwrap() + "\n",
    )
    .unwrap();
    println!(
        "{}",
        serde_json::json!({"language": "rust", "metrics": metrics})
    );
}
