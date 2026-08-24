//! A planetary body: identity and shape.
//!
//! Mirrors `ptiff::Planet` (see `libptiff/include/ptiff/geometry/planet.hpp`).

use crate::geometry::{Ellipsoid, Frame};
use std::cmp::Ordering;
use std::fmt;

/// A planetary body: identity and shape.
///
/// Describes one body that PTIFF datasets may be georeferenced against (Earth,
/// the Moon, Mars, ...). A `Planet` is an immutable value object assembled from
/// its name, IAU identifier, reference [`Ellipsoid`], and canonical IAU default
/// reference frame.
///
/// New bodies are just new `Planet` instances — there is deliberately **no
/// enum-of-planets** to extend, so adding a body is a runtime concern rather
/// than a library-code change (matching the C++ oracle).
#[derive(Debug, Clone, PartialEq)]
pub struct Planet {
    name: String,
    iau_identifier: String,
    ellipsoid: Ellipsoid,
    reference_frame: Frame,
}

impl Planet {
    /// Constructs a planetary body.
    ///
    /// - `name`: human-readable body name (e.g. `"Moon"`).
    /// - `iau_id`: IAU body identifier (e.g. `"301"` for the Moon).
    /// - `ellipsoid`: reference ellipsoid describing the shape.
    /// - `reference_frame`: canonical IAU default frame (e.g. `"IAU_MOON"`).
    #[inline]
    pub fn new(
        name: impl Into<String>,
        iau_id: impl Into<String>,
        ellipsoid: Ellipsoid,
        reference_frame: Frame,
    ) -> Self {
        Self {
            name: name.into(),
            iau_identifier: iau_id.into(),
            ellipsoid,
            reference_frame,
        }
    }

    /// The human-readable body name (e.g. `"Moon"`).
    #[inline]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The IAU body identifier (e.g. `"301"` for the Moon).
    #[inline]
    pub fn iau_identifier(&self) -> &str {
        &self.iau_identifier
    }

    /// The reference ellipsoid describing the body's shape.
    #[inline]
    pub fn ellipsoid(&self) -> Ellipsoid {
        self.ellipsoid
    }

    /// The body's canonical IAU default reference frame (e.g. `"IAU_MOON"`).
    #[inline]
    pub fn reference_frame(&self) -> Frame {
        self.reference_frame
    }
}

impl fmt::Display for Planet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl PartialOrd for Planet {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Planet {
    fn cmp(&self, other: &Self) -> Ordering {
        self.name.cmp(&other.name)
    }
}

impl Eq for Planet {}

#[cfg(test)]
mod tests {
    use super::*;

    fn moon() -> Planet {
        Planet::new(
            "Moon",
            "301",
            Ellipsoid::new(1_738_100.0, 1_736_000.0),
            Frame::IAU_MOON,
        )
    }

    #[test]
    fn fields_are_exposed() {
        let m = moon();
        assert_eq!(m.name(), "Moon");
        assert_eq!(m.iau_identifier(), "301");
        assert_eq!(m.ellipsoid(), Ellipsoid::new(1_738_100.0, 1_736_000.0));
        assert_eq!(m.reference_frame(), Frame::IAU_MOON);
    }

    #[test]
    fn equality_is_by_fields() {
        assert_eq!(moon(), moon());
        let mars = Planet::new(
            "Mars",
            "499",
            Ellipsoid::new(3_396_190.0, 3_376_200.0),
            Frame::new("IAU_MARS"),
        );
        assert_ne!(moon(), mars);
    }

    #[test]
    fn ordering_is_by_name() {
        let earth = Planet::new("Earth", "399", Ellipsoid::UNSPECIFIED, Frame::IAU_EARTH);
        assert!(moon() > earth);
    }
}
