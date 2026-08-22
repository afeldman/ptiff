//! Camera C ABI (`bindings/c/ptiff_camera.h`).
//!
//! The `ptiff_camera` struct is a structured view of the `ptiff.camera.*`
//! extension fields: pinhole intrinsics, extrinsics (rotation quaternion +
//! world translation), an ISO-8601 timestamp, and the three derived matrices
//! (K, [R|t], P) all in row-major doubles.
//!
//! The structured camera round-trip is a **later slice**: the Rust core models
//! the camera extension domain structurally (see `ptiff_core::geometry::Camera`)
//! but the `SceneDeserializer` does not yet reconstruct `camera`/`crs` from the
//! TIFF storage model (they are excluded from the serde path), so the values
//! are not currently readable back from a file through the idiomatic `Tiff`.
//! Until that land, [`ptiff_open_path_camera`] is a recognised-but-unimplemented
//! entry point returning `PTIFF_ERROR_NOT_IMPLEMENTED`, and the symbol/type
//! surface stays ABI-complete (foreign runtimes link and call, but get a clean
//! "not implemented" error rather than a wrong answer).

use crate::error::{ptiff_error_code, to_c_error};

/// Mirror of the C `ptiff_camera` struct (`ptiff_camera.h`).
///
/// All matrices are row-major `double`s; `projection = K * [R|t]` (3×4). The
/// `has_*` flags are `0` when the corresponding field group is absent.
#[repr(C)]
#[derive(Debug, Clone)]
pub struct ptiff_camera {
    // intrinsics
    pub has_intrinsics: i32,
    pub focal_length_x: f64,
    pub focal_length_y: f64,
    pub principal_x: f64,
    pub principal_y: f64,
    pub intrinsics: [f64; 9],
    // extrinsics
    pub has_extrinsics: i32,
    pub rotation_w: f64,
    pub rotation_x: f64,
    pub rotation_y: f64,
    pub rotation_z: f64,
    pub position_x: f64,
    pub position_y: f64,
    pub position_z: f64,
    pub extrinsics: [f64; 12],
    // projection: P = K * [R|t]
    pub projection: [f64; 12],
    // ISO-8601 UTC observation timestamp
    pub timestamp: [u8; 64],
}

/// Reads the structured camera calibration decoded from the `ptiff.camera.*`
/// extension fields of the file at `path`.
///
/// **Not yet implemented.** The structured camera round-trip from a TIFF file
/// requires the core's `SceneDeserializer` to reconstruct `camera`/`crs` from
/// the storage model, which is a later slice; see the module docs. Returns
/// `-PTIFF_ERROR_NOT_IMPLEMENTED` and leaves `*out` untouched.
///
/// # Safety
///
/// `path` and `out` must be non-null (the header documents this); the function
/// dereferences neither argument, so a null pointer is simply ignored.
#[no_mangle]
pub extern "C" fn ptiff_open_path_camera(
    _path: *const std::os::raw::c_char,
    _out: *mut ptiff_camera,
) -> i32 {
    // Recognised-but-unimplemented: never touch the pointers.
    let _ = (to_c_error, ptiff_error_code::PTIFF_ERROR_NOT_IMPLEMENTED);
    -(ptiff_error_code::PTIFF_ERROR_NOT_IMPLEMENTED as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_path_camera_reports_not_implemented() {
        // The entry point must be present and ABI-callable; until the camera
        // round-trip lands it reports NotImplemented (a recognised code).
        let rc = ptiff_open_path_camera(std::ptr::null(), std::ptr::null_mut());
        assert_eq!(rc, -(ptiff_error_code::PTIFF_ERROR_NOT_IMPLEMENTED as i32));
    }

    #[test]
    fn camera_layout_is_c_shaped() {
        use core::mem::{align_of, size_of};
        // A plain, C-ABI-safe struct: no non-C types, expected alignment for a
        // f64-led struct. (Exact size is platform/`repr(C)`-determined; we only
        // assert it is not zero-sized and aligns like f64.)
        assert!(size_of::<ptiff_camera>() > size_of::<[f64; 12]>());
        assert_eq!(align_of::<ptiff_camera>(), align_of::<f64>());
    }
}
