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
