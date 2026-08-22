//! A camera model capturing intrinsic and extrinsic parameters for a sensor.
//!
//! Mirrors `ptiff::Camera` (see `libptiff/include/ptiff/geometry/camera.hpp`).

use crate::geometry::{Extrinsics, Intrinsics};

/// A 3x3 intrinsic (calibration) matrix `K`, row-major (9 doubles).
pub type IntrinsicsMatrix = [f64; 9];

/// A 3x4 extrinsic matrix `[R | t]`, row-major (12 doubles).
pub type ExtrinsicsMatrix = [f64; 12];

/// A 3x4 projection matrix `P = K * [R | t]`, row-major (12 doubles).
pub type ProjectionMatrix = [f64; 12];

/// A 3x3 rotation matrix (row-major) derived from an [`Extrinsics`] rotation
/// quaternion.
pub type RotationMatrix = [f64; 9];

/// A camera model capturing intrinsic and extrinsic parameters for a single
/// sensor.
///
/// A `Camera` represents the acquisition geometry of one sensor: its intrinsic
/// parameters ([`Intrinsics`]) and its extrinsic pose ([`Extrinsics`]), together
/// with an observation timestamp. From these it can synthesise the full 3x4
/// projection matrix `P = K * [R | t]`, the standard pinhole mapping from world
/// to image coordinates.
///
/// The timestamp is an ISO-8601 UTC string (e.g. `"2026-08-21T12:34:56.000Z"`).
/// Time-dependent calibration (a satellite-mounted camera that moves during
/// acquisition) is represented by treating each frame's `Camera` as the pose
/// valid at that frame's timestamp.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Camera {
    model: String,
    intrinsics: Intrinsics,
    extrinsics: Extrinsics,
    timestamp: String,
}

impl Camera {
    /// Constructs a default (identity) camera: model `"pinhole"`, zero
    /// intrinsics, identity pose, no timestamp.
    #[inline]
    pub fn new() -> Self {
        Self {
            model: "pinhole".to_string(),
            intrinsics: Intrinsics::default(),
            extrinsics: Extrinsics::IDENTITY,
            timestamp: String::new(),
        }
    }

    /// Constructs a fully-specified camera.
    ///
    /// - `model`: camera model identifier (e.g. `"pinhole"`).
    /// - `intrinsics`: intrinsic parameters (`fx`, `fy`, `cx`, `cy`).
    /// - `extrinsics`: extrinsic pose (rotation + translation).
    /// - `timestamp`: ISO-8601 UTC observation timestamp; empty means unset.
    #[inline]
    pub fn from_model(
        model: impl Into<String>,
        intrinsics: Intrinsics,
        extrinsics: Extrinsics,
        timestamp: impl Into<String>,
    ) -> Self {
        Self {
            model: model.into(),
            intrinsics,
            extrinsics,
            timestamp: timestamp.into(),
        }
    }

    /// Returns the camera model identifier (e.g. `"pinhole"`).
    #[inline]
    pub fn model_name(&self) -> &str {
        &self.model
    }

    /// Returns the intrinsic parameters.
    #[inline]
    pub fn intrinsics(&self) -> Intrinsics {
        self.intrinsics
    }

    /// Returns the extrinsic pose.
    #[inline]
    pub fn extrinsics(&self) -> Extrinsics {
        self.extrinsics
    }

    /// Returns the ISO-8601 UTC observation timestamp (empty when unset).
    #[inline]
    pub fn timestamp(&self) -> &str {
        &self.timestamp
    }

    /// Sets the observation timestamp.
    #[inline]
    pub fn set_timestamp(&mut self, timestamp: impl Into<String>) {
        self.timestamp = timestamp.into();
    }

    /// Builds the 3x3 intrinsic (calibration) matrix `K` (row-major).
    ///
    /// ```text
    ///     K = [ fx   0  cx ]
    ///         [  0  fy  cy ]
    ///         [  0   0   1 ]
    /// ```
    #[inline]
    pub fn intrinsics_matrix(&self) -> IntrinsicsMatrix {
        let i = &self.intrinsics;
        [
            i.focal_length_pixels_x,
            0.0,
            i.principal_point_x,
            0.0,
            i.focal_length_pixels_y,
            i.principal_point_y,
            0.0,
            0.0,
            1.0,
        ]
    }

    /// Builds the 3x4 extrinsic matrix `[R | t]` (row-major) from the rotation
    /// quaternion and the world translation.
    ///
    /// `R` is the camera-to-world rotation; `t` is the camera position in world
    /// coordinates.
    #[inline]
    pub fn extrinsics_matrix(&self) -> ExtrinsicsMatrix {
        let (r, t) = rotation_matrix_and_translation(&self.extrinsics);
        let mut out = [0.0; 12];
        out[0] = r[0];
        out[1] = r[1];
        out[2] = r[2];
        out[3] = t[0];
        out[4] = r[3];
        out[5] = r[4];
        out[6] = r[5];
        out[7] = t[1];
        out[8] = r[6];
        out[9] = r[7];
        out[10] = r[8];
        out[11] = t[2];
        out
    }

    /// The 3x3 rotation matrix (row-major) of the extrinsic pose.
    #[inline]
    pub fn rotation_matrix(&self) -> RotationMatrix {
        let (r, _) = rotation_matrix_and_translation(&self.extrinsics);
        r
    }

    /// The world translation of the extrinsic pose.
    #[inline]
    pub fn translation(&self) -> [f64; 3] {
        let (_, t) = rotation_matrix_and_translation(&self.extrinsics);
        t
    }

    /// Computes the full 3x4 projection matrix `P = K * [R | t]` (row-major).
    ///
    /// This maps a world point `X` to a homogeneous image point via `x ~ P * X`.
    /// The result is the standard pinhole projection built from the two
    /// calibration matrices.
    #[inline]
    pub fn projection_matrix(&self) -> ProjectionMatrix {
        let kmat = self.intrinsics_matrix();
        let rt = self.extrinsics_matrix();
        // P[i][j] = sum_m K[i][m] * Rt[m][j], row-major 3x4.
        let mut p = [0.0; 12];
        for row in 0..3 {
            for col in 0..4 {
                let mut v = 0.0;
                for m in 0..3 {
                    v += kmat[row * 3 + m] * rt[m * 4 + col];
                }
                p[row * 4 + col] = v;
            }
        }
        p
    }
}

impl Default for Camera {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

/// Computes the row-major 3x3 rotation matrix and the 3-component translation
/// from an [`Extrinsics`] pose, exactly mirroring the C++ oracle's quaternion →
/// rotation formulas (camera-to-world).
pub(crate) fn rotation_matrix_and_translation(e: &Extrinsics) -> (RotationMatrix, [f64; 3]) {
    let q = &e.rotation;
    let t = &e.translation;
    let w = q.w;
    let x = q.x;
    let y = q.y;
    let z = q.z;
    let xx = x * x;
    let yy = y * y;
    let zz = z * z;
    let xy = x * y;
    let xz = x * z;
    let yz = y * z;
    let wx = w * x;
    let wy = w * y;
    let wz = w * z;

    let r = [
        1.0 - 2.0 * (yy + zz),
        2.0 * (xy - wz),
        2.0 * (xz + wy),
        2.0 * (xy + wz),
        1.0 - 2.0 * (xx + zz),
        2.0 * (yz - wx),
        2.0 * (xz - wy),
        2.0 * (yz + wx),
        1.0 - 2.0 * (xx + yy),
    ];
    (r, [t.x, t.y, t.z])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{Quaternion, Vec3};

    #[test]
    fn default_camera_is_pinhole_identity() {
        let c = Camera::new();
        assert_eq!(c.model_name(), "pinhole");
        assert_eq!(c.intrinsics(), Intrinsics::default());
        assert_eq!(c.extrinsics(), Extrinsics::IDENTITY);
        assert_eq!(c.timestamp(), "");
    }

    #[test]
    fn intrinsics_matrix_is_pinhole() {
        let c = Camera::from_model(
            "pinhole",
            Intrinsics::new(900.0, 900.0, 512.0, 384.0),
            Extrinsics::IDENTITY,
            "",
        );
        let k = c.intrinsics_matrix();
        assert_eq!(k, [900.0, 0.0, 512.0, 0.0, 900.0, 384.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn identity_extrinsics_gives_identity_rotation() {
        let c = Camera::new();
        let r = c.rotation_matrix();
        assert_eq!(r, [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]);
        assert_eq!(c.translation(), [0.0, 0.0, 0.0]);
    }

    #[test]
    fn extrinsics_matrix_rows_carry_rotation_and_translation() {
        // 180° about +x: R = [[1,0,0],[0,-1,0],[0,0,-1]]; translation (1,2,3).
        let c = Camera::from_model(
            "pinhole",
            Intrinsics::default(),
            Extrinsics::new(
                Quaternion::new(0.0, 1.0, 0.0, 0.0),
                Vec3::new(1.0, 2.0, 3.0),
            ),
            "",
        );
        let rt = c.extrinsics_matrix();
        // Row 0: [R00, R01, R02, tx] = [1, 0, 0, 1]
        assert_eq!(rt[0], 1.0);
        assert_eq!(rt[1], 0.0);
        assert_eq!(rt[2], 0.0);
        assert_eq!(rt[3], 1.0);
        // Row 1: [R10, R11, R12, ty] = [0, -1, 0, 2]
        assert_eq!(rt[4], 0.0);
        assert_eq!(rt[5], -1.0);
        assert_eq!(rt[6], 0.0);
        assert_eq!(rt[7], 2.0);
        // Row 2: [R20, R21, R22, tz] = [0, 0, -1, 3]
        assert_eq!(rt[8], 0.0);
        assert_eq!(rt[9], 0.0);
        assert_eq!(rt[10], -1.0);
        assert_eq!(rt[11], 3.0);
    }

    #[test]
    fn projection_matrix_equals_k_times_rt() {
        let c = Camera::from_model(
            "pinhole",
            Intrinsics::new(900.0, 900.0, 512.0, 384.0),
            Extrinsics::new(Quaternion::IDENTITY, Vec3::new(1.0, 2.0, 3.0)),
            "",
        );
        let p = c.projection_matrix();
        let k = c.intrinsics_matrix();
        let rt = c.extrinsics_matrix();
        for row in 0..3 {
            for col in 0..4 {
                let mut expected = 0.0;
                for kk in 0..3 {
                    expected += k[row * 3 + kk] * rt[kk * 4 + col];
                }
                assert!((p[row * 4 + col] - expected).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn projection_maps_world_point_to_image_plane() {
        // Identity pose, principal point at image center, unit focal length.
        let c = Camera::from_model(
            "pinhole",
            Intrinsics::new(1.0, 1.0, 512.0, 384.0),
            Extrinsics::IDENTITY,
            "",
        );
        let p = c.projection_matrix();
        // World point directly in front: X = (0, 0, 1).
        // x = P·X (homogeneous): px = 0*1 + 512, py = 0*1 + 384, pw = 1.
        let (x, y, w) = (
            p[0] * 0.0 + p[1] * 0.0 + p[2] * 1.0 + p[3],
            p[4] * 0.0 + p[5] * 0.0 + p[6] * 1.0 + p[7],
            p[8] * 0.0 + p[9] * 0.0 + p[10] * 1.0 + p[11],
        );
        assert!((w - 1.0).abs() < 1e-9);
        assert!((x - 512.0).abs() < 1e-9);
        assert!((y - 384.0).abs() < 1e-9);
    }
}
