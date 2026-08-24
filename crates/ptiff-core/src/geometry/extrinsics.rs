//! A camera's extrinsic pose: rotation + translation (camera-to-world).
//!
//! Mirrors `ptiff::Extrinsics` (see `libptiff/include/ptiff/geometry/extrinsics.hpp`).

use crate::geometry::{Quaternion, Vec3};

/// A camera's extrinsic pose: its orientation and world position.
///
/// Storage-only value combining a unit [`Quaternion`] rotation and a [`Vec3`]
/// translation. Per GEOMETRY-FOUNDATION.md this **remains the canonical PTIFF
/// representation** (serialised, FFI-stable); an SE(3)/[`crate::geometry::Pose`]
/// view is derived from it when math is needed.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Extrinsics {
    /// Camera-to-world rotation (unit quaternion).
    pub rotation: Quaternion,
    /// Camera position in world coordinates (meters).
    pub translation: Vec3,
}

impl Extrinsics {
    /// The identity extrinsics: no rotation, at the world origin.
    pub const IDENTITY: Self = Self {
        rotation: Quaternion::IDENTITY,
        translation: Vec3::ZERO,
    };

    /// Constructs extrinsics from its rotation and translation.
    #[inline]
    pub const fn new(rotation: Quaternion, translation: Vec3) -> Self {
        Self {
            rotation,
            translation,
        }
    }
}

impl Default for Extrinsics {
    /// Identity extrinsics — matches the C++ default (identity rotation, zero
    /// translation).
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
        let e = Extrinsics::default();
        assert_eq!(e, Extrinsics::IDENTITY);
        assert_eq!(e.rotation, Quaternion::IDENTITY);
        assert_eq!(e.translation, Vec3::ZERO);
    }

    #[test]
    fn new_carries_fields() {
        let rot = Quaternion::new(0.0, 1.0, 0.0, 0.0);
        let t = Vec3::new(1.0, 2.0, 3.0);
        let e = Extrinsics::new(rot, t);
        assert_eq!(e.rotation, rot);
        assert_eq!(e.translation, t);
    }
}
