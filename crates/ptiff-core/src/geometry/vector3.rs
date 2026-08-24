//! A 3D double-precision vector (position, translation, direction, ...).
//!
//! Mirrors `ptiff::Vec3` (see `libptiff/include/ptiff/geometry/vector3.hpp`).

/// A 3D double-precision vector (position, translation, direction, ...).
///
/// Storage-only value: holds an `x`/`y`/`z` triple with defaulted equality. No
/// linear-algebra operations (addition, cross product, ...) are provided here —
/// the C++ oracle deliberately keeps this type minimal and self-contained; PTIFF
/// geometry math lives in the [`crate::geometry`] wrappers built on `multicalc`.
///
/// The default-initialised value is the zero vector. Value comparison between
/// equal vectors is exact (no epsilon tolerance).
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Vec3 {
    /// X coordinate (right / east).
    pub x: f64,
    /// Y coordinate (down / north).
    pub y: f64,
    /// Z coordinate (into / up).
    pub z: f64,
}

impl Vec3 {
    /// The zero vector `(0, 0, 0)`.
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    /// Constructs a vector from its components, matching the C++ aggregate init
    /// `Vec3{x, y, z}`.
    #[inline]
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /// normalises the vector to unit length, returning a new vector. Returns the zero
    /// vector if the input vector is zero-length.
    #[inline]
    pub fn normalised(self) -> Self {
        let len = (self.x * self.x + self.y * self.y + self.z * self.z).sqrt();
        if len == 0.0 {
            Self::ZERO
        } else {
            Self {
                x: self.x / len,
                y: self.y / len,
                z: self.z / len,
            }
        }
    }
}

impl Default for Vec3 {
    /// The zero vector `(0, 0, 0)` — matches the C++ default-initialised value.
    #[inline]
    fn default() -> Self {
        Self::ZERO
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_zero_vector() {
        let v = Vec3::default();
        assert_eq!(v, Vec3::ZERO);
        assert_eq!(v, Vec3::new(0.0, 0.0, 0.0));
    }

    #[test]
    fn new_orders_components() {
        let v = Vec3::new(1.0, 2.0, 3.0);
        assert_eq!((v.x, v.y, v.z), (1.0, 2.0, 3.0));
    }

    #[test]
    fn equality_is_exact() {
        assert_eq!(Vec3::new(1.0, 2.0, 3.0), Vec3::new(1.0, 2.0, 3.0));
        assert_ne!(Vec3::new(1.0, 2.0, 3.0), Vec3::new(1.0, 2.0, 4.0));
    }
}
