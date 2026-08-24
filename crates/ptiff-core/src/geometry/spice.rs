//! SPICE state mapping onto the PTIFF `multicalc` geometry foundation.
//!
//! A SPICE `state` is a 6‑vector `(position, velocity)` for the translation and
//! a quaternion + angular velocity for the attitude. This module maps that data
//! onto the existing [`Pose`](crate::geometry::Pose) (SE(3) + frames) and
//! [`Screw`](crate::geometry::Screw) / `Twist` (se(3) velocity) machinery without
//! pulling in any CSPICE bindings:
//!
//! ```text
//!  SPICE state
//!    position p   ──►  SE(3) translation
//!    orientation q──►  SE(3) rotation
//!    velocity v   ──►  Twist.linear   (body or spatial)
//!    angular  ω   ──►  Twist.angular
//!          │
//!          ▼
//!    Pose (p, q)  +  Twist (v, ω)
//!          │             │
//!          │      SE3::adjoint ─► body/spatial velocity transform
//!          └─► SE3::interpolate ─► propagation over Δt
//! ```
//!
//! CSPICE itself stays an optional, later-phase FFI concern; this is the
//! **mathematical mapping layer** only (GEOMETRY-FOUNDATION.md §6).

use crate::geometry::{FramePair, Pose, Quaternion, Vec3};
use crate::{Error, Result};
use multicalc::linear_algebra::Vector3D;
use multicalc::spatial::{Twist, SE3};

/// A spacecraft/physical state as SPICE models it: a pose (`position` +
/// `orientation`) together with a velocity (`linear` + `angular`), expressed in
/// one reference [`FramePair`].
///
/// Units follow SPICE convention: `position` in meters, `orientation` a unit
/// quaternion, `linear` velocity in m/s and `angular` velocity in rad/s.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpiceState {
    /// Position of the body origin in the `from` frame, meters.
    pub position: Vec3,
    /// Orientation of the body frame relative to the `from` frame.
    pub orientation: Quaternion,
    /// Linear velocity of the body origin in the `from` frame, m/s.
    pub linear_velocity: Vec3,
    /// Angular velocity of the body in the `from` frame, rad/s.
    pub angular_velocity: Vec3,
}

impl SpiceState {
    /// Constructs a SPICE state from its parts.
    #[must_use]
    pub const fn new(
        position: Vec3,
        orientation: Quaternion,
        linear_velocity: Vec3,
        angular_velocity: Vec3,
    ) -> Self {
        Self {
            position,
            orientation,
            linear_velocity,
            angular_velocity,
        }
    }

    /// Builds a state from a [`Pose`] combined with a velocity [`Twist`].
    ///
    /// The twist's linear part becomes `linear_velocity` and its angular part
    /// `angular_velocity`. The pose supplies `position`/`orientation`; its frame
    /// pair is discarded here (the state itself declares no frames).
    #[must_use]
    pub fn from_pose_and_twist(pose: &Pose, twist: &Twist<f64>) -> Self {
        let l = *twist.linear().as_array();
        let a = *twist.angular().as_array();
        Self {
            position: pose.translation(),
            orientation: pose.rotation(),
            linear_velocity: Vec3::new(l[0], l[1], l[2]),
            angular_velocity: Vec3::new(a[0], a[1], a[2]),
        }
    }

    /// The pose part of this state (position + orientation) with explicit frames.
    pub fn pose(&self, frames: FramePair) -> Result<Pose> {
        Ok(Pose::from_parts(self.orientation, self.position, frames))
    }

    /// The velocity part as a `multicalc` [`Twist`] `[linear; angular]`.
    #[must_use]
    pub fn twist(&self) -> Twist<f64> {
        Twist::new(
            Vector3D::new([
                self.linear_velocity.x,
                self.linear_velocity.y,
                self.linear_velocity.z,
            ]),
            Vector3D::new([
                self.angular_velocity.x,
                self.angular_velocity.y,
                self.angular_velocity.z,
            ]),
        )
    }

    /// The velocity part as a PTIFF [`Screw`] (twist wrapper).
    #[must_use]
    pub fn screw(&self) -> crate::geometry::Screw {
        crate::geometry::Screw::from_twist(self.twist())
    }

    /// Propagates this state forward by `dt` seconds along its (assumed-inertial)
    /// velocity, returning the new state.
    ///
    /// Translation advances linearly (`p' = p + v·dt`). Orientation advances by
    /// the incremental rotation `exp(ω·dt)` in the world frame, using the same
    /// axis/angle decomposition `multicalc` uses for `SE3::exp` of a pure angular
    /// twist (constant-rate, small-step approximation appropriate for the short
    /// integration steps SPICE states are sampled at).
    #[must_use]
    pub fn propagate(&self, dt: f64, _frames: FramePair) -> SpiceState {
        // New translation: p + v·dt.
        let new_p = Vec3::new(
            self.position.x + self.linear_velocity.x * dt,
            self.position.y + self.linear_velocity.y * dt,
            self.position.z + self.linear_velocity.z * dt,
        );
        // Incremental rotation from the angular velocity (world-frame).
        let w = Vector3D::new([
            self.angular_velocity.x,
            self.angular_velocity.y,
            self.angular_velocity.z,
        ]);
        let dq = multicalc::spatial::Quaternion::from_scaled_axis(w * dt);
        // q' = dq ⊗ q  (rotate world, then apply existing orientation).
        let cur = multicalc::spatial::Quaternion::new(
            self.orientation.w,
            self.orientation.x,
            self.orientation.y,
            self.orientation.z,
        );
        let q_combined = dq * cur;
        let new_q = Quaternion::new(
            q_combined.w(),
            q_combined.x(),
            q_combined.y(),
            q_combined.z(),
        );
        // Velocity is constant under a constant twist (no acceleration modelled).
        Self {
            position: new_p,
            orientation: new_q,
            linear_velocity: self.linear_velocity,
            angular_velocity: self.angular_velocity,
        }
    }

    /// Propagates via the SE(3) geodesic screw motion: the new pose is the
    /// current pose composed with `exp([v; ω]·dt)`, so a helical (screwing)
    /// trajectory is followed rather than a separate linear-translation /
    /// axis-angle-rotation pair. Velocity is left unchanged (constant twist).
    #[must_use]
    pub fn propagate_screw(&self, dt: f64, frames: FramePair) -> SpiceState {
        let t = self.twist().to_vector() * dt;
        let dse3 = SE3::exp(t);
        let pose = self.pose(frames).unwrap_or_else(|_| Pose::identity(frames));
        let new_pose = Pose::from_se3(dse3 * pose.to_se3(), frames);
        Self {
            position: new_pose.translation(),
            orientation: new_pose.rotation(),
            linear_velocity: self.linear_velocity,
            angular_velocity: self.angular_velocity,
        }
    }

    /// Transforms the velocity part into another pose's frame using the SE(3)
    /// adjoint: `twist' = Ad(pose) · twist`. This is the body↔spatial velocity
    /// chain rule (GEOMETRY-FOUNDATION.md §3, §6).
    ///
    /// Position and orientation are unchanged; only the velocity components are
    /// rewritten. The operation is infallible (the SE(3) adjoint always exists),
    /// so the returned `Result` is always `Ok`.
    pub fn transform_velocity_by(&self, pose: &Pose) -> Result<Self> {
        let adj = pose.adjoint();
        let v = adj * self.twist().to_vector();
        let a = *v.as_array();
        Ok(Self {
            position: self.position,
            orientation: self.orientation,
            linear_velocity: Vec3::new(a[0], a[1], a[2]),
            angular_velocity: Vec3::new(a[3], a[4], a[5]),
        })
    }
}

impl Default for SpiceState {
    /// Zero position/velocity, identity orientation — the initial rest state.
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            orientation: Quaternion::IDENTITY,
            linear_velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
        }
    }
}

impl TryFrom<&Pose> for SpiceState {
    type Error = Error;

    /// Extracts the pose part (position + orientation) as a state with zero
    /// velocity. Frame pairs are not carried by the state.
    fn try_from(pose: &Pose) -> Result<Self> {
        Ok(SpiceState {
            position: pose.translation(),
            orientation: pose.rotation(),
            linear_velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{Frame, FramePair, Screw};

    fn j2000_to_body() -> FramePair {
        FramePair::new(Frame::J2000, Frame::SPACECRAFT)
    }

    #[test]
    fn default_state_is_rest() {
        let s = SpiceState::default();
        assert_eq!(s.position, Vec3::ZERO);
        assert_eq!(s.orientation, Quaternion::IDENTITY);
        assert_eq!(s.linear_velocity, Vec3::ZERO);
        assert_eq!(s.angular_velocity, Vec3::ZERO);
    }

    #[test]
    fn pose_round_trips_position_and_orientation() {
        let s = SpiceState::new(
            Vec3::new(1.0, 2.0, 3.0),
            Quaternion::from_euler_angles(0.1, 0.2, 0.3),
            Vec3::ZERO,
            Vec3::ZERO,
        );
        let p = s.pose(j2000_to_body()).unwrap();
        assert!((p.translation().x - 1.0).abs() < 1e-12);
        assert!((p.translation().y - 2.0).abs() < 1e-12);
        assert!((p.translation().z - 3.0).abs() < 1e-12);
        assert_eq!(p.rotation(), s.orientation);
    }

    #[test]
    fn twist_round_trips_velocity() {
        let s = SpiceState::new(
            Vec3::ZERO,
            Quaternion::IDENTITY,
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        );
        let t = s.twist();
        assert_eq!(*t.linear().as_array(), [1.0, 0.0, 0.0]);
        assert_eq!(*t.angular().as_array(), [0.0, 1.0, 0.0]);
        let s2 = SpiceState::from_pose_and_twist(&Pose::identity(j2000_to_body()), &t);
        assert!((s2.linear_velocity.x - 1.0).abs() < 1e-15);
        assert!((s2.angular_velocity.y - 1.0).abs() < 1e-15);
    }

    #[test]
    fn screw_wraps_twist() {
        let s = SpiceState::new(
            Vec3::ZERO,
            Quaternion::IDENTITY,
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.0, 0.0, 2.0),
        );
        let sc = s.screw();
        assert_eq!(s.twist(), *sc.to_twist());
        assert!(sc.pitch().is_some());
    }

    #[test]
    fn propagate_advances_constant_velocity() {
        let s = SpiceState::new(
            Vec3::new(1.0, 0.0, 0.0),
            Quaternion::IDENTITY,
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::ZERO,
        );
        let s2 = s.propagate(2.0, j2000_to_body());
        assert!((s2.position.x - 3.0).abs() < 1e-12);
        assert_eq!(s2.orientation, Quaternion::IDENTITY);
        assert_eq!(s2.linear_velocity, s.linear_velocity);
    }

    #[test]
    fn propagate_rotates_by_world_angular_velocity() {
        // After dt=π/2 about +x the orientation should be a 90° rotation about x.
        let s = SpiceState::new(
            Vec3::ZERO,
            Quaternion::IDENTITY,
            Vec3::ZERO,
            Vec3::new(1.0, 0.0, 0.0),
        );
        let s2 = s.propagate(std::f64::consts::FRAC_PI_2, j2000_to_body());
        let q = s2.orientation;
        let d = 2.0_f64.sqrt() / 2.0;
        assert!((q.w - d).abs() < 1e-12);
        assert!((q.x - d).abs() < 1e-12);
        assert!(q.y.abs() < 1e-12);
        assert!(q.z.abs() < 1e-12);
    }

    #[test]
    fn propagate_screw_follows_helix() {
        // Screw along +z with rotation about +z through the origin. For v=1, ω=2,
        // the pitch is h = (v·ω)/‖ω‖² = 2/4 = 0.5. Over dt = π/2 the rotation
        // angle is ‖ω‖·dt = π (a half-turn) and the advance along the axis is
        // h·π = 0.5π.
        let s = SpiceState::new(
            Vec3::ZERO,
            Quaternion::IDENTITY,
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.0, 0.0, 2.0),
        );
        let s2 = s.propagate_screw(std::f64::consts::FRAC_PI_2, j2000_to_body());
        assert!((s2.position.z - std::f64::consts::PI / 2.0).abs() < 1e-9);
        // Orientation is a half-turn about ±z: w ≈ 0, |z| ≈ 1.
        let q = s2.orientation;
        assert!(q.w.abs() < 1e-9 && q.z.abs() > 0.999);
    }

    #[test]
    fn transform_velocity_by_adjoint_matches_body_frame() {
        // Pose = 90° about +x. A body-frame velocity along +z maps to spatial +y.
        let pose = Pose::from_parts(
            Quaternion::from_euler_angles(std::f64::consts::FRAC_PI_2, 0.0, 0.0),
            Vec3::ZERO,
            j2000_to_body(),
        );
        let s = SpiceState::new(
            Vec3::ZERO,
            Quaternion::IDENTITY,
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.0, 0.0, 1.0),
        );
        let out = s.transform_velocity_by(&pose).unwrap();
        assert!(out.linear_velocity.y.abs() > 0.999);
        assert!(out.linear_velocity.z.abs() < 1e-9);
        assert!(out.angular_velocity.y.abs() > 0.999);
        assert_eq!(out.position, s.position);
        assert_eq!(out.orientation, s.orientation);
    }

    #[test]
    fn try_from_pose_gives_zero_velocity() {
        let p = Pose::from_parts(
            Quaternion::IDENTITY,
            Vec3::new(5.0, 6.0, 7.0),
            j2000_to_body(),
        );
        let s = SpiceState::try_from(&p).unwrap();
        assert_eq!(s.position, Vec3::new(5.0, 6.0, 7.0));
        assert_eq!(s.orientation, Quaternion::IDENTITY);
        assert_eq!(s.linear_velocity, Vec3::ZERO);
        assert_eq!(s.angular_velocity, Vec3::ZERO);
    }

    #[test]
    fn screw_smoke() {
        let _s = Screw::new([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    }
}
