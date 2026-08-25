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

/// Canonical lowercase string for a [`LensModelKind`], used as the value of the
/// `ptiff.camera.lens_kind` extension field.
#[must_use]
pub fn lens_model_kind_str(kind: LensModelKind) -> &'static str {
    match kind {
        LensModelKind::Pinhole => "pinhole",
        LensModelKind::Fisheye => "fisheye",
        LensModelKind::Pushbroom => "pushbroom",
    }
}

/// Parses a canonical [`LensModelKind`] string back into its enum value.
///
/// Unknown strings map to [`LensModelKind::Pinhole`] (the least-specific
/// default), mirroring the forward-compatibility convention used elsewhere in
/// the extension domains: a foreign/future writer's unhandled kind is tolerated
/// rather than failing the file.
#[must_use]
pub fn lens_model_kind_from_str(s: &str) -> LensModelKind {
    match s {
        "fisheye" => LensModelKind::Fisheye,
        "pushbroom" => LensModelKind::Pushbroom,
        _ => LensModelKind::Pinhole,
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

    #[test]
    fn lens_kind_str_round_trips() {
        assert_eq!(lens_model_kind_str(LensModelKind::Pinhole), "pinhole");
        assert_eq!(lens_model_kind_str(LensModelKind::Fisheye), "fisheye");
        assert_eq!(lens_model_kind_str(LensModelKind::Pushbroom), "pushbroom");
        // Canonical strings parse back to the matching kind.
        assert_eq!(lens_model_kind_from_str("pinhole"), LensModelKind::Pinhole);
        assert_eq!(lens_model_kind_from_str("fisheye"), LensModelKind::Fisheye);
        assert_eq!(
            lens_model_kind_from_str("pushbroom"),
            LensModelKind::Pushbroom
        );
    }

    #[test]
    fn unknown_lens_kind_falls_back_to_pinhole() {
        assert_eq!(
            lens_model_kind_from_str("unknown-future-kind"),
            LensModelKind::Pinhole
        );
    }
}
