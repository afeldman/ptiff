//! Map projection types for a coordinate reference system.
//!
//! Mirrors `ptiff::Projection` / `ptiff::ProjectionKind`
//! (see `libptiff/include/ptiff/geometry/projection.hpp`).

use crate::error::{Error, ErrorCode, Result};
use std::collections::BTreeMap;

/// Which map projection a coordinate reference system uses.
///
/// New kinds are added here as new enum values — never as a new C++ type —
/// matching `LensModelKind`'s and `ScientificLayer`'s extension pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum ProjectionKind {
    /// Equirectangular (plate carrée) projection.
    Equirectangular,
    /// Stereographic projection.
    Stereographic,
    /// Sinusoidal (Sanson–Flamsteed) projection.
    Sinusoidal,
    /// Azimuthal orthographic projection.
    Orthographic,
}

/// Named parameters for a projection (e.g. `"central_meridian"`,
/// `"standard_parallel"`).
///
/// Holds a [`ProjectionKind`] plus an optional set of named double parameters.
/// Which keys are meaningful depends on `kind()`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Projection {
    kind: ProjectionKind,
    parameters: BTreeMap<String, f64>,
}

impl Projection {
    /// A projection of the given kind with no parameters.
    #[inline]
    pub fn new(kind: ProjectionKind) -> Self {
        Self {
            kind,
            parameters: BTreeMap::new(),
        }
    }

    /// Returns the projection kind.
    #[inline]
    pub fn kind(&self) -> ProjectionKind {
        self.kind
    }

    /// Looks up a named parameter.
    ///
    /// Returns [`ErrorCode::NotFound`] if `key` was never set.
    pub fn parameter(&self, key: &str) -> Result<f64> {
        self.parameters.get(key).copied().ok_or_else(|| {
            Error::new(
                ErrorCode::NotFound,
                format!("Projection::parameter: no such key `{key}`"),
            )
        })
    }

    /// The number of stored parameters.
    #[inline]
    pub fn len(&self) -> usize {
        self.parameters.len()
    }

    /// Whether this projection has no parameters.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.parameters.is_empty()
    }

    /// Iterates over `(key, value)` parameter pairs in ascending key order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, f64)> + '_ {
        self.parameters.iter().map(|(k, v)| (k.as_str(), *v))
    }

    /// Sets a named parameter.
    pub fn set_parameter(&mut self, key: impl Into<String>, value: f64) {
        self.parameters.insert(key.into(), value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_kind_and_no_parameters() {
        let p = Projection::new(ProjectionKind::Equirectangular);
        assert_eq!(p.kind(), ProjectionKind::Equirectangular);
        assert!(p.is_empty());
    }

    #[test]
    fn parameter_round_trip() {
        let mut p = Projection::new(ProjectionKind::Equirectangular);
        p.set_parameter("central_meridian", 0.0);
        assert_eq!(p.parameter("central_meridian").unwrap(), 0.0);
        assert!(
            matches!(p.parameter("standard_parallel"), Err(e) if e.code() == ErrorCode::NotFound)
        );
    }
}
