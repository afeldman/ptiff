//! PTIFF identity semantics contract (P0-05).
//!
//! This module pins down **what identity means in PTIFF** without deciding how
//! identifiers are written to disk. The frozen architecture (ADR-010) requires:
//!
//! ```text
//! PTIFF-local identity          (identifies an entity within one Scene)
//!         !=
//! external/persistent identity  (identifies the same entity in an outside system)
//!         !=
//! physical TIFF identity        (IFD order, offsets, tile layout, file name)
//! ```
//!
//! and the companion separations:
//!
//! ```text
//! identity semantics  !=  serialization syntax   (deferred, ADR-003/ADR-010 detail)
//! identity semantics  !=  manifest encoding      (deferred, ADR-011)
//! identity semantics  !=  manifest location      (deferred, ADR-012)
//! ```
//!
//! # Contract
//!
//! * **Local identifiers** are the typed [`crate::id::Id`] handles minted by a
//!   [`crate::Scene`] (`ImageId`, `CameraId`, `GeometryId`, ...). They are
//!   opaque per-issuer tokens: unique within the Scene that issued them,
//!   meaningless across Scenes, and never derived from TIFF layout.
//! * **External identifiers** ([`ExternalId`]) optionally point at the same
//!   entity in an external system (PDS4, ISIS, mission archives, source
//!   datasets). They are references, not PTIFF-native identity: nothing about
//!   a PTIFF-local id is validated against, derived from, or required by an
//!   external id.
//! * **Physical identity** (IFD index, array position, offsets, file name) is
//!   a presentation property of the 1.x container, not scientific identity.
//!   The typed `Scene` layer may currently *implement* lookups positionally
//!   (see [`crate::Scene`]), but no scientific claim may be built on it.
//!
//! # Deferred on purpose
//!
//! No lexical syntax for identifiers is defined here (no UUID/ULID/URI
//! commitment), no namespace registry exists, and nothing here serializes to
//! the manifest (ADR-011/ADR-012 are unresolved). In-memory attachment of
//! [`ExternalId`] to an image is available now; its on-disk representation is
//! a later manifest decision.

use crate::{Error, Result};

/// An identifier of an entity in an external system.
///
/// `ExternalId` is deliberately **not** a PTIFF-local identifier:
///
/// * [`ExternalId::namespace`] names the external authority/system that owns
///   the identifier (for example `"pds4"` for a PDS4 LID). It is an opaque
///   string; a namespace registry is a deferred governance decision and no
///   registry is consulted or required here.
/// * [`ExternalId::value`] is the identifier as issued by that system (for
///   example a PDS4 LID such as `"urn:nasa:pds:orex.raw_pds:data"`). No PDS4
///   (or any other) validation rules are applied to PTIFF identity.
/// * [`ExternalId::version`] is optional (for example a PDS4 VID).
///
/// Only structural rules are enforced: namespace and value must be non-empty
/// so the pair `(namespace, value)` is unambiguous. No further lexical
/// constraints are imposed, so the type does not silently commit to an
/// external identifier syntax.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExternalId {
    namespace: String,
    value: String,
    version: Option<String>,
}

impl ExternalId {
    /// Builds an external identifier with `namespace` and `value`.
    ///
    /// # Errors
    ///
    /// [`crate::ErrorCode::InvalidArgument`] if either field is empty.
    pub fn new(namespace: impl Into<String>, value: impl Into<String>) -> Result<Self> {
        let namespace = namespace.into();
        let value = value.into();
        if namespace.is_empty() {
            return Err(Error::invalid_argument(
                "ExternalId: namespace must not be empty",
            ));
        }
        if value.is_empty() {
            return Err(Error::invalid_argument(
                "ExternalId: value must not be empty",
            ));
        }
        Ok(Self {
            namespace,
            value,
            version: None,
        })
    }

    /// Attaches an optional version (for example a PDS4 VID).
    ///
    /// # Errors
    ///
    /// [`crate::ErrorCode::InvalidArgument`] if `version` is empty.
    pub fn with_version(mut self, version: impl Into<String>) -> Result<Self> {
        let version = version.into();
        if version.is_empty() {
            return Err(Error::invalid_argument(
                "ExternalId: version must not be empty",
            ));
        }
        self.version = Some(version);
        Ok(self)
    }

    /// The external authority/system that owns this identifier.
    #[must_use]
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    /// The identifier value as issued by the external system.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }

    /// The optional version of the identifier.
    #[must_use]
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    /// True when the other id names the same external entity: same namespace,
    /// same value, and — when either side carries a version — equal versions.
    /// Two ids without versions are the same external entity if namespace and
    /// value match; an explicit version refines (never replaces) the pair.
    #[must_use]
    pub fn same_external_entity(&self, other: &ExternalId) -> bool {
        self.namespace == other.namespace
            && self.value == other.value
            && match (&self.version, &other.version) {
                (None, None) => true,
                (Some(a), Some(b)) => a == b,
                _ => false,
            }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ErrorCode;

    #[test]
    fn constructs_with_namespace_and_value() {
        let id = ExternalId::new("pds4", "urn:nasa:pds:orex:data").unwrap();
        assert_eq!(id.namespace(), "pds4");
        assert_eq!(id.value(), "urn:nasa:pds:orex:data");
        assert_eq!(id.version(), None);
    }

    #[test]
    fn empty_namespace_or_value_is_invalid_argument() {
        let e = ExternalId::new("", "value").unwrap_err();
        assert_eq!(e.code(), ErrorCode::InvalidArgument);
        let e = ExternalId::new("pds4", "").unwrap_err();
        assert_eq!(e.code(), ErrorCode::InvalidArgument);
    }

    #[test]
    fn version_is_optional_and_refines_equality() {
        let a = ExternalId::new("pds4", "lid").unwrap();
        let b = ExternalId::new("pds4", "lid").unwrap();
        assert!(a.same_external_entity(&b));

        let v1 = ExternalId::new("pds4", "lid")
            .unwrap()
            .with_version("1.0")
            .unwrap();
        let v2 = ExternalId::new("pds4", "lid")
            .unwrap()
            .with_version("2.0")
            .unwrap();
        // An unversioned id and a versioned id do not silently alias.
        assert!(!a.same_external_entity(&v1));
        assert!(!v1.same_external_entity(&v2));

        let empty_version = ExternalId::new("pds4", "lid").unwrap().with_version("");
        assert!(empty_version.is_err());
    }

    #[test]
    fn same_namespace_value_in_different_namespaces_is_distinct() {
        let pds = ExternalId::new("pds4", "urn:nasa:pds:data").unwrap();
        let isis = ExternalId::new("isis", "urn:nasa:pds:data").unwrap();
        assert!(!pds.same_external_entity(&isis));
    }

    #[test]
    fn structural_equality_is_deterministic() {
        let a = ExternalId::new("pds4", "lid")
            .unwrap()
            .with_version("1.0")
            .unwrap();
        let b = ExternalId::new("pds4", "lid")
            .unwrap()
            .with_version("1.0")
            .unwrap();
        assert_eq!(a, b);
    }
}
