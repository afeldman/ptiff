//! Build script for `ptiff-c`.
//!
//! Generates the public C header `ptiff_c.h` from this crate's `extern "C"`
//! declarations via cbindgen (the ABI's source of truth, plan §7.4). The
//! header lands in `OUT_DIR` and is re-emitted on every `cargo build`, so FFI
//! consumers (SWIG, the C-ABI test suite, CMake) can consume it straight out
//! of the cargo target dir.

use std::env;
use std::path::PathBuf;

fn main() {
    // Rebuild the header whenever a source file changes.
    println!("cargo:rerun-if-changed=cbindgen.toml");
    println!("cargo:rerun-if-changed=src");

    let crate_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");

    let config = cbindgen::Config::from_file("cbindgen.toml")
        .expect("failed to parse cbindgen.toml");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let header_path = out_dir.join("ptiff_c.h");

    cbindgen::Builder::new()
        .with_crate(crate_dir)
        .with_config(config)
        .generate()
        .expect("cbindgen failed to generate ptiff_c.h")
        .write_to_file(&header_path);

    // Emit the location so the rest of the build (tests, docs) can find it and
    // so downstream build scripts can rely on `DEP`-style location via the env.
    println!("cargo:rustc-env=PTIFF_C_HEADER={}", header_path.display());
    println!("cargo:warning=generated C header at {}", header_path.display());
}
