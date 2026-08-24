//! A planetary coordinate reference system: body, frame realization, projection.
//!
//! Mirrors `ptiff::CoordinateReferenceSystem`
//! (see `libptiff/include/ptiff/geometry/coordinate_reference_system.hpp`).

use crate::error::{Error, ErrorCode, Result};
use crate::geometry::{Frame, Planet, Projection};

/// A planetary coordinate reference system: the body, frame realization, and
/// projection.
///
/// Combines a planetary [`Planet`] (identity/shape), an optional
/// [`Frame`] realization override, and a [`Projection`] into one object that
/// fully denotes how a dataset is georeferenced.
///
/// Sprint-1 note (matching the C++ oracle): this is an interface-only stub.
/// [`CoordinateReferenceSystem::identifier`] returns
/// [`ErrorCode::NotImplemented`] until real CRS support exists. The type is
/// `Clone`; copy semantics will be decided once there is real state.
#[derive(Debug, Clone, PartialEq)]
pub struct CoordinateReferenceSystem {
    planet: Planet,
    frame_override: Option<Frame>,
    projection: Projection,
}

impl CoordinateReferenceSystem {
    /// Constructs a CRS from its planet, optional frame override, and projection.
    #[inline]
    pub fn new(planet: Planet, frame: Option<Frame>, projection: Projection) -> Self {
        Self {
            planet,
            frame_override: frame,
            projection,
        }
    }

    /// The planetary body.
    #[inline]
    pub fn planet(&self) -> &Planet {
        &self.planet
    }

    /// The effective frame realization (the override if set, else the body's
    /// canonical IAU default frame).
    #[inline]
    pub fn frame(&self) -> Frame {
        self.frame_override
            .unwrap_or_else(|| self.planet.reference_frame())
    }

    /// The object's declared frame override, if any.
    #[inline]
    pub fn frame_override(&self) -> Option<Frame> {
        self.frame_override
    }

    /// The map projection.
    #[inline]
    pub fn projection(&self) -> &Projection {
        &self.projection
    }

    /// Returns the CRS identifier (e.g. an authority code).
    ///
    /// Sprint-1 stub: returns [`ErrorCode::NotImplemented`](crate::ErrorCode::NotImplemented)
    /// until CRS identifier resolution exists.
    pub fn identifier(&self) -> Result<String> {
        Err(Error::new(
            ErrorCode::NotImplemented,
            "CoordinateReferenceSystem::identifier: not implemented",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn moon_crs() -> CoordinateReferenceSystem {
        CoordinateReferenceSystem::new(
            Planet::new(
                "Moon",
                "301",
                crate::geometry::Ellipsoid::UNSPECIFIED,
                Frame::IAU_MOON,
            ),
            None,
            Projection::new(crate::geometry::ProjectionKind::Equirectangular),
        )
    }

    #[test]
    fn fields_are_exposed() {
        let crs = moon_crs();
        assert_eq!(crs.planet().name(), "Moon");
        assert_eq!(crs.frame(), Frame::IAU_MOON); // canonical default
        assert_eq!(crs.frame_override(), None);
        assert_eq!(
            crs.projection().kind(),
            crate::geometry::ProjectionKind::Equirectangular
        );
    }

    #[test]
    fn frame_override_takes_precedence() {
        let crs = CoordinateReferenceSystem::new(
            Planet::new(
                "Moon",
                "301",
                crate::geometry::Ellipsoid::UNSPECIFIED,
                Frame::IAU_MOON,
            ),
            Some(Frame::new("MOON_ME")),
            Projection::new(crate::geometry::ProjectionKind::Sinusoidal),
        );
        assert_eq!(crs.frame(), Frame::new("MOON_ME"));
        assert_eq!(crs.frame_override(), Some(Frame::new("MOON_ME")));
    }

    #[test]
    fn identifier_is_not_implemented_stub() {
        let crs = moon_crs();
        assert!(matches!(crs.identifier(), Err(e) if e.code() == ErrorCode::NotImplemented));
    }
}
