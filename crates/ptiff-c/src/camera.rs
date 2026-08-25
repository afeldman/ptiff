//! Camera C ABI (emitted into `target/ptiff_c.h` by cbindgen from this crate).
//!
//! The `ptiff_camera` struct is a structured view of the `ptiff.camera.*`
//! extension fields: pinhole intrinsics, extrinsics (rotation quaternion +
//! world translation), an ISO-8601 timestamp, and the three derived matrices
//! (K, [R|t], P) all in row-major doubles.
//!
//! The Rust core models the camera extension domain structurally
//! (`ptiff_core::geometry::Camera`) and round-trips it through the PTIFF tags
//! 65002/65003 via `geometry::marshal`. On top of that, [`ptiff_open_path_camera`]
//! decodes the full structured `ptiff_camera` from a file (matching the C++
//! oracle the original `bindings/c` veneer implemented), and
//! `ptiff_sink_create_camera` (in `pixel_bridge.rs`) attaches it to a write.

use ptiff_core::{Camera, Extrinsics, Intrinsics, Quaternion, Vec3};
use std::ffi::CStr;
use std::os::raw::c_char;

use crate::error::{ptiff_error_code, to_c_error};

/// Mirror of the C `ptiff_camera` struct (`ptiff_camera.h`).
///
/// A structured view of the `ptiff.camera.*` extension fields: the pinhole
/// intrinsics (fx, fy, cx, cy), the extrinsics (rotation quaternion + world
/// translation), the ISO-8601 observation timestamp, the three derived
/// matrices, and the lens (distortion) model. All matrices are row-major
/// `double`s. `has_intrinsics` / `has_extrinsics` are `0` when the
/// corresponding field group is absent from the file; `projection` is set
/// whenever both groups are present. `timestamp` is an empty string when
/// unset. The lens fields (`has_lens`, `lens_kind`, `lens_param_count`, and
/// the fixed `lens_param_key`/`lens_param_value` arrays) carry the structured
/// distortion model: a canonical kind string plus up to
/// [`K_CAMERA_LENS_MAX_PARAMS`] named double parameters.
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
    // ISO-8601 UTC observation timestamp (a NUL-terminated char buffer;
    // cbindgen emits `char[64]`, the ABI shape every foreign runtime assigns
    // as a plain string).
    pub timestamp: [c_char; 64],
    // lens (distortion) model
    pub has_lens: i32,
    pub lens_kind: [c_char; 32],
    pub lens_param_count: u32,
    pub lens_param_key: [[c_char; 16]; K_CAMERA_LENS_MAX_PARAMS],
    pub lens_param_value: [f64; K_CAMERA_LENS_MAX_PARAMS],
}

/// Maximum number of named lens (distortion) parameters carried by the fixed
/// C-ABI `ptiff_camera` struct. Kept small and bounded so the ABI stays
/// self-contained (no pointers, no variable-length fields); readers that need
/// more coefficients work on the flat `ptiff.camera.lens.*` field view instead.
pub const K_CAMERA_LENS_MAX_PARAMS: usize = 8;

/// Fixed key-buffer length for one lens parameter name.
pub const K_CAMERA_LENS_KEY_LEN: usize = 16;

impl ptiff_camera {
    /// A zero-initialized camera (all `has_*` flags 0, matrices 0, empty
    /// timestamp), matching `std::memset(&cam, 0, sizeof(cam))` in the C++ oracle.
    fn c_default() -> Self {
        Self {
            has_intrinsics: 0,
            focal_length_x: 0.0,
            focal_length_y: 0.0,
            principal_x: 0.0,
            principal_y: 0.0,
            intrinsics: [0.0; 9],
            has_extrinsics: 0,
            rotation_w: 0.0,
            rotation_x: 0.0,
            rotation_y: 0.0,
            rotation_z: 0.0,
            position_x: 0.0,
            position_y: 0.0,
            position_z: 0.0,
            extrinsics: [0.0; 12],
            projection: [0.0; 12],
            timestamp: [0 as c_char; 64],
            has_lens: 0,
            lens_kind: [0 as c_char; 32],
            lens_param_count: 0,
            lens_param_key: [[0 as c_char; K_CAMERA_LENS_KEY_LEN]; K_CAMERA_LENS_MAX_PARAMS],
            lens_param_value: [0.0; K_CAMERA_LENS_MAX_PARAMS],
        }
    }
}

/// Marshals a core [`Camera`] into the structured [`ptiff_camera`] view.
///
/// A core `Camera` always carries full intrinsics and extrinsics (the marshal
/// schema requires all fields), so both `has_*` flags are set and every matrix
/// is derived from the authoritative domain model — mirroring how the C++
/// oracle builds K / [R|t] / P through `ptiff::Camera`.
fn camera_to_c(cam: &Camera) -> ptiff_camera {
    let mut out = ptiff_camera::c_default();

    let i = cam.intrinsics();
    out.has_intrinsics = 1;
    out.focal_length_x = i.focal_length_pixels_x;
    out.focal_length_y = i.focal_length_pixels_y;
    out.principal_x = i.principal_point_x;
    out.principal_y = i.principal_point_y;
    out.intrinsics = cam.intrinsics_matrix();

    let e = cam.extrinsics();
    // The oracle reports has_extrinsics = 0 when the file/field-set carried no
    // extrinsic pose. Absence is represented in the domain model by the
    // identity pose (camera_from_model defaults to it), so an identity
    // extrinsics maps back to has_extrinsics = 0 on the ABI.
    out.has_extrinsics = if e == Extrinsics::IDENTITY { 0 } else { 1 };
    out.rotation_w = e.rotation.w;
    out.rotation_x = e.rotation.x;
    out.rotation_y = e.rotation.y;
    out.rotation_z = e.rotation.z;
    out.position_x = e.translation.x;
    out.position_y = e.translation.y;
    out.position_z = e.translation.z;
    out.extrinsics = cam.extrinsics_matrix();

    out.projection = cam.projection_matrix();

    // Timestamp (truncate safely into the fixed buffer, NUL-terminated).
    let ts = cam.timestamp().as_bytes();
    let n = ts.len().min(out.timestamp.len() - 1);
    for (i, b) in ts[..n].iter().enumerate() {
        out.timestamp[i] = (*b) as c_char;
    }
    // out.timestamp[n..] stays zero (from c_default), i.e. a trailing NUL.

    // Lens (distortion) model: copy the canonical kind string plus up to
    // K_CAMERA_LENS_MAX_PARAMS named double parameters into the fixed arrays
    // (truncating key names to K_CAMERA_LENS_KEY_LEN-1, NUL-terminated). A
    // default pinhole lens with no parameters sets has_lens = 1 so readers
    // know the kind is explicit; parameters beyond the fixed cap are dropped
    // on the ABI (the flat `ptiff.camera.lens.*` field view carries them).
    let lens = cam.lens_model();
    out.has_lens = 1;
    write_cstr(
        &mut out.lens_kind,
        ptiff::lens_model_kind_str(lens.kind()).as_bytes(),
    );
    let mut count = 0u32;
    for (key, value) in lens.iter() {
        if count >= K_CAMERA_LENS_MAX_PARAMS as u32 {
            break;
        }
        write_cstr(&mut out.lens_param_key[count as usize], key.as_bytes());
        out.lens_param_value[count as usize] = value;
        count += 1;
    }
    out.lens_param_count = count;

    out
}

/// Reconstructs a core [`Camera`] from the structured [`ptiff_camera`] view.
///
/// The model comes from the C ABI's fixed `"pinhole"` convention (the C++ write
/// side stores `"pinhole"` as-is), so a non-empty model string is preserved.
/// The lens (distortion) model is rebuilt from `has_lens` / `lens_kind` /
/// `lens_param_*`; when `has_lens` is 0 (old writers without lens fields) the
/// default pinhole lens is used.
pub fn camera_from_c(cam: &ptiff_camera) -> Camera {
    let timestamp = read_timestamp(&cam.timestamp);

    let mut lens = ptiff::LensModel::new(if cam.has_lens == 0 {
        ptiff::LensModelKind::Pinhole
    } else {
        ptiff::lens_model_kind_from_str(&read_fixed_cstr(&cam.lens_kind))
    });
    let n = (cam.lens_param_count as usize).min(K_CAMERA_LENS_MAX_PARAMS);
    for i in 0..n {
        let key = read_fixed_cstr(&cam.lens_param_key[i]);
        if !key.is_empty() {
            lens.set_parameter(key, cam.lens_param_value[i]);
        }
    }

    ptiff::Camera::from_model_with_lens(
        "pinhole",
        Intrinsics::new(
            cam.focal_length_x,
            cam.focal_length_y,
            cam.principal_x,
            cam.principal_y,
        ),
        Extrinsics::new(
            Quaternion::new(
                cam.rotation_w,
                cam.rotation_x,
                cam.rotation_y,
                cam.rotation_z,
            ),
            Vec3::new(cam.position_x, cam.position_y, cam.position_z),
        ),
        timestamp,
        lens,
    )
}

/// Copies `bytes` into `out` a fixed char buffer, NUL-terminated and truncated
/// to fit (leaving the remaining bytes zero from the caller's default init).
fn write_cstr<const N: usize>(out: &mut [c_char; N], bytes: &[u8]) {
    let n = bytes.len().min(out.len() - 1);
    for (i, b) in bytes[..n].iter().enumerate() {
        out[i] = (*b) as c_char;
    }
    // out[n] stays zero (uninitialized tail from caller) -> trailing NUL.
}

/// Reads a NUL-terminated (or blank-padded) fixed char buffer into a `String`.
fn read_fixed_cstr<const N: usize>(buf: &[c_char; N]) -> String {
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    #[allow(clippy::unnecessary_cast)]
    let bytes: Vec<u8> = buf[..end].iter().map(|&b| b as u8).collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Reads a NUL-terminated (or blank-padded) `[c_char; 64]` timestamp into a `String`.
fn read_timestamp(buf: &[c_char; 64]) -> String {
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    // `c_char` is signed (i8) on x86_64/Windows/macOS but unsigned (u8) on
    // aarch64 Linux, so this cast is a real conversion on some targets and a
    // no-op on others -- clippy only sees the latter on the aarch64 CI runner.
    #[allow(clippy::unnecessary_cast)]
    let bytes: Vec<u8> = buf[..end].iter().map(|&b| b as u8).collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Reads the structured camera calibration decoded from the `ptiff.camera.*`
/// extension fields of the file at `path`.
///
/// Returns `0` on success and fills `*out` (with the `has_*` flags set when
/// the camera domain is present, otherwise an all-zero struct), and a negative
/// `ptiff_error_code` on failure (missing/invalid file, leaving `*out`
/// untouched).
///
/// # Safety
///
/// `path` and `out` must be non-null (the header documents this); `out` must
/// point to writable storage of at least `sizeof(ptiff_camera)`.
#[no_mangle]
pub extern "C" fn ptiff_open_path_camera(path: *const c_char, out: *mut ptiff_camera) -> i32 {
    if path.is_null() || out.is_null() {
        return -(ptiff_error_code::PTIFF_ERROR_INVALID_ARGUMENT as i32);
    }
    // Safety: validated non-null above; C string is NUL-terminated per header.
    let path_s = unsafe { CStr::from_ptr(path) }
        .to_string_lossy()
        .into_owned();

    let result = (|| -> Result<ptiff_camera, ptiff::Error> {
        let tiff = ptiff::Tiff::open(&path_s)?;
        let scene = tiff.scene();
        if scene.image_count() == 0 {
            return Err(ptiff::Error::not_found(
                "ptiff_open_path_camera: file has no images",
            ));
        }
        let image = scene.image_at(0)?;
        Ok(match image.camera() {
            Some(cam) => camera_to_c(cam),
            None => ptiff_camera::c_default(),
        })
    })();

    match result {
        Ok(cam) => {
            // Safety: `out` is a writable pointer of at least the struct size.
            unsafe { *out = cam };
            0
        }
        Err(e) => to_c_error(&e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_path_camera_null_arguments_is_invalid_argument() {
        assert_eq!(
            ptiff_open_path_camera(std::ptr::null(), std::ptr::null_mut()),
            -(ptiff_error_code::PTIFF_ERROR_INVALID_ARGUMENT as i32)
        );
    }

    #[test]
    fn null_camera_field_preserves_uint8_fallback() {
        // Sanity: building from a default (all-zero) C camera yields a well-formed
        // domain Camera (identity-ish), so the write path never panics.
        let cam = camera_from_c(&ptiff_camera::c_default());
        assert_eq!(cam.model_name(), "pinhole");
        assert_eq!(cam.timestamp(), "");
    }

    #[test]
    fn camera_c_layout_is_c_shaped() {
        use core::mem::{align_of, size_of};
        // A plain, C-ABI-safe struct: no non-C types, expected alignment for a
        // f64-led struct. (Exact size is platform/`repr(C)`-determined; we only
        // assert it is not zero-sized and aligns like f64.)
        assert!(size_of::<ptiff_camera>() > size_of::<[f64; 12]>());
        assert_eq!(align_of::<ptiff_camera>(), align_of::<f64>());
    }

    #[test]
    fn timestamp_round_trips_through_the_fixed_buffer() {
        let cam = Camera::from_model(
            "pinhole",
            Intrinsics::new(900.0, 901.0, 512.5, 384.25),
            Extrinsics::new(
                Quaternion::new(0.7, 0.1, 0.2, 0.3),
                Vec3::new(1.0, 2.0, 3.0),
            ),
            "2026-08-21T12:34:56.000Z",
        );
        let c = camera_to_c(&cam);
        assert_eq!(read_timestamp(&c.timestamp), "2026-08-21T12:34:56.000Z");
        // Truncation safety against an over-long timestamp.
        let long = camera_to_c(&Camera::from_model(
            "pinhole",
            cam.intrinsics(),
            cam.extrinsics(),
            "x".repeat(200),
        ));
        assert!(long.timestamp[long.timestamp.len() - 1] == 0);
    }

    #[test]
    fn camera_to_c_sets_matrices_and_flags() {
        let cam = Camera::from_model(
            "pinhole",
            Intrinsics::new(900.0, 901.0, 512.5, 384.25),
            Extrinsics::new(
                Quaternion::new(0.7, 0.1, 0.2, 0.3),
                Vec3::new(1.0, 2.0, 3.0),
            ),
            "",
        );
        let c = camera_to_c(&cam);
        assert_eq!(c.has_intrinsics, 1);
        assert_eq!(c.has_extrinsics, 1);
        assert_eq!(c.focal_length_x, 900.0);
        assert_eq!(c.principal_y, 384.25);
        assert_eq!(c.intrinsics, cam.intrinsics_matrix());
        assert_eq!(c.extrinsics, cam.extrinsics_matrix());
        assert_eq!(c.projection, cam.projection_matrix());
        assert_eq!(c.rotation_w, 0.7);
        assert_eq!(c.position_z, 3.0);
        // Default lens: count 0 but has_lens set (kind explicit).
        assert_eq!(c.has_lens, 1);
        assert_eq!(read_fixed_cstr(&c.lens_kind), "pinhole");
        assert_eq!(c.lens_param_count, 0);
    }

    #[test]
    fn lens_model_round_trips_through_c_abi() {
        let mut lens = ptiff::LensModel::new(ptiff::LensModelKind::Fisheye);
        lens.set_parameter("k1", -0.1);
        lens.set_parameter("k2", 0.05);
        let cam = ptiff::Camera::from_model_with_lens(
            "pinhole",
            Intrinsics::new(900.0, 901.0, 512.5, 384.25),
            Extrinsics::IDENTITY,
            "2026-08-21T12:34:56.000Z",
            lens,
        );
        let c = camera_to_c(&cam);
        assert_eq!(c.has_lens, 1);
        assert_eq!(read_fixed_cstr(&c.lens_kind), "fisheye");
        assert_eq!(c.lens_param_count, 2);
        // Parameters are sorted by key.
        assert_eq!(read_fixed_cstr(&c.lens_param_key[0]), "k1");
        assert_eq!(c.lens_param_value[0], -0.1);
        assert_eq!(read_fixed_cstr(&c.lens_param_key[1]), "k2");
        assert_eq!(c.lens_param_value[1], 0.05);

        // camera_from_c must reproduce the lens model.
        let back = camera_from_c(&c);
        assert_eq!(back.lens_model().kind(), ptiff::LensModelKind::Fisheye);
        assert_eq!(back.lens_model().parameter("k1").unwrap(), -0.1);
        assert_eq!(back.lens_model().parameter("k2").unwrap(), 0.05);
        // And the camera as a whole equals the original.
        assert_eq!(back, cam);
    }

    #[test]
    fn lens_kind_defaults_to_pinhole_when_has_lens_is_zero() {
        // A struct from an "old" writer (has_lens == 0, kind all-zero) must
        // build a camera with the pinhole default, never failing.
        let mut c = ptiff_camera::c_default();
        c.has_lens = 0;
        let cam = camera_from_c(&c);
        assert_eq!(cam.lens_model().kind(), ptiff::LensModelKind::Pinhole);
        assert!(cam.lens_model().is_empty());
    }
}
