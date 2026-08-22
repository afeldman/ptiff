//! A rotation represented as a unit quaternion (w, x, y, z).
//!
//! Mirrors `ptiff::Quaternion` (see `libptiff/include/ptiff/geometry/quaternion.hpp`).

/// A rotation represented as a unit quaternion (w, x, y, z).
///
/// Storage-only value: holds four double components and provides defaulted
/// equality. Unlike a [`crate::geometry::Vec3`], **no** normalisation is
/// performed on construction — a caller is responsible for supplying a unit
/// quaternion. Composition/inversion operations are not implemented here; PTIFF
/// geometry math lives in the [`crate::geometry`] wrappers built on `multicalc`.
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
}

impl Default for Quaternion {
    /// The identity quaternion `(1, 0, 0, 0)` — matches the C++ default value.
    #[inline]
    fn default() -> Self {
        Self::IDENTITY
    }
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
}
