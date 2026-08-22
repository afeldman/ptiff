//! Locates `libptiff_c` (the language-agnostic C ABI), emits the link flags,
//! and generates the raw FFI declarations with `bindgen` straight from the
//! bridge headers.
//!
//! Resolution order:
//!   1. `PTIFF_C_LIB_DIR` env var (explicit; e.g. a CMake/Conan build output or
//!      an install prefix). Kept for in-tree dev/CI workflows that build from
//!      source without installing. `PTIFF_C_INCLUDE` (optional) points at the
//!      directory containing the `ptiff_*.h` headers; defaults to `bindings/c`
//!      relative to the crate root.
//!   2. pkg-config lookup for `libptiff_c` (the standard, layout-agnostic path
//!      for consuming an *installed* C/C++ library). Set `PKG_CONFIG_PATH` to the
//!      `lib/pkgconfig` dir of an install prefix if it is not on the default path.
//!      The installed `*.h` are used for bindgen (from the pkg-config include dirs).
//!
//! The pkg-config file (see `bindings/c/libptiff_c.pc.in`) carries the include
//! dirs, the `-lptiff_c`, and (via `Requires.private`) the transitive `-lptiff`.
//! Because the .pc is relocatable, this works against any install prefix.

use std::collections::HashSet;
use std::env;
use std::path::{Path, PathBuf};

fn main() {
    let out_dir = env::var_os("OUT_DIR").expect("OUT_DIR set by cargo");

    // ---- Explicit env override (development / CI) -------------------------
    if let Some(dir) = env::var_os("PTIFF_C_LIB_DIR") {
        let bindir = PathBuf::from(&dir);
        println!("cargo:rustc-link-search=native={}", bindir.display());
        let mut emitted = HashSet::new();
        link_rpath(&bindir, &mut emitted);
        // Transitive libptiff for shared builds that keep backend registration
        // alive (and for static builds where -lptiff is required).
        if let Some(ptiff_dir) = env::var_os("PTIFF_LIB_DIR") {
            let pd = PathBuf::from(ptiff_dir);
            if pd != bindir {
                println!("cargo:rustc-link-search=native={}", pd.display());
            }
            link_rpath(&pd, &mut emitted);
            println!("cargo:rustc-link-lib=ptiff");
        }
        println!("cargo:rustc-link-lib=ptiff_c");

        // Header dir for bindgen: PTIFF_C_INCLUDE override, else repo bindings/c.
        let inc_dir = match env::var_os("PTIFF_C_INCLUDE") {
            Some(i) => PathBuf::from(i),
            None => Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bindings/c"),
        };
        let gen_path = generate_bindgen(&inc_dir, Path::new(&out_dir));
        for h in bridge_headers() {
            println!("cargo:rerun-if-changed={}", inc_dir.join(h).display());
        }
        println!("cargo:rerun-if-changed={gen_path}");
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

    // Link libs reported by pkg-config (libptiff_c and, for --static, libptiff).
    for lib in &library.libs {
        println!("cargo:rustc-link-lib={lib}");
    }
    // rpath: make the dynamic loader find the shared libs at runtime.
    let mut emitted = HashSet::new();
    for path in &library.link_paths {
        link_rpath(path, &mut emitted);
    }
    // Bindgen from the installed headers (first include_path that has them).
    let inc_dir = library
        .include_paths
        .iter()
        .find(|p| p.join("ptiff_pixel_bridge.h").exists())
        .cloned()
        .unwrap_or_else(|| {
            panic!(
                "pkg-config include_paths do not contain the bridge headers: {:?}",
                library.include_paths
            )
        });
    let gen_path = generate_bindgen(&inc_dir, Path::new(&out_dir));
    for h in bridge_headers() {
        println!("cargo:rerun-if-changed={}", inc_dir.join(h).display());
    }
    println!("cargo:rerun-if-changed={gen_path}");
}

/// All public bridge headers (see `bindings/c`). bindgen aggregates these into
/// one translation unit; `rerun-if-changed` covers the same set so a
/// headers-only change re-runs bindgen.
fn bridge_headers() -> [&'static str; 8] {
    [
        "ptiff_bridge.h",
        "ptiff_version.h",
        "ptiff_logger.h",
        "ptiff_error.h",
        "ptiff_camera.h",
        "ptiff_image_bridge.h",
        "ptiff_metadata.h",
        "ptiff_pixel_bridge.h",
    ]
}

/// Runs `bindgen` over the aggregate bridge header and writes `ffi.rs`.
fn generate_bindgen(inc: &Path, out_dir: &Path) -> String {
    // Help bindgen locate libclang when it is not on the default search path
    // (e.g. a Homebrew llvm keg).
    if env::var_os("LIBCLANG_PATH").is_none() {
        for cand in [
            "/opt/homebrew/opt/llvm/lib",
            "/usr/local/opt/llvm/lib",
            "/usr/lib/llvm/lib",
        ] {
            if Path::new(cand).join("libclang.dylib").exists()
                || Path::new(cand).join("libclang.so").exists()
            {
                env::set_var("LIBCLANG_PATH", cand);
                break;
            }
        }
    }

    // Aggregate header so bindgen sees the full ABI in one translation unit.
    let aggregate = bridge_headers()
        .iter()
        .map(|h| format!("#include \"{h}\""))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let bindings = bindgen::Builder::default()
        .header_contents("ptiff_abi.h", &aggregate)
        .clang_arg(format!("-I{}", inc.display()))
        // Keep the public surface we actually bind; drop libc/stdint noise and
        // the export macro.
        .allowlist_var("PTIFF_.*")
        .allowlist_type("ptiff_.*")
        .allowlist_function("ptiff_.*")
        // Opaque handles: no definition leaked.
        .opaque_type("ptiff_image")
        .opaque_type("ptiff_source")
        .opaque_type("ptiff_sink")
        // The e_port macro expands to an attribute; drop it entirely.
        .blocklist_item("PTIFF_C_API")
        // Generates derives for plain C structs so builders can rely on
        // `PartialEq`/`Default` (e.g. Option<TileInfo> in ImageDescriptorBuilder).
        .derive_default(true)
        .derive_partialeq(true)
        .derive_eq(true)
        .generate()
        .expect("failed to generate libptiff FFI bindings");

    let dest = out_dir.join("ptiff_ffi.rs");
    bindings
        .write_to_file(&dest)
        .expect("failed to write ptiff_ffi.rs");
    dest.display().to_string()
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
