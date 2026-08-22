//! Screw-theory wrappers over the se(3) twist (GEOMETRY-FOUNDATION.md §3).
//!
//! `multicalc` provides the raw Lie machinery (`Twist`, `SE3::exp`/`log`,
//! `SE3::adjoint`, geodesic `interpolate`). This module adds the **named screw
//! primitives** PTIFF wants on top: decomposing a `[v; ω]` twist into its
//! [`ScrewAxis`] (unit axis + point + pitch) and generating [`ScrewMotion`]s
//! (screw motions via `exp`).
//!
//! The convention is spatial (body-fixed) twist `ξ = [v; ω]` with `ω` angular
//! velocity, matching `multicalc` and the SPICE mapping (GEOMETRY-FOUNDATION.md
//! §6).

use multicalc::linear_algebra::Vector3D;
use multicalc::spatial::{Twist, SE3};

/// A screw motion: the exponential of a twist, i.e. `exp(ξ·θ)`.
///
/// A `ScrewMotion` is a rigid-body motion (a rotation about an axis combined
/// with a translation along that axis). It is generated from a [`Screw`] via
/// [`Screw::motion`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrewMotion {
    /// The SE(3) transform resulting from the screw motion.
    pub se3: SE3<f64>,
}

impl ScrewMotion {
    /// The underlying SE(3) transform of this screw motion.
    #[inline]
    pub fn as_se3(&self) -> &SE3<f64> {
        &self.se3
    }

    /// Applies the screw motion to a 3D point.
    #[inline]
    pub fn act(&self, p: Vector3D<f64>) -> Vector3D<f64> {
        self.se3.act(p)
    }

    /// The inverse screw motion.
    #[inline]
    pub fn inverse(&self) -> Self {
        Self {
            se3: self.se3.inverse(),
        }
    }
}

/// A screw axis together with its pitch, decomposed from a `[v; ω]` twist.
///
/// Per Se(3) screw theory, a twist `ξ = (v, ω)` decomposes into
///   - a unit rotation-axis direction `ŵ = ω / ‖ω‖`,
///   - a point `q` on the axis (`q = (ω × v) / ‖ω‖²`),
///   - the screw pitch `h = (v · ω) / ‖ω‖²`,
///
/// such that `v = h·ω + q × ω`.
///
/// For a **pure rotation** (`h = 0`) the screw reduces to a rotation about the
/// axis through `q`. For a **pure translation** (`ω ≈ 0`) there is no finite
/// rotation axis; the screw axis is degenerate (infinite pitch).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrewAxis {
    /// Unit direction of the rotation axis `ŵ = ω/‖ω‖`.
    pub direction: [f64; 3],
    /// A point on the screw axis, `q = (ω × v) / ‖ω‖²`.
    pub point: [f64; 3],
    /// The screw pitch `h = (v · ω) / ‖ω‖²` (translation per radian of
    /// rotation). `None` marks a pure translation (degenerate axis).
    pub pitch: Option<f64>,
}

/// A spatial (body-fixed) twist `ξ = [v; ω]` with screw-theory operations.
///
/// Wraps `multicalc::spatial::Twist<f64>` and adds
/// - axis/pitch decomposition ([`Screw::axis`], [`Screw::pitch`]),
/// - screw-motion generation ([`Screw::motion`]),
/// - magnitude queries ([`Screw::angular_magnitude`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Screw {
    twist: Twist<f64>,
}

impl Screw {
    /// Constructs a screw from a `multicalc` [`Twist`].
    #[inline]
    pub fn from_twist(twist: Twist<f64>) -> Self {
        Self { twist }
    }

    /// Constructs a screw from linear and angular parts `v`, `ω`.
    #[inline]
    pub fn new(linear: [f64; 3], angular: [f64; 3]) -> Self {
        Self {
            twist: Twist::new(Vector3D::new(linear), Vector3D::new(angular)),
        }
    }

    /// Constructs a screw from a `[vx, vy, vz, ωx, ωy, ωz]` array.
    #[inline]
    pub fn from_array(a: [f64; 6]) -> Self {
        Self {
            twist: Twist::from_array(a),
        }
    }

    /// The linear (translational) part `v`, as a 3-vector.
    #[inline]
    pub fn linear(&self) -> [f64; 3] {
        *self.twist.linear().as_array()
    }

    /// The angular (rotational) part `ω`, as a 3-vector.
    #[inline]
    pub fn angular(&self) -> [f64; 3] {
        *self.twist.angular().as_array()
    }

    /// The raw `multicalc` [`Twist`].
    #[inline]
    pub fn to_twist(&self) -> &Twist<f64> {
        &self.twist
    }

    /// The angular-magnitude `‖ω‖` (the rotation angle per unit "time" for a
    /// motion-generating twist).
    #[inline]
    pub fn angular_magnitude(&self) -> f64 {
        let [wx, wy, wz] = *self.twist.angular().as_array();
        (wx * wx + wy * wy + wz * wz).sqrt()
    }

    /// The screw pitch `h = (v · ω) / ‖ω‖²`.
    ///
    /// Returns `None` for a pure translation (`‖ω‖ ≈ 0`), where the pitch is
    /// infinite and no finite rotation axis exists.
    #[inline]
    pub fn pitch(&self) -> Option<f64> {
        let w = *self.twist.angular().as_array();
        let v = *self.twist.linear().as_array();
        let w2 = w[0] * w[0] + w[1] * w[1] + w[2] * w[2];
        if w2 < 1e-30 {
            None
        } else {
            let vd = v[0] * w[0] + v[1] * w[1] + v[2] * w[2];
            Some(vd / w2)
        }
    }

    /// Decomposes the twist into its screw axis and pitch.
    ///
    /// Returns `None` for a pure translation (`‖ω‖ ≈ 0`), which has a degenerate
    /// (infinite-pitch) axis.
    #[inline]
    pub fn axis(&self) -> Option<ScrewAxis> {
        let w = *self.twist.angular().as_array();
        let v = *self.twist.linear().as_array();
        let w2 = w[0] * w[0] + w[1] * w[1] + w[2] * w[2];
        if w2 < 1e-30 {
            return None;
        }
        let theta = w2.sqrt();
        let dir = [w[0] / theta, w[1] / theta, w[2] / theta];
        let vd = v[0] * w[0] + v[1] * w[1] + v[2] * w[2];
        let pitch = vd / w2;
        // q = (ω × v) / ‖ω‖²  (a point on the axis).
        let point = [
            (w[1] * v[2] - w[2] * v[1]) / w2,
            (w[2] * v[0] - w[0] * v[2]) / w2,
            (w[0] * v[1] - w[1] * v[0]) / w2,
        ];
        Some(ScrewAxis {
            direction: dir,
            point,
            pitch: Some(pitch),
        })
    }

    /// The screw motion `exp(ξ·θ)`: a rotation of `θ` radians about the axis
    /// through the screw's point, combined with a translation of `θ·h` along it.
    ///
    /// This is the SE(3) exponential of the twist scaled by `θ`.
    #[inline]
    pub fn motion(&self, theta: f64) -> ScrewMotion {
        let xi = self.twist.to_vector().scale(theta);
        ScrewMotion { se3: SE3::exp(xi) }
    }
}

impl Default for Screw {
    /// The zero twist (no motion).
    fn default() -> Self {
        Self {
            twist: Twist::zeros(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_twist_is_no_motion() {
        let s = Screw::default();
        assert_eq!(s.linear(), [0.0, 0.0, 0.0]);
        assert_eq!(s.angular(), [0.0, 0.0, 0.0]);
        assert_eq!(s.pitch(), None);
        assert_eq!(s.axis(), None);
        // exp(0) == identity
        let id = s.motion(0.0).se3.inverse().compose(s.motion(0.0).se3);
        let t = id.translation();
        assert!((t.as_array()[0]).abs() < 1e-9);
    }

    #[test]
    fn pure_rotation_about_z_has_zero_pitch() {
        // Rotate about the z axis through the origin: ω=(0,0,1), v=0.
        let s = Screw::new([0.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
        let h = s.pitch().unwrap();
        assert!(h.abs() < 1e-12);
        let axis = s.axis().unwrap();
        assert!((axis.direction[2] - 1.0).abs() < 1e-12);
        // Point on axis is the origin.
        assert!(axis.point[0].abs() < 1e-12);
        assert!(axis.point[1].abs() < 1e-12);
        assert!(axis.point[2].abs() < 1e-12);
        // exp(π) about z is a 180° rotation: (1,0,0) -> (-1,0,0).
        let motion = s.motion(std::f64::consts::PI);
        let out = motion.act(Vector3D::new([1.0, 0.0, 0.0]));
        assert!((out.as_array()[0] + 1.0).abs() < 1e-9);
        assert!(out.as_array()[1].abs() < 1e-9);
    }

    #[test]
    fn screw_with_pitch_moves_along_axis() {
        // Rotate about z through the origin with pitch h=1:
        // v = h*ω + q×ω = (0,0,1), ω=(0,0,1).
        let s = Screw::new([0.0, 0.0, 1.0], [0.0, 0.0, 1.0]);
        let h = s.pitch().unwrap();
        assert!((h - 1.0).abs() < 1e-12);
        let axis = s.axis().unwrap();
        assert!((axis.direction[2] - 1.0).abs() < 1e-12);
        // Full turn 2π about z with pitch 1 advances along z by 2π.
        let motion = s.motion(2.0 * std::f64::consts::PI);
        let out = motion.act(Vector3D::new([0.0, 0.0, 0.0]));
        assert!((out.as_array()[2] - 2.0 * std::f64::consts::PI).abs() < 1e-6);
    }

    #[test]
    fn linear_and_angular_parts_are_exposed() {
        let s = Screw::from_array([1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        assert_eq!(s.linear(), [1.0, 2.0, 3.0]);
        assert_eq!(s.angular(), [4.0, 5.0, 6.0]);
        let expected = (4.0_f64 * 4.0_f64 + 5.0_f64 * 5.0_f64 + 6.0_f64 * 6.0_f64).sqrt();
        assert_eq!(s.angular_magnitude(), expected);
    }

    #[test]
    fn pure_translation_has_no_axis() {
        let s = Screw::new([1.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
        assert_eq!(s.pitch(), None);
        assert_eq!(s.axis(), None);
    }

    #[test]
    fn motion_matches_se3_exp() {
        let s = Screw::from_array([0.1, -0.2, 0.3, 0.2, -0.1, 0.4]);
        let theta = 0.5;
        let motion = s.motion(theta);
        // Compare against direct SE3::exp of the scaled twist.
        let xi = Twist::from_array([0.1, -0.2, 0.3, 0.2, -0.1, 0.4])
            .to_vector()
            .scale(theta);
        let expected = SE3::exp(xi);
        let [a, b, c] = *expected.translation().as_array();
        let [x, y, z] = *motion.se3.translation().as_array();
        assert!((a - x).abs() < 1e-9);
        assert!((b - y).abs() < 1e-9);
        assert!((c - z).abs() < 1e-9);
    }
}
