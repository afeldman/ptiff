//! Metadata C ABI (`bindings/c/ptiff_metadata.h`).
//!
//! [`ptiff_open_path`] opens a TIFF/BigTIFF file read-only and fills a
//! [`crate::types::ptiff_image_descriptor`] with the first image's metadata
//! (the same fields `ptiff_source_descriptor` exposes, without retaining a
//! handle).
//!
//! [`ptiff_open_path_fields`] / [`ptiff_fields_free`] expose the flattened
//! `ptiff.<domain>.<key>` extension fields. That flat field view is a **later
//! slice**: the Rust core models the extension domains structurally (Camera /
//! CRS) rather than as a string-keyed field map, so there is no lossless
//! `ptiff.*` field iteration to marshal yet. The `_fields` entry point is
//! recognised-but-unimplemented for now (returning `PTIFF_ERROR_NOT_IMPLEMENTED`),
//! keeping the ABI symbol surface complete.

use crate::error::{ptiff_error_code, to_c_error};
use crate::types::{image_to_c, ptiff_image_descriptor};
use ptiff::{Error, Tiff};
use std::os::raw::c_char;

/// Opens the TIFF/BigTIFF file at `path` (read-only) and fills `desc` with the
/// first image's metadata. Returns 0 on success, a negative ptiff error code on
/// failure (and leaves `*desc` untouched). `path` and `desc` must be non-null.
///
/// # Safety
///
/// `path` must be a valid NUL-terminated C string; `desc` must point to
/// writable storage of at least `sizeof(ptiff_image_descriptor)`.
#[no_mangle]
pub extern "C" fn ptiff_open_path(path: *const c_char, desc: *mut ptiff_image_descriptor) -> i32 {
    if path.is_null() || desc.is_null() {
        return -(ptiff_error_code::PTIFF_ERROR_INVALID_ARGUMENT as i32);
    }
    // Safety: validated non-null above; C string is NUL-terminated per header.
    let path_s = unsafe { std::ffi::CStr::from_ptr(path) }
        .to_string_lossy()
        .into_owned();
    let result = (|| -> Result<ptiff_image_descriptor, ptiff::Error> {
        let tiff = Tiff::open(&path_s)?;
        let scene = tiff.scene();
        if scene.image_count() == 0 {
            return Err(Error::not_found("ptiff_open_path: file has no images"));
        }
        let image = scene.image_at(0)?;
        Ok(image_to_c(image))
    })();
    match result {
        Ok(d) => {
            // Safety: `desc` is a writable pointer of at least the struct size.
            unsafe { *desc = d };
            0
        }
        Err(e) => to_c_error(&e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_path_or_desc_is_invalid_argument() {
        assert_eq!(
            ptiff_open_path(std::ptr::null(), std::ptr::null_mut()),
            -(ptiff_error_code::PTIFF_ERROR_INVALID_ARGUMENT as i32)
        );
    }

    #[test]
    fn null_path_with_valid_desc_is_invalid_argument() {
        let mut d = ptiff_image_descriptor::c_default();
        assert_eq!(
            // Safety: null path + valid desc; path is checked first.
            ptiff_open_path(std::ptr::null(), &mut d as *mut ptiff_image_descriptor),
            -(ptiff_error_code::PTIFF_ERROR_INVALID_ARGUMENT as i32)
        );
    }

    #[test]
    fn open_path_reports_error_for_missing_file() {
        let mut d = ptiff_image_descriptor::c_default();
        let path = std::ffi::CString::new("/nonexistent/definitely_missing.tif").unwrap();
        // Safety: valid path + valid desc; the file does not exist.
        let rc = ptiff_open_path(path.as_ptr(), &mut d as *mut ptiff_image_descriptor);
        assert!(
            rc < 0,
            "opening a missing file must fail with a negative code"
        );
    }
}
