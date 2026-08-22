//! A PTIFF pose: an SE(3) transform bound to an explicit frame pair.
//!
//! Per GEOMETRY-FOUNDATION.md §4.4, a bare SE(3) transform does not say whether it
//! means `camera → spacecraft` or `spacecraft → J2000`. A [`Pose`] attaches the
//! frame semantics via a [`FramePair`] (`from`/`to`) to the `multicalc` SE(3)
//! kernel, so the scientific meaning is preserved while the math stays clean.
//!
//! The underlying representation is `multicalc::spatial::SE3<f64>`; the PTIFF
//! storage values ([`Vec3`](crate::geometry::Vec3), [`Quaternion`](crate::geometry::Quaternion))
//! convert to/from it. Quaternion + translation remains the canonical serialized
//! representation (GEOMETRY-FOUNDATION.md §5); `Pose` is the computational wrapper.

use crate::error::{Error, ErrorCode, Result};
use crate::geometry::{Extrinsics, Frame, FramePair, Quaternion, Vec3};
use multicalc::linear_algebra::{Vector3D, Vector6D};
use multicalc::spatial::{SE3, SO3};

/// A directed spatial transform expressed in an explicit frame pair.
///
/// `Pose` owns an SE(3) transform (`rotation` + `translation`) and a
/// [`FramePair`] indicating the source (`from`) and destination (`to`) frames.
/// The pose expresses coordinates of `from` in `to`.
///
/// Frame-aware composition is checked: composing `self` (`from → self.to`) with
/// `rhs` (`rhs.from → to`) requires `self.to == rhs.from`, and yields a pose for
/// `self.from → rhs.to`. See [`Pose::compose`].
#[derive(Debug, Clone, Copy)]
pub struct Pose {
    se3: SE3<f64>,
    frames: FramePair,
}

impl Pose {
    /// The identity transform over the given frame pair: no rotation, no
    /// translation.
    #[inline]
    pub fn identity(frames: FramePair) -> Self {
        Self {
            se3: SE3::identity(),
            frames,
        }
    }

    /// Constructs a pose from an SE(3) transform and an explicit frame pair.
    #[inline]
    pub fn from_se3(se3: SE3<f64>, frames: FramePair) -> Self {
        Self { se3, frames }
    }

    /// Constructs a pose from a rotation quaternion and a translation.
    #[inline]
    pub fn from_parts(rotation: Quaternion, translation: Vec3, frames: FramePair) -> Self {
        let q = multicalc::spatial::Quaternion::new(rotation.w, rotation.x, rotation.y, rotation.z);
        let so3 = SO3::from_quaternion(q);
        let t = Vector3D::new([translation.x, translation.y, translation.z]);
        Self {
            se3: SE3::from_parts(so3, t),
            frames,
        }
    }

    /// Constructs a pose from the canonical [`Extrinsics`] storage value.
    #[inline]
    pub fn from_extrinsics(extrinsics: Extrinsics, frames: FramePair) -> Self {
        Self::from_parts(extrinsics.rotation, extrinsics.translation, frames)
    }

    /// The underlying SE(3) transform.
    #[inline]
    pub fn as_se3(&self) -> &SE3<f64> {
        &self.se3
    }

    /// The underlying SE(3) transform, by value.
    #[inline]
    pub fn to_se3(&self) -> SE3<f64> {
        self.se3
    }

    /// The frame pair this pose is expressed in (`from → to`).
    #[inline]
    pub fn frames(&self) -> FramePair {
        self.frames
    }

    /// The source frame.
    #[inline]
    pub fn from_frame(&self) -> Frame {
        self.frames.from
    }

    /// The destination frame.
    #[inline]
    pub fn to_frame(&self) -> Frame {
        self.frames.to
    }

    /// The rotation as a PTIFF [`Quaternion`].
    #[inline]
    pub fn rotation(&self) -> Quaternion {
        let q = self.se3.rotation().quaternion();
        Quaternion::new(q.w(), q.x(), q.y(), q.z())
    }

    /// The translation as a PTIFF [`Vec3`].
    #[inline]
    pub fn translation(&self) -> Vec3 {
        let t = self.se3.translation();
        let [x, y, z] = *t.as_array();
        Vec3::new(x, y, z)
    }

    /// The canonical [`Extrinsics`] storage value for this pose.
    #[inline]
    pub fn extrinsics(&self) -> Extrinsics {
        Extrinsics::new(self.rotation(), self.translation())
    }

    /// Composes two poses, checking frame compatibility.
    ///
    /// `self` must express `from → to` and `rhs` must express `to → to'`; the
    /// result expresses `from → to'`. The SE(3) composition is `self * rhs`
    /// (apply `rhs` first, then `self`).
    ///
    /// Returns [`ErrorCode::InvalidArgument`] if the frames do not chain
    /// (`self.to != rhs.from`).
    #[inline]
    pub fn compose(&self, rhs: &Self) -> Result<Self> {
        if self.to_frame() != rhs.from_frame() {
            let msg = format!(
                "Pose::compose: frame mismatch: rhs.from ({}) != self.to ({})",
                rhs.from_frame(),
                self.to_frame()
            );
            return Err(Error::new(ErrorCode::InvalidArgument, msg));
        }
        let frames = FramePair::new(self.from_frame(), rhs.to_frame());
        Ok(Self {
            se3: self.se3.compose(rhs.se3),
            frames,
        })
    }

    /// The inverse pose, swapping the frame direction (`from → to` becomes
    /// `to → from`).
    #[inline]
    pub fn inverse(&self) -> Self {
        Self {
            se3: self.se3.inverse(),
            frames: FramePair::new(self.to_frame(), self.from_frame()),
        }
    }

    /// The relative pose from `self` to `other` over a common frame.
    ///
    /// Given `self: A → B` and `other: A → B'` (both starting in the same frame
    /// `A`), returns the pose `B → B'` that maps from `self`'s destination to
    /// `other`'s destination. Equivalent to `other * self⁻¹` in SE(3).
    ///
    /// Returns [`ErrorCode::InvalidArgument`] if the source frames do not match.
    #[inline]
    pub fn relative(&self, other: &Self) -> Result<Self> {
        if self.from_frame() != other.from_frame() {
            let msg = format!(
                "Pose::relative: source frame mismatch: self.from ({}) != other.from ({})",
                self.from_frame(),
                other.from_frame()
            );
            return Err(Error::new(ErrorCode::InvalidArgument, msg));
        }
        let frames = FramePair::new(self.to_frame(), other.to_frame());
        Ok(Self {
            se3: self.se3.inverse().compose(other.se3),
            frames,
        })
    }

    /// Applies the pose to a 3D point (rotates + translates).
    #[inline]
    pub fn act(&self, point: Vec3) -> Vec3 {
        let p = Vector3D::new([point.x, point.y, point.z]);
        let out = self.se3.act(p);
        let [x, y, z] = *out.as_array();
        Vec3::new(x, y, z)
    }

    /// The exponential map from a `[v; ω]` twist (se(3) element) to a pose with
    /// the given frames.
    #[inline]
    pub fn exp(xi: Vector6D<f64>, frames: FramePair) -> Self {
        Self {
            se3: SE3::exp(xi),
            frames,
        }
    }

    /// The logarithm, the inverse of [`Pose::exp`], returning the `[v; ω]` twist
    /// of this pose.
    #[inline]
    pub fn log(&self) -> Vector6D<f64> {
        self.se3.log()
    }

    /// The 6×6 SE(3) adjoint for velocity / twist coordinate transforms.
    #[inline]
    pub fn adjoint(&self) -> multicalc::linear_algebra::Matrix6D<f64> {
        self.se3.adjoint()
    }

    /// Geodesic (screw-motion) interpolation. Requires `self` and `other` to
    /// share the same frame pair.
    ///
    /// Returns [`ErrorCode::InvalidArgument`] if the frame pairs differ.
    #[inline]
    pub fn interpolate(&self, other: &Self, t: f64) -> Result<Self> {
        if self.frames() != other.frames() {
            let msg = format!(
                "Pose::interpolate: frame mismatch: self ({}) != other ({})",
                self.frames(),
                other.frames()
            );
            return Err(Error::new(ErrorCode::InvalidArgument, msg));
        }
        Ok(Self {
            se3: self.se3.interpolate(other.se3, t),
            frames: self.frames,
        })
    }
}

impl Default for Pose {
    /// Identity over the default (empty) frame pair.
    fn default() -> Self {
        // An empty frame pair is intentionally ambiguous ("from → to"); the
        // caller should supply explicit frames via `Pose::identity(pair)`.
        Self::identity(FramePair::new(Frame::new(""), Frame::new("")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use multicalc::linear_algebra::Vector;

    fn cam_to_sc() -> FramePair {
        FramePair::new(Frame::CAMERA, Frame::SPACECRAFT)
    }

    fn sc_to_j2000() -> FramePair {
        FramePair::new(Frame::SPACECRAFT, Frame::J2000)
    }

    #[test]
    fn identity_pose_is_se3_identity() {
        let p = Pose::identity(cam_to_sc());
        assert_eq!(p.rotation(), Quaternion::IDENTITY);
        assert_eq!(p.translation(), Vec3::ZERO);
        assert_eq!(p.frames(), cam_to_sc());
    }

    #[test]
    fn from_parts_round_trips_rotation_and_translation() {
        let p = Pose::from_parts(
            Quaternion::new(0.0, 1.0, 0.0, 0.0),
            Vec3::new(1.0, 2.0, 3.0),
            cam_to_sc(),
        );
        assert_eq!(p.rotation(), Quaternion::new(0.0, 1.0, 0.0, 0.0));
        assert_eq!(p.translation(), Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn extrinsics_conversion_round_trip() {
        let e = Extrinsics::new(
            Quaternion::new(0.0, 0.0, 0.0, 1.0),
            Vec3::new(4.0, 5.0, 6.0),
        );
        let p = Pose::from_extrinsics(e, cam_to_sc());
        assert_eq!(p.extrinsics(), e);
        assert_eq!(p.frames(), cam_to_sc());
    }

    #[test]
    fn inverse_swaps_frames_and_inverts_se3() {
        let p = Pose::from_parts(Quaternion::IDENTITY, Vec3::new(1.0, 0.0, 0.0), cam_to_sc());
        let inv = p.inverse();
        assert_eq!(
            inv.frames(),
            FramePair::new(Frame::SPACECRAFT, Frame::CAMERA)
        );
        // inverse * self == identity SE(3) for the translation part
        let composed = inv.to_se3().compose(p.to_se3());
        let t = composed.translation();
        let [x, y, z] = *t.as_array();
        assert!((x.abs() < 1e-9) && (y.abs() < 1e-9) && (z.abs() < 1e-9));
    }

    #[test]
    fn compose_checks_frame_chaining() {
        let a_to_b = Pose::identity(cam_to_sc());
        let b_to_c = Pose::identity(sc_to_j2000());
        let chained = a_to_b.compose(&b_to_c).unwrap();
        assert_eq!(
            chained.frames(),
            FramePair::new(Frame::CAMERA, Frame::J2000)
        );

        // Non-chaining frames must be rejected: this pose's `from` (J2000)
        // does not match `a_to_b`'s `to` (spacecraft).
        let wrong = Pose::identity(FramePair::new(Frame::J2000, Frame::CAMERA));
        assert!(a_to_b.compose(&wrong).is_err());
    }

    #[test]
    fn relative_requires_common_source_frame() {
        let a = Pose::from_parts(Quaternion::IDENTITY, Vec3::ZERO, cam_to_sc());
        let b = Pose::from_parts(Quaternion::IDENTITY, Vec3::new(1.0, 0.0, 0.0), cam_to_sc());
        let rel = a.relative(&b).unwrap();
        // Both start in camera; relative maps spacecraft(B) → spacecraft(B').
        assert_eq!(
            rel.frames(),
            FramePair::new(Frame::SPACECRAFT, Frame::SPACECRAFT)
        );
        let out = rel.act(Vec3::ZERO);
        assert!((out.x - 1.0).abs() < 1e-9);

        // Mismatched source frames are rejected.
        let other_frame = Pose::identity(sc_to_j2000());
        assert!(a.relative(&other_frame).is_err());
    }

    #[test]
    fn act_applies_rotation_then_translation() {
        // 180° about +x maps (0,1,0) -> (0,-1,0); plus translation (0,0,1).
        let p = Pose::from_parts(
            Quaternion::new(0.0, 1.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            cam_to_sc(),
        );
        let out = p.act(Vec3::new(0.0, 1.0, 0.0));
        assert!((out.x).abs() < 1e-9);
        assert!((out.y + 1.0).abs() < 1e-9);
        assert!((out.z - 1.0).abs() < 1e-9);
    }

    #[test]
    fn log_exp_round_trip() {
        let xi = Vector::new([0.1_f64, -0.2, 0.3, 0.2, -0.1, 0.4]);
        let p = Pose::exp(xi, cam_to_sc());
        let back = p.log();
        for i in 0..6 {
            assert!((back.as_array()[i] - xi.as_array()[i]).abs() < 1e-9);
        }
    }

    #[test]
    fn adjoint_is_consistent_with_se3() {
        let p = Pose::from_parts(
            Quaternion::new(0.0, 0.0, 0.0, 1.0),
            Vec3::new(1.0, 0.0, 0.0),
            cam_to_sc(),
        );
        // Pose::adjoint must equal the underlying SE3 adjoint.
        let expected = p.to_se3().adjoint();
        assert_eq!(p.adjoint(), expected);
    }

    #[test]
    fn interpolate_requires_same_frames() {
        let a = Pose::identity(cam_to_sc());
        let b = Pose::from_parts(Quaternion::IDENTITY, Vec3::new(2.0, 0.0, 0.0), cam_to_sc());
        let mid = a.interpolate(&b, 0.5).unwrap();
        assert!((mid.translation().x - 1.0).abs() < 1e-9);

        let other = Pose::identity(sc_to_j2000());
        assert!(a.interpolate(&other, 0.5).is_err());
    }

    #[test]
    fn frames_are_directed_and_displayed() {
        let p = Pose::identity(cam_to_sc());
        assert_eq!(p.from_frame(), Frame::CAMERA);
        assert_eq!(p.to_frame(), Frame::SPACECRAFT);
    }

    // --- numerical edge cases (GEOMETRY-FOUNDATION.md Step 9) ---

    #[test]
    fn small_angle_exp_log_round_trip() {
        // A tiny rotation about +z: log∘exp must recover the original twist.
        let xi = Vector::new([0.0_f64, 0.0, 0.0, 0.0, 0.0, 1e-8]);
        let p = Pose::exp(xi, cam_to_sc());
        let back = p.log();
        for i in 0..6 {
            assert!((back.as_array()[i] - xi.as_array()[i]).abs() < 1e-15);
        }
    }

    #[test]
    fn pure_rotation_pose_keeps_origin_fixed() {
        // A pure rotation (identity translation) must keep the frame origin fixed.
        let p = Pose::from_parts(Quaternion::new(0.0, 0.0, 0.0, 1.0), Vec3::ZERO, cam_to_sc());
        let out = p.act(Vec3::ZERO);
        assert_eq!(out, Vec3::ZERO);
    }

    #[test]
    fn pure_translation_pose_preserves_direction() {
        // A pure translation must not rotate: applying it to a unit vector keeps it parallel.
        let p = Pose::from_parts(Quaternion::IDENTITY, Vec3::new(10.0, 0.0, 0.0), cam_to_sc());
        let dir = Vec3::new(1.0, 0.0, 0.0);
        // (p ∘ act): translation-only acts add; a direction should be unchanged up to no rotation.
        let out = p.act(dir);
        let rotated_dir = Vec3::new(out.x - 10.0, out.y, out.z);
        assert!((rotated_dir.x - 1.0).abs() < 1e-9);
        assert!(rotated_dir.y.abs() < 1e-9);
        assert!(rotated_dir.z.abs() < 1e-9);
    }

    #[test]
    fn composed_then_inverse_returns_to_origin() {
        // Given camera→spacecraft and spacecraft→J2000, applying the composition then its
        // inverse must restore the original J2000-free point of view.
        let cam_sc = Pose::from_parts(Quaternion::IDENTITY, Vec3::new(1.0, 0.0, 0.0), cam_to_sc());
        let sc_j2000 = Pose::from_parts(
            Quaternion::new(0.0, 0.0, 0.0, 1.0),
            Vec3::new(0.0, 2.0, 0.0),
            sc_to_j2000(),
        );
        let cam_j2000 = cam_sc.compose(&sc_j2000).unwrap();
        let inv = cam_j2000.inverse();
        let round = cam_j2000.compose(&inv).unwrap();
        assert_eq!(round.frames(), FramePair::new(Frame::CAMERA, Frame::CAMERA));
        // round ~ identity: recomposes to identity transform.
        let seam = round.to_se3();
        let t = seam.translation();
        assert!(t.as_array()[0].abs() < 1e-9);
        assert!(t.as_array()[1].abs() < 1e-9);
        assert!(t.as_array()[2].abs() < 1e-9);
    }
}
