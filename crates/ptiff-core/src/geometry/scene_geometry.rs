//! A 3D geometric product associated with a scene.
//!
//! Mirrors `ptiff::Geometry` / `ptiff::GeometryKind`
//! (see `libptiff/include/ptiff/geometry.hpp`).

use crate::error::{Error, ErrorCode, Result};
use crate::ImageId;
use std::collections::BTreeMap;

// Note: `Geometry` does not derive serde because it carries an `Option<ImageId>`
// and `Id` has no serde support yet. Serialization of the geometric-product store
// is deferred (GEOMETRY-FOUNDATION.md §7); the kind + named-parameter fields are
// trivially serializable when `Id` gains serde derives.

/// What a [`Geometry`] represents.
///
/// Identifies the class of 3D geometric product carried by a geometry object.
/// No concrete kind exists yet beyond `Unspecified` — a real (if empty) type
/// rather than a reserved namespace, so the enum can grow concrete domain values
/// without breaking callers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum GeometryKind {
    /// No specific kind assigned.
    Unspecified,
}

/// A 3D geometric product associated with a scene (mesh, point cloud,
/// parametric surface), reserved for future extension.
///
/// A `Geometry` is the extension point for 3D geometric data tied to a
/// `crate::Scene` (e.g. a mesh produced by stereo processing). It currently
/// carries a kind and an optional source image, plus an unstructured
/// named-parameter store.
///
/// There is no `id()` accessor — Geometry identity is scoped to whichever
/// `Scene` it was added to.
#[derive(Debug, Clone, PartialEq)]
pub struct Geometry {
    kind: GeometryKind,
    source_image: Option<ImageId>,
    parameters: BTreeMap<String, String>,
}

impl Geometry {
    /// Constructs a geometry object of the given kind with no source image.
    #[inline]
    pub fn new(kind: GeometryKind) -> Self {
        Self {
            kind,
            source_image: None,
            parameters: BTreeMap::new(),
        }
    }

    /// Constructs a geometry object with an optional source image.
    #[inline]
    pub fn from_source(kind: GeometryKind, source_image: Option<ImageId>) -> Self {
        Self {
            kind,
            source_image,
            parameters: BTreeMap::new(),
        }
    }

    /// The kind of geometry.
    #[inline]
    pub fn kind(&self) -> GeometryKind {
        self.kind
    }

    /// The optional source image this geometry was derived from.
    #[inline]
    pub fn source_image(&self) -> Option<ImageId> {
        self.source_image
    }

    /// Looks up a named parameter.
    ///
    /// Returns [`ErrorCode::NotFound`] if `key` was never set.
    pub fn parameter(&self, key: &str) -> Result<&str> {
        self.parameters.get(key).map(String::as_str).ok_or_else(|| {
            Error::new(
                ErrorCode::NotFound,
                format!("Geometry::parameter: no such key `{key}`"),
            )
        })
    }

    /// The number of stored parameters.
    #[inline]
    pub fn len(&self) -> usize {
        self.parameters.len()
    }

    /// Whether this geometry has no parameters.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.parameters.is_empty()
    }

    /// Iterates over `(key, value)` pairs in ascending key order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> + '_ {
        self.parameters
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// Sets a named parameter.
    pub fn set_parameter(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.parameters.insert(key.into(), value.into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_kind_and_no_source() {
        let g = Geometry::new(GeometryKind::Unspecified);
        assert_eq!(g.kind(), GeometryKind::Unspecified);
        assert_eq!(g.source_image(), None);
        assert!(g.is_empty());
    }

    #[test]
    fn parameter_round_trip() {
        let mut g = Geometry::new(GeometryKind::Unspecified);
        g.set_parameter("units", "meters");
        assert_eq!(g.parameter("units").unwrap(), "meters");
        assert!(matches!(g.parameter("nope"), Err(e) if e.code() == ErrorCode::NotFound));
    }

    #[test]
    fn source_image_is_preserved() {
        let g = Geometry::from_source(GeometryKind::Unspecified, Some(ImageId::new(7)));
        assert_eq!(g.source_image(), Some(ImageId::new(7)));
    }

    #[test]
    fn parameters_iterate_sorted() {
        let mut g = Geometry::new(GeometryKind::Unspecified);
        g.set_parameter("b", "2");
        g.set_parameter("a", "1");
        let keys: Vec<_> = g.iter().map(|(k, _)| k).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }
}
