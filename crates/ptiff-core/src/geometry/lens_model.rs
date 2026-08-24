//! Camera lens model: kind plus named distortion parameters.
//!
//! Mirrors `ptiff::LensModel` (see `libptiff/include/ptiff/geometry/lens_model.hpp`).

use crate::error::{Error, ErrorCode, Result};
use std::collections::BTreeMap;

/// Which projection a camera's intrinsics should be interpreted under.
///
/// New kinds are added here as new enum values — never as a new C++ type — so
/// the Go/Python/Rust bindings never need to learn a new type for a new lens
/// model (matching the C++ migration oracle).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum LensModelKind {
    /// Perspective (pinhole) projection. Distortion params optional.
    Pinhole,
    /// Wide-angle fisheye projection.
    Fisheye,
    /// Line-scan (pushbroom) sensor projection.
    Pushbroom,
}

/// Named numeric parameters for a lens model (e.g. `"k1"`/`"k2"`/`"p1"`
/// distortion coefficients).
///
/// Holds a [`LensModelKind`] plus an optional set of named double parameters.
/// Which keys are meaningful depends on `kind()`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LensModel {
    kind: LensModelKind,
    parameters: BTreeMap<String, f64>,
}

impl LensModel {
    /// A lens model of the given kind with no parameters.
    #[inline]
    pub fn new(kind: LensModelKind) -> Self {
        Self {
            kind,
            parameters: BTreeMap::new(),
        }
    }

    /// Returns the lens model kind.
    #[inline]
    pub fn kind(&self) -> LensModelKind {
        self.kind
    }

    /// Looks up a named parameter.
    ///
    /// Returns [`ErrorCode::NotFound`] if `key` was never set.
    pub fn parameter(&self, key: &str) -> Result<f64> {
        self.parameters.get(key).copied().ok_or_else(|| {
            Error::new(
                ErrorCode::NotFound,
                format!("LensModel::parameter: no such key `{key}`"),
            )
        })
    }

    /// The number of stored parameters.
    #[inline]
    pub fn len(&self) -> usize {
        self.parameters.len()
    }

    /// Whether this model has no parameters.
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
        let m = LensModel::new(LensModelKind::Pinhole);
        assert_eq!(m.kind(), LensModelKind::Pinhole);
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn parameter_round_trip() {
        let mut m = LensModel::new(LensModelKind::Pinhole);
        m.set_parameter("k1", -0.1);
        m.set_parameter("k2", 0.05);
        assert_eq!(m.parameter("k1").unwrap(), -0.1);
        assert_eq!(m.parameter("k2").unwrap(), 0.05);
        assert!(matches!(m.parameter("p1"), Err(e) if e.code() == ErrorCode::NotFound));
    }

    #[test]
    fn parameters_iterate_sorted() {
        let mut m = LensModel::new(LensModelKind::Fisheye);
        m.set_parameter("b", 2.0);
        m.set_parameter("a", 1.0);
        let keys: Vec<_> = m.iter().map(|(k, _)| k).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }
}
