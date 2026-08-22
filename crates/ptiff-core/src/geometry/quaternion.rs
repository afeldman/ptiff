//! A rotation represented as a unit quaternion (w, x, y, z).
//!
//! Mirrors `ptiff::Quaternion` (see `libptiff/include/ptiff/geometry/quaternion.hpp`).
//! Convenience conversions to/from intrinsic **ZYX Euler angles** (radians and
//! degrees) and to an `angle + axis` are provided on top of the storage value,
//! delegating the underlying trig to `multicalc`.

/// A rotation represented as a unit quaternion (w, x, y, z).
///
/// Storage value: holds four double components and provides defaulted equality.
/// Unlike a [`crate::geometry::Vec3`], **no** normalisation is performed on
/// construction — a caller is responsible for supplying a unit quaternion.
/// Composition/inversion operations are not implemented here; PTIFF geometry
/// math lives in the [`crate::geometry`] wrappers built on `multicalc`.
///
/// The Euler-angle helpers follow the **ZYX intrinsic** convention
/// `R = Rz(yaw)·Ry(pitch)·Rx(roll)` — the same convention `multicalc` uses, so
/// these convert 1:1 with no reordering.
///
/// ## Convention
/// The real part is stored first (`w`), followed by the imaginary parts
/// (`x`, `y`, `z`) — the Hamiltonian convention `q = w + x·i + y·j + z·k`. This
/// is also the ordering used by `multicalc`'s `Quaternion` (`[w, x, y, z]`),
/// so 1:1 conversions require no component reordering.
///
/// The default value is the identity quaternion `(1, 0, 0, 0)`.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Quaternion {
    /// Real part.
    pub w: f64,
    /// Imaginary component of the x axis.
    pub x: f64,
    /// Imaginary component of the y axis.
    pub y: f64,
    /// Imaginary component of the z axis.
    pub z: f64,
}

impl Quaternion {
    /// The identity quaternion `(1, 0, 0, 0)`.
    pub const IDENTITY: Self = Self {
        w: 1.0,
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    /// Constructs a quaternion from its components, matching the C++ aggregate
    /// init `Quaternion{w, x, y, z}`.
    #[inline]
    pub const fn new(w: f64, x: f64, y: f64, z: f64) -> Self {
        Self { w, x, y, z }
    }

    /// Constructs a unit quaternion from intrinsic **ZYX** Euler angles in
    /// **radians**: `R = Rz(yaw)·Ry(pitch)·Rx(roll)`.
    ///
    /// This is the constructor counterpart of [`Quaternion::to_rad`]. For input
    /// in degrees use [`Quaternion::from_euler_angles_deg`].
    #[must_use]
    pub fn from_euler_angles(roll: f64, pitch: f64, yaw: f64) -> Self {
        to_ptiff(multicalc::spatial::Quaternion::from_euler_zyx(
            roll, pitch, yaw,
        ))
    }

    /// Constructs a unit quaternion from intrinsic **ZYX** Euler angles in
    /// **degrees**: `R = Rz(yaw)·Ry(pitch)·Rx(roll)`.
    #[must_use]
    pub fn from_euler_angles_deg(roll_deg: f64, pitch_deg: f64, yaw_deg: f64) -> Self {
        Self::from_euler_angles(
            roll_deg.to_radians(),
            pitch_deg.to_radians(),
            yaw_deg.to_radians(),
        )
    }

    /// Converts this quaternion to its intrinsic **ZYX** Euler angles in
    /// **radians** as `(roll, pitch, yaw)` — the inverse of
    /// [`Quaternion::from_euler_angles`].
    ///
    /// This is the `quaternion2rad` helper: it gives the rotation-angle
    /// representation of the unit quaternion in radian measure. At the gimbal
    /// lock poles (`pitch = ±π/2`) a canonical `roll = 0` split is returned.
    #[must_use]
    pub fn to_rad(self) -> (f64, f64, f64) {
        from_ptiff(self).to_euler_zyx()
    }

    /// Alias for [`Quaternion::to_rad`]; converts this quaternion to its ZYX
    /// Euler angles in **radians**.
    #[must_use]
    pub fn quaternion2rad(self) -> (f64, f64, f64) {
        self.to_rad()
    }

    /// Converts this quaternion to its intrinsic **ZYX** Euler angles in
    /// **degrees** as `(roll_deg, pitch_deg, yaw_deg)`.
    #[must_use]
    pub fn to_deg(self) -> (f64, f64, f64) {
        let (r, p, y) = self.to_rad();
        (r.to_degrees(), p.to_degrees(), y.to_degrees())
    }

    /// Decomposes this quaternion into an `(axis, angle)` pair, where `angle`
    /// is in **radians**. Assumes a unit quaternion.
    #[must_use]
    pub fn to_angle_axis(self) -> (crate::geometry::Vec3, f64) {
        let (axis, angle) = from_ptiff(self).to_axis_angle();
        let a = *axis.as_array();
        (crate::geometry::Vec3::new(a[0], a[1], a[2]), angle)
    }
}

impl Default for Quaternion {
    /// The identity quaternion `(1, 0, 0, 0)` — matches the C++ default value.
    #[inline]
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// Converts a `multicalc` quaternion (already `[w, x, y, z]`) to a PTIFF one.
#[inline]
fn to_ptiff(q: multicalc::spatial::Quaternion<f64>) -> Quaternion {
    Quaternion::new(q.w(), q.x(), q.y(), q.z())
}

/// Converts a PTIFF quaternion to its `multicalc` counterpart.
#[inline]
fn from_ptiff(q: Quaternion) -> multicalc::spatial::Quaternion<f64> {
    multicalc::spatial::Quaternion::new(q.w, q.x, q.y, q.z)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_identity() {
        let q = Quaternion::default();
        assert_eq!(q, Quaternion::IDENTITY);
        assert_eq!(q, Quaternion::new(1.0, 0.0, 0.0, 0.0));
    }

    #[test]
    fn new_orders_components() {
        let q = Quaternion::new(0.0, 1.0, 0.0, 0.0); // 180° about +x
        assert_eq!((q.w, q.x, q.y, q.z), (0.0, 1.0, 0.0, 0.0));
    }

    #[test]
    fn equality_is_exact() {
        assert_eq!(
            Quaternion::new(0.0, 1.0, 0.0, 0.0),
            Quaternion::new(0.0, 1.0, 0.0, 0.0)
        );
        assert_ne!(
            Quaternion::new(1.0, 0.0, 0.0, 0.0),
            Quaternion::new(0.0, 1.0, 0.0, 0.0)
        );
    }

    #[test]
    fn identity_has_zero_euler_angles() {
        let (r, p, y) = Quaternion::IDENTITY.to_rad();
        assert!((r.abs() + p.abs() + y.abs()) < 1e-12);
        let (r, p, y) = Quaternion::IDENTITY.to_deg();
        assert!((r.abs() + p.abs() + y.abs()) < 1e-9);
    }

    #[test]
    fn from_and_to_rad_round_trip() {
        let q = Quaternion::from_euler_angles(0.3, -0.7, 1.2);
        let (r, p, y) = q.to_rad();
        let back = Quaternion::from_euler_angles(r, p, y);
        // Unit quaternions q and -q encode the same rotation; compare components
        // with sign-insensitive closeness.
        let a = (q.w, q.x, q.y, q.z);
        let b = (back.w, back.x, back.y, back.z);
        let dist = (a.0 - b.0).abs() + (a.1 - b.1).abs() + (a.2 - b.2).abs() + (a.3 - b.3).abs();
        let dist_neg =
            (a.0 + b.0).abs() + (a.1 + b.1).abs() + (a.2 + b.2).abs() + (a.3 + b.3).abs();
        assert!(dist.min(dist_neg) < 1e-12);
    }

    #[test]
    fn pure_rotation_about_x() {
        // 90° rotation about +x: q = (cos45°, sin45°, 0, 0).
        let q = Quaternion::from_euler_angles(std::f64::consts::FRAC_PI_2, 0.0, 0.0);
        let d = 2.0_f64.sqrt() / 2.0;
        assert!((q.w - d).abs() < 1e-12);
        assert!((q.x - d).abs() < 1e-12);
        assert!(q.y.abs() < 1e-12);
        assert!(q.z.abs() < 1e-12);
        let (r, p, y) = q.to_rad();
        assert!((r - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        assert!(p.abs() < 1e-12);
        assert!(y.abs() < 1e-12);
    }

    #[test]
    fn radians_and_degrees_agree() {
        const DEG: [f64; 3] = [30.0, -45.0, 120.0];
        let q_deg = Quaternion::from_euler_angles_deg(DEG[0], DEG[1], DEG[2]);
        let q_rad = Quaternion::from_euler_angles(
            DEG[0].to_radians(),
            DEG[1].to_radians(),
            DEG[2].to_radians(),
        );
        assert!(
            (q_deg.w - q_rad.w).abs() < 1e-15
                && (q_deg.x - q_rad.x).abs() < 1e-15
                && (q_deg.y - q_rad.y).abs() < 1e-15
                && (q_deg.z - q_rad.z).abs() < 1e-15
        );
        let (r, p, y) = q_deg.to_deg();
        assert!((r - DEG[0]).abs() < 1e-9);
        assert!((p - DEG[1]).abs() < 1e-9);
        assert!((y - DEG[2]).abs() < 1e-9);
    }

    #[test]
    fn quaternion2rad_is_an_alias_for_to_rad() {
        let q = Quaternion::from_euler_angles(0.1, 0.2, 0.3);
        assert_eq!(q.quaternion2rad(), q.to_rad());
    }

    #[test]
    fn angle_axis_round_trip() {
        let q = Quaternion::from_euler_angles(0.5, -0.3, 0.8);
        let (axis, angle) = q.to_angle_axis();
        // Unit axis.
        let n = (axis.x * axis.x + axis.y * axis.y + axis.z * axis.z).sqrt();
        assert!((n - 1.0).abs() < 1e-12);
        // Rebuild via half-angle formula and compare (sign-insensitive).
        let half = angle / 2.0;
        let (s, c) = half.sin_cos();
        let back = Quaternion::new(c, axis.x * s, axis.y * s, axis.z * s);
        assert!(
            ((q.w - back.w).abs()
                + (q.x - back.x).abs()
                + (q.y - back.y).abs()
                + (q.z - back.z).abs())
            .min(
                (q.w + back.w).abs()
                    + (q.x + back.x).abs()
                    + (q.y + back.y).abs()
                    + (q.z + back.z).abs(),
            ) < 1e-12
        );
    }
}
