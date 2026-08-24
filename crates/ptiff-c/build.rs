//! Build script for `ptiff-c`.
//!
//! Generates the public C header `ptiff_c.h` from this crate's `extern "C"`
//! declarations via cbindgen (the ABI's source of truth, plan §7.4). The
//! header is emitted to `OUT_DIR` and a copy is placed at the workspace
//! `target/ptiff_c.h` (a stable path) so the C-ABI test suite, SWIG and FFI
//! consumers can include it regardless of the per-build `OUT_DIR` hash.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    // Rebuild the header whenever a source file changes.
    println!("cargo:rerun-if-changed=cbindgen.toml");
    println!("cargo:rerun-if-changed=src");

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let config = cbindgen::Config::from_file(manifest_dir.join("cbindgen.toml"))
        .expect("failed to parse cbindgen.toml");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let header_out = out_dir.join("ptiff_c.h");

    cbindgen::Builder::new()
        .with_crate(&manifest_dir)
        .with_config(config)
        .generate()
        .expect("cbindgen failed to generate ptiff_c.h")
        .write_to_file(&header_out);

    // Stable copy of the header at <target-dir>/ptiff_c.h so consumers (the C
    // ABI test suite, SWIG, the CMake packaging install and any isolated/nix
    // build) can include it from a deterministic path without knowing the
    // per-build OUT_DIR hash. The target dir is CARGO_TARGET_DIR when set;
    // otherwise the default <workspace-root>/target/ (the dir two levels up
    // from CARGO_MANIFEST_DIR: crates/ptiff-c -> crates -> <root>).
    let stable_dir = match env::var("CARGO_TARGET_DIR") {
        Ok(dir) => PathBuf::from(dir),
        Err(_) => {
            let root = manifest_dir
                .parent()
                .and_then(Path::parent)
                .expect("workspace root");
            root.join("target")
        }
    };
    fs::create_dir_all(&stable_dir).expect("create stable header dir");
    let stable_header = stable_dir.join("ptiff_c.h");
    fs::copy(&header_out, &stable_header).expect("copy ptiff_c.h to stable path");

    println!("cargo:rustc-env=PTIFF_C_HEADER={}", stable_header.display());
    println!("cargo:rustc-env=OUT_DIR={}", out_dir.display());
    println!(
        "cargo:warning=generated C header at {}",
        stable_header.display()
    );
}
