//! Pinhole-style intrinsic parameters for a camera, in pixels.
//!
//! Mirrors `ptiff::Intrinsics` (see `libptiff/include/ptiff/geometry/intrinsics.hpp`).

/// Pinhole-style intrinsic parameters for a camera, in pixels.
///
/// Describes the image-side characteristics of a sensor: the focal length (in
/// pixel units, per axis) and the principal point (the optical-axis intersect
/// with the image plane, in pixel coordinates). How these values are interpreted
/// (e.g. whether lens distortion is assumed) depends on the owning camera's
/// lens model.
///
/// All values default to `0.0`; a zero focal length does **not** denote an
/// infinite focal length but an "unset" value.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Intrinsics {
    /// Focal length along the horizontal image axis, in pixels.
    pub focal_length_pixels_x: f64,
    /// Focal length along the vertical image axis, in pixels.
    pub focal_length_pixels_y: f64,
    /// Principal point X coordinate, in pixels (column axis).
    pub principal_point_x: f64,
    /// Principal point Y coordinate, in pixels (row axis).
    pub principal_point_y: f64,
}

impl Intrinsics {
    /// An all-zero "unset" intrinsics value.
    pub const ZERO: Self = Self {
        focal_length_pixels_x: 0.0,
        focal_length_pixels_y: 0.0,
        principal_point_x: 0.0,
        principal_point_y: 0.0,
    };

    /// Constructs intrinsics from the focal lengths and principal point,
    /// matching the C++ aggregate init order.
    #[inline]
    pub const fn new(
        focal_length_pixels_x: f64,
        focal_length_pixels_y: f64,
        principal_point_x: f64,
        principal_point_y: f64,
    ) -> Self {
        Self {
            focal_length_pixels_x,
            focal_length_pixels_y,
            principal_point_x,
            principal_point_y,
        }
    }
}

impl Default for Intrinsics {
    /// The all-zero "unset" value — matches the C++ default.
    #[inline]
    fn default() -> Self {
        Self::ZERO
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_zero() {
        let i = Intrinsics::default();
        assert_eq!(i, Intrinsics::ZERO);
    }

    #[test]
    fn new_orders_fields() {
        let i = Intrinsics::new(900.0, 900.0, 512.0, 384.0);
        assert_eq!(
            (i.focal_length_pixels_x, i.focal_length_pixels_y),
            (900.0, 900.0)
        );
        assert_eq!((i.principal_point_x, i.principal_point_y), (512.0, 384.0));
    }
}
