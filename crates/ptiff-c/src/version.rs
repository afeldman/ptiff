//! C-ABI version surface (emitted into `target/ptiff_c.h` by cbindgen from this crate).
//!
//! PTIFF's ABI philosophy (plan §7.2): `compile_time_version()` and
//! `runtime_version()` let a consumer detect an ABI/library mismatch early.
//! The Rust core exposes both as the compile-time, runtime-queryable
//! [`ptiff::APP_VERSION`] / [`ptiff::VERSION_STR`]; this bridge marshals them
//! into the fixed C struct and the out-parameter variants (the latter exist so
//! runtimes without C struct-by-value return, e.g. Ruby Fiddle, can read the
//! version without an aggregate return).

use ptiff::APP_VERSION;

/// The ABI version of `libptiff_c`, exported into the generated header as the
/// `#define PTIFF_ABI_VERSION` preprocessor constant (plan §7.5).
///
/// Unlike the semantic crate version (1.0.1, carried by `ptiff_version` /
/// `ptiff_compile_time_version`), this is a **monotonic break counter**: it
/// increments by one on every *breaking* C-ABI change (a removed or
/// reordered symbol, a struct-layout change, a changed parameter/return
/// contract). Pure additions (new functions/structs) bump the minor SemVer
/// but leave `PTIFF_ABI_VERSION` unchanged, so a foreign runtime can pre-check
/// the two with a single `#if PTIFF_ABI_VERSION < n` guard without knowing the
/// crate's exact release cadence.
///
/// The ABI starts at 1 (initial stable surface over the 1.0.1 Rust core).
pub const PTIFF_ABI_VERSION: u32 = 1;

/// Mirror of the C `ptiff_version` struct (see `ptiff_version.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ptiff_version {
    /// Major ABI-visible version.
    pub major: i32,
    /// Minor ABI-visible version.
    pub minor: i32,
    /// Patch ABI-visible version.
    pub patch: i32,
}

/// Returns the version the library was compiled and linked against.
#[no_mangle]
pub extern "C" fn ptiff_compile_time_version() -> ptiff_version {
    ptiff_version {
        major: APP_VERSION.major as i32,
        minor: APP_VERSION.minor as i32,
        patch: APP_VERSION.patch as i32,
    }
}

/// Returns the version of the loaded library at runtime.
///
/// The Rust core is statically linked into `libptiff_c`, so the runtime and
/// compile-time version always agree; both are kept so the `libptiff_c`
/// contract of pairing them for ABI-mismatch detection is preserved verbatim.
#[no_mangle]
pub extern "C" fn ptiff_runtime_version() -> ptiff_version {
    ptiff_compile_time_version()
}

/// Out-parameter variant of [`ptiff_compile_time_version`]. Either pair of
/// pointers may be `NULL`.
#[no_mangle]
pub extern "C" fn ptiff_compile_time_version_out(
    major: *mut i32,
    minor: *mut i32,
    patch: *mut i32,
) {
    let v = ptiff_compile_time_version();
    // Safety: a NULL out-param is explicitly allowed; a non-NULL pointer is
    // trusted to point to writable storage of at least one `i32` (the C header
    // documents this contract). The values are plain integers with no
    // aliasing, so the writes cannot violate the caller's memory model.
    unsafe {
        if !major.is_null() {
            *major = v.major;
        }
        if !minor.is_null() {
            *minor = v.minor;
        }
        if !patch.is_null() {
            *patch = v.patch;
        }
    }
}

/// Out-parameter variant of [`ptiff_runtime_version`].
#[no_mangle]
pub extern "C" fn ptiff_runtime_version_out(major: *mut i32, minor: *mut i32, patch: *mut i32) {
    ptiff_compile_time_version_out(major, minor, patch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ptiff::VERSION_STR;

    #[test]
    fn version_matches_cargo_and_semver() {
        // APP_VERSION is parsed from Cargo.toml; VERSION_STR mirrors it. The
        // C struct must reflect both.
        assert_eq!(VERSION_STR, env!("CARGO_PKG_VERSION"));
        let v = ptiff_runtime_version();
        assert_eq!(
            format!("{}.{}.{}", v.major, v.minor, v.patch),
            VERSION_STR,
            "C version struct mirrors the crate semantic version"
        );
    }

    #[test]
    fn abi_version_is_initial_and_positive() {
        // §7.5: PTIFF_ABI_VERSION is a monotonic break counter, distinct from
        // the crate SemVer. It must be >= 1 from the first stable ABI onwards.
        // These are compile-time constants, so check them in `const` blocks:
        // clippy::assertions_on_constants flags them as constant-value asserts.
        const { assert!(PTIFF_ABI_VERSION >= 1, "ABI version starts at 1") };
        // Stable surface is contract-pinned by the cbindgen-exported header;
        // the value is also a `#define` in target/ptiff_c.h (asserted by the
        // header-level test in the C-ABI test suite).
        const { assert!(PTIFF_ABI_VERSION == 1) };
    }

    #[test]
    fn out_variant_matches_by_value() {
        let direct = ptiff_runtime_version();
        let mut major = -1;
        let mut minor = -1;
        let mut patch = -1;
        // The pointers are non-null and valid for one `i32` each.
        ptiff_runtime_version_out(&mut major, &mut minor, &mut patch);
        assert_eq!(major, direct.major);
        assert_eq!(minor, direct.minor);
        assert_eq!(patch, direct.patch);
    }

    #[test]
    fn out_variant_accepts_null() {
        // The header documents that any out pointer may be NULL; the call must
        // not panic on a NULL pointer. All pointers are NULL, explicitly allowed.
        ptiff_compile_time_version_out(
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        ptiff_runtime_version_out(
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
    }
}
