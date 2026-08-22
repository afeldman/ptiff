//! A biaxial reference ellipsoid for a planetary body.
//!
//! Mirrors `ptiff::Ellipsoid` (see `libptiff/include/ptiff/geometry/ellipsoid.hpp`).

/// A biaxial reference ellipsoid for a planetary body.
///
/// Describes an oblate (or spheroidal) body shape via its equatorial
/// (semi-major) and polar (semi-minor) radii, in meters.
///
/// Both axes zero (`semi_major_axis_meters == semi_minor_axis_meters == 0`)
/// is the degenerate/unknown case and is interpreted as "not specified".
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Ellipsoid {
    /// Equatorial radius of the body, in meters.
    pub semi_major_axis_meters: f64,
    /// Polar radius of the body, in meters.
    pub semi_minor_axis_meters: f64,
}

impl Ellipsoid {
    /// The degenerate, "not specified" ellipsoid (both axes zero).
    pub const UNSPECIFIED: Self = Self {
        semi_major_axis_meters: 0.0,
        semi_minor_axis_meters: 0.0,
    };

    /// Constructs a biaxial ellipsoid from its equatorial and polar radii.
    #[inline]
    pub const fn new(semi_major_axis_meters: f64, semi_minor_axis_meters: f64) -> Self {
        Self {
            semi_major_axis_meters,
            semi_minor_axis_meters,
        }
    }
}

impl Default for Ellipsoid {
    /// The degenerate/unknown ellipsoid.
    #[inline]
    fn default() -> Self {
        Self::UNSPECIFIED
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_unspecified() {
        let e = Ellipsoid::default();
        assert_eq!(e, Ellipsoid::UNSPECIFIED);
        assert_eq!(
            (e.semi_major_axis_meters, e.semi_minor_axis_meters),
            (0.0, 0.0)
        );
    }

    #[test]
    fn moon_reference_ellipsoid() {
        let moon = Ellipsoid::new(1_738_100.0, 1_736_000.0);
        assert_eq!(moon.semi_major_axis_meters, 1_738_100.0);
        assert_eq!(moon.semi_minor_axis_meters, 1_736_000.0);
    }
}
