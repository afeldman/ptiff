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
use crate::types::{apply_layout_tile_info, image_to_c, ptiff_image_descriptor};
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
        let mut descriptor = image_to_c(image);
        let layout = tiff.tile_layout(0)?;
        apply_layout_tile_info(&layout, &mut descriptor);
        Ok(descriptor)
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
        // A missing path surfaces as NOT_FOUND (matching the C++ oracle /
        // ptiff_error.h semantics), not an invalid-stream code.
        assert_eq!(rc, -(ptiff_error_code::PTIFF_ERROR_NOT_FOUND as i32));
    }

    #[test]
    fn open_path_reports_tile_info_for_tiled_file() {
        // A tiled file must round-trip its tile info through ptiff_open_path,
        // the same re-derivation from the layout that ptiff_source_descriptor
        // does (C++ oracle parity; the core leaves tile_info unset).
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("meta.tif");
        let path_s = path.to_str().unwrap().to_string();

        // Use the pixel-bridge sink to produce a small tiled file.
        let mut d = ptiff_image_descriptor::c_default();
        d.width = 64;
        d.height = 48;
        d.pixel_type = crate::types::ptiff_pixel_type::PTIFF_PIXEL_UINT8 as i32;
        d.channel_count = 1;
        d.has_tile_info = 1;
        d.tile_info.tile_width = 16;
        d.tile_info.tile_height = 16;
        let sink = crate::pixel_bridge::ptiff_sink_create(
            std::ffi::CString::new(path_s.clone()).unwrap().as_ptr(),
            &d as *const ptiff_image_descriptor,
        );
        assert!(!sink.is_null());
        let cols = crate::pixel_bridge::ptiff_sink_tile_columns(sink);
        let rows = crate::pixel_bridge::ptiff_sink_tile_rows(sink);
        let n = crate::pixel_bridge::ptiff_sink_tile_byte_size(sink);
        let buf = vec![0u8; n];
        for row in 0..rows {
            for col in 0..cols {
                assert_eq!(
                    crate::pixel_bridge::ptiff_sink_write_tile(
                        sink,
                        col,
                        row,
                        buf.as_ptr(),
                        buf.len()
                    ),
                    0
                );
            }
        }
        crate::pixel_bridge::ptiff_sink_close(sink);

        // Read the metadata descriptor back.
        let mut out = ptiff_image_descriptor::c_default();
        let rc = ptiff_open_path(
            std::ffi::CString::new(path_s).unwrap().as_ptr(),
            &mut out as *mut ptiff_image_descriptor,
        );
        assert_eq!(rc, 0);
        assert_eq!(out.width, 64);
        assert_eq!(out.height, 48);
        assert_eq!(out.has_tile_info, 1);
        assert_eq!(out.tile_info.tile_width, 16);
        assert_eq!(out.tile_info.tile_height, 16);
    }
}
