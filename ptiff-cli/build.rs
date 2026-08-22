//! Emits link-search + rpath flags for the final `ptiff` binary so the dynamic
//! loader can find `libptiff_c` (and its transitive `libptiff`) at runtime.
//!
//! Path-dependency `cargo:rustc-link-arg` flags from the `ptiff` binding crate
//! do not propagate to the CLI binary, so we resolve the library here too and
//! embed the rpath into the final executable. Uses the same resolution order as
//! `bindings/rust/build.rs`: `PTIFF_C_LIB_DIR` (dev/CI) then pkg-config
//! (`libptiff_c.pc`) for installed prefixes.

use std::collections::HashSet;
use std::env;
use std::path::{Path, PathBuf};

fn main() {
    // ---- Explicit env override (development / CI) -------------------------
    if let Some(dir) = env::var_os("PTIFF_C_LIB_DIR") {
        let dir = PathBuf::from(dir);
        println!("cargo:rustc-link-search=native={}", dir.display());
        let mut emitted = HashSet::new();
        link_rpath(&dir, &mut emitted);
        if let Some(ptiff_dir) = env::var_os("PTIFF_LIB_DIR") {
            let pd = PathBuf::from(ptiff_dir);
            if pd != dir {
                println!("cargo:rustc-link-search=native={}", pd.display());
            }
            link_rpath(&pd, &mut emitted);
            println!("cargo:rustc-link-lib=ptiff");
        }
        println!("cargo:rustc-link-lib=ptiff_c");
        return;
    }

    // ---- pkg-config (installed library) -----------------------------------
    let library = match pkg_config::Config::new().probe("libptiff_c") {
        Ok(lib) => lib,
        Err(e) => {
            panic!(
                "could not locate libptiff_c. Set PTIFF_C_LIB_DIR (=<build>/bindings/c) or \
                 run `cmake --install` with a prefix and point PKG_CONFIG_PATH at its \
                 lib/pkgconfig. pkg-config error: {e}"
            );
        }
    };

    // The binding already links the libs; here we only need the rpath entries so
    // the final executable can resolve the shared libraries at runtime.
    let mut emitted = HashSet::new();
    for path in &library.link_paths {
        link_rpath(path, &mut emitted);
    }
}

/// Emits at most one rpath entry per directory (on macOS/Linux) so the final
/// binary can find the shared library at runtime.
fn link_rpath(dir: &Path, emitted: &mut HashSet<PathBuf>) {
    if let Ok(sysroot) = env::var("CARGO_CFG_TARGET_OS") {
        if (sysroot == "macos" || sysroot == "linux") && emitted.insert(dir.to_owned()) {
            println!("cargo:rustc-link-arg=-Wl,-rpath,{}", dir.display());
        }
    }
}
