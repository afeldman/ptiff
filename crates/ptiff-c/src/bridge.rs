//! Minimal C ABI bridge (emitted into `target/ptiff_c.h` by cbindgen from this crate).
//!
//! Provides the process-wide [`ptiff_backend_names`] (backed by the core's
//! [`ptiff::BackendFactory`]) and the uniform C-string deallocator
//! [`ptiff_free_string`] every foreign runtime uses to release strings handed
//! across the boundary.
//!
//! C strings are allocated as `Vec<u8>` of capacity `len + 1` whose allocation
//! is leaked to the caller; [`ptiff_free_string`] is the symmetric deallocator
//! that reconstructs and drops that `Vec`. Ownership is thus unambiguous: the
//! callee allocates, the caller returns it with `ptiff_free_string`.

use ptiff::BackendFactory;
use std::os::raw::c_char;

/// Returns a newly allocated, comma-space-joined list of registered backend
/// names — or `NULL` if none are registered. The caller releases the string
/// with [`ptiff_free_string`].
///
/// # Safety
///
/// The returned pointer is a heap allocation owned by the caller; it must be
/// freed exactly once with [`ptiff_free_string`].
#[no_mangle]
pub extern "C" fn ptiff_backend_names() -> *mut c_char {
    let names = BackendFactory::instance().registered_backends();
    let joined = names.join(", ");
    if joined.is_empty() {
        return std::ptr::null_mut();
    }
    alloc_c_string(joined.as_bytes())
}

/// Frees a string previously returned by [`ptiff_backend_names`]. `NULL` is a
/// no-op.
///
/// # Safety
///
/// `s` must be null or a pointer previously returned by
/// [`ptiff_backend_names`], freed exactly once.
#[no_mangle]
pub extern "C" fn ptiff_free_string(s: *mut c_char) {
    if !s.is_null() {
        // Safety: `s` was heap-allocated with `Vec` capacity `len + 1` in
        // `alloc_c_string` (see below); `from_raw_parts` is the symmetric free.
        unsafe {
            let len = std::ffi::CStr::from_ptr(s).to_bytes().len();
            drop(Vec::from_raw_parts(s, len, len + 1));
        }
    }
}

/// Allocates a NUL-terminated C string from a byte slice using a `Vec` with
/// capacity `len + 1`. The returned pointer is freed with [`ptiff_free_string`].
fn alloc_c_string(bytes: &[u8]) -> *mut c_char {
    let mut v: Vec<u8> = Vec::with_capacity(bytes.len() + 1);
    v.extend_from_slice(bytes);
    v.push(0);
    // Leak the allocation: the pointer outlives this call by design; the
    // capacity `len + 1` is reclaimed by `from_raw_parts` in `free_string`.
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p as *mut c_char
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_names_returns_a_nonempty_comma_list() {
        // The singleton factory self-registers at least the memory and TIFF
        // backends, so the joined list is non-empty and NULL-free.
        let ptr = ptiff_backend_names();
        assert!(!ptr.is_null());
        // Safety: ptr is a valid, NUL-terminated C string owned by us.
        let s = unsafe { std::ffi::CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned();
        assert!(!s.is_empty());
        // sanity: registered backends come comma-separated
        let _ = s.contains(',');
        ptiff_free_string(ptr);
    }

    #[test]
    fn free_string_accepts_null() {
        // Null is a documented no-op.
        ptiff_free_string(std::ptr::null_mut());
    }

    #[test]
    fn allocc_string_round_trips_and_frees() {
        let p = alloc_c_string(b"a, b, c");
        // Safety: p is NUL-terminated and owned by us.
        let s = unsafe { std::ffi::CStr::from_ptr(p) }
            .to_string_lossy()
            .into_owned();
        assert_eq!(s, "a, b, c");
        // Free exactly once.
        ptiff_free_string(p);
    }
}
