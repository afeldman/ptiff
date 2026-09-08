//! MF-01 — Manifest foundation (internal manifest-domain representation).
//!
//! This module establishes the minimal typed structures for a future
//! manifest layer to represent PTIFF 2.0 semantic entities. It does **not**
//! implement physical serialization, TIFF embedding, or JSON/JCS wire formats.
//!
//! # Types
//!
//! - [`ManifestRef`] — A Scene-local reference carrying an entity domain and id
//!   (G2-ratified identity model: {entity, id}).
//! - [`ManifestMetadata`] — A placeholder for manifest-level metadata/version.
//!
//! # Deferred decisions (not implemented here)
//!
//! Per the frozen G2 architecture:
//! - complete manifest schema
//! - exact manifest field names
//! - token-registry governance
//! - JSON Schema $id conventions
//! - coordinate/unit/time/spectral representations
//! - merge/distributed identity minting
//! - SR-01 physical axis/storage mapping
//! - validator API
//! - overlay implementation
//! - binding/canonicalization details
//! - extension registry content
//! - manifest size limits
//! - update/revision mechanics
//! - CBOR

use crate::id::{DataObjectId, ObservationId, ProcessRecordId, ProductId};

/// A strongly-typed reference to a Core Model entity within the manifest.
///
/// Per the G2-ratified identity model (ADR-010), manifest references are
/// conceptually `{entity: domain, id: u64}`. This enum wraps the existing
/// Scene-scoped typed ids ([`ObservationId`], [`DataObjectId`],
/// [`ProductId`], [`ProcessRecordId`]) to preserve entity kind at the type
/// level rather than reducing everything to a bare `u64`.
///
/// # Invariants (local Rust invariants only)
///
/// - **Valid domain:** each variant corresponds to an initial Core Model
///   entity kind (Observation, DataObject, Product, ProcessRecord).
/// - **Non-negative ID:** the wrapped id value is always ≥ 0 (guaranteed by
///   the underlying Id type construction).
///
/// # Deferred decisions (not encoded here)
///
/// Per G2 open items:
/// - The complete manifest schema and exact field names are open.
/// - Token-registry governance, JSON Schema $id conventions, coordinate/unit
///   representations, and all other serialization details remain deferred
///   to later increments.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ManifestRef {
    /// Reference to an [`Observation`] entity.
    Observation(ObservationId),
    /// Reference to a [`DataObject`] entity.
    DataObject(DataObjectId),
    /// Reference to a [`Product`] entity.
    Product(ProductId),
    /// Reference to a [`ProcessRecord`] entity.
    ProcessRecord(ProcessRecordId),
}

impl ManifestRef {
    /// The numeric Scene-local id value (shared by all variants).
    ///
    /// This exposes the underlying `u64` id without exposing the entity domain.
    #[must_use]
    pub const fn value(self) -> u64 {
        match self {
            ManifestRef::Observation(id) => id.value(),
            ManifestRef::DataObject(id) => id.value(),
            ManifestRef::Product(id) => id.value(),
            ManifestRef::ProcessRecord(id) => id.value(),
        }
    }

    /// Returns the entity domain of this reference.
    #[must_use]
    pub const fn domain(&self) -> ManifestEntityKind {
        match self {
            ManifestRef::Observation(_) => ManifestEntityKind::Observation,
            ManifestRef::DataObject(_) => ManifestEntityKind::DataObject,
            ManifestRef::Product(_) => ManifestEntityKind::Product,
            ManifestRef::ProcessRecord(_) => ManifestEntityKind::ProcessRecord,
        }
    }

    /// Observations are distinct from DataObjects and Products.
    #[must_use]
    pub const fn is_observation(self) -> bool {
        matches!(self, ManifestRef::Observation(_))
    }

    /// DataObjects are distinct from Observations and Products.
    #[must_use]
    pub const fn is_data_object(self) -> bool {
        matches!(self, ManifestRef::DataObject(_))
    }

    /// Products are distinct from Observations and DataObjects.
    #[must_use]
    pub const fn is_product(self) -> bool {
        matches!(self, ManifestRef::Product(_))
    }

    /// ProcessRecords are distinct from Observations, DataObjects, and Products.
    #[must_use]
    pub const fn is_process_record(self) -> bool {
        matches!(self, ManifestRef::ProcessRecord(_))
    }
}

/// The set of initial Core Model entity domains supported by manifest references.
///
/// This enum is additive (`#[non_exhaustive]`) — future increments may add
/// new entity kinds as the semantic model evolves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ManifestEntityKind {
    /// An [`Observation`] (scientific measurement/acquisition event).
    Observation,
    /// A [`DataObject`] (concrete scientific data representation).
    DataObject,
    /// A [`Product`] (scientific or derived result).
    Product,
    /// A [`ProcessRecord`] (provenance process step).
    ProcessRecord,
}

impl ManifestEntityKind {
    /// Returns the domain name as a string slice for logging/debugging.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            ManifestEntityKind::Observation => "Observation",
            ManifestEntityKind::DataObject => "DataObject",
            ManifestEntityKind::Product => "Product",
            ManifestEntityKind::ProcessRecord => "ProcessRecord",
        }
    }
}

/// Placeholder for manifest-level metadata.
///
/// Per MF-01 scope, this provides the minimum structure to represent
/// manifest metadata without committing to final field names or wire format.
/// The `version` field is a placeholder — the exact semantic and serialization
/// of version numbers remains deferred (see ADR open items).
///
/// # Deferred decisions
///
/// - Complete manifest schema
/// - Exact manifest field names
/// - Token-registry governance
/// - JSON Schema $id conventions
#[derive(Debug, Clone, Default)]
pub struct ManifestMetadata {
    version: u64,
}

impl ManifestMetadata {
    /// Creates a new manifest metadata with the given version.
    #[must_use]
    pub const fn new(version: u64) -> Self {
        Self { version }
    }

    /// The manifest version (placeholder field).
    #[must_use]
    pub const fn version(&self) -> u64 {
        self.version
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::Scene;
    use crate::semantic::{DataObject, Observation, ProcessRecord, Product};

    #[test]
    fn manifest_ref_observation_is_constructible() {
        let id = ObservationId::new(5);
        let ref_ = ManifestRef::Observation(id);

        assert!(ref_.is_observation());
        assert!(!ref_.is_data_object());
        assert!(!ref_.is_product());
        assert!(!ref_.is_process_record());
    }

    #[test]
    fn manifest_ref_data_object_is_constructible() {
        let id = DataObjectId::new(5);
        let ref_ = ManifestRef::DataObject(id);

        assert!(ref_.is_data_object());
        assert!(!ref_.is_observation());
        assert!(!ref_.is_product());
        assert!(!ref_.is_process_record());
    }

    #[test]
    fn manifest_ref_product_is_constructible() {
        let id = ProductId::new(5);
        let ref_ = ManifestRef::Product(id);

        assert!(ref_.is_product());
        assert!(!ref_.is_observation());
        assert!(!ref_.is_data_object());
        assert!(!ref_.is_process_record());
    }

    #[test]
    fn manifest_ref_process_record_is_constructible() {
        let id = ProcessRecordId::new(5);
        let ref_ = ManifestRef::ProcessRecord(id);

        assert!(ref_.is_process_record());
        assert!(!ref_.is_observation());
        assert!(!ref_.is_data_object());
        assert!(!ref_.is_product());
    }

    #[test]
    fn manifest_ref_value_preserves_id() {
        let obs_id = ObservationId::new(123);
        let ref_ = ManifestRef::Observation(obs_id);
        assert_eq!(ref_.value(), 123);

        let dobj_id = DataObjectId::new(456);
        let ref_ = ManifestRef::DataObject(dobj_id);
        assert_eq!(ref_.value(), 456);

        let prod_id = ProductId::new(789);
        let ref_ = ManifestRef::Product(prod_id);
        assert_eq!(ref_.value(), 789);

        let proc_id = ProcessRecordId::new(999);
        let ref_ = ManifestRef::ProcessRecord(proc_id);
        assert_eq!(ref_.value(), 999);
    }

    #[test]
    fn manifest_entity_kind_has_correct_names() {
        assert_eq!(ManifestEntityKind::Observation.as_str(), "Observation");
        assert_eq!(ManifestEntityKind::DataObject.as_str(), "DataObject");
        assert_eq!(ManifestEntityKind::Product.as_str(), "Product");
        assert_eq!(ManifestEntityKind::ProcessRecord.as_str(), "ProcessRecord");
    }

    #[test]
    fn manifest_metadata_is_constructible() {
        let meta = ManifestMetadata::new(1);
        assert_eq!(meta.version(), 1);
    }

    #[test]
    fn manifest_metadata_version_preserved() {
        let versions: Vec<u64> = vec![0, 1, 10, 100, 256];
        for v in versions.iter() {
            let meta = ManifestMetadata::new(*v);
            assert_eq!(meta.version(), *v);
        }
    }

    #[test]
    fn observation_can_be_referenced_in_manifest() {
        let mut scene = Scene::new();
        let obs_id = scene.add_observation(Observation::new());

        assert_eq!(obs_id.value(), 0); // First entity gets id 0

        let ref_ = ManifestRef::Observation(obs_id);
        assert!(ref_.is_observation());
        assert_eq!(ref_.value(), 0);
    }

    #[test]
    fn data_object_can_be_referenced_in_manifest() {
        let mut scene = Scene::new();
        let dobj_id = scene.add_data_object(DataObject::new());

        // ID is position-based in the current Scene (0..n), not a global counter
        assert!(dobj_id.value() >= 0);

        let ref_ = ManifestRef::DataObject(dobj_id);
        assert!(ref_.is_data_object());
        assert_eq!(ref_.value(), dobj_id.value());
    }

    #[test]
    fn product_can_be_referenced_in_manifest() {
        let mut scene = Scene::new();
        let prod_id = scene.add_product(Product::new());

        // ID is position-based in the current Scene (0..n), not a global counter
        assert!(prod_id.value() >= 0);

        let ref_ = ManifestRef::Product(prod_id);
        assert!(ref_.is_product());
        assert_eq!(ref_.value(), prod_id.value());
    }

    #[test]
    fn process_record_can_be_referenced_in_manifest() {
        let mut scene = Scene::new();
        let proc_id = scene.add_process_record(ProcessRecord::new());

        // ID is position-based in the current Scene (0..n), not a global counter
        assert!(proc_id.value() >= 0);

        let ref_ = ManifestRef::ProcessRecord(proc_id);
        assert!(ref_.is_process_record());
        assert_eq!(ref_.value(), proc_id.value());
    }

    #[test]
    fn manifest_refs_are_distinct_by_domain() {
        let mut scene = Scene::new();

        // Add entities in order
        let obs_id = scene.add_observation(Observation::new());
        let dobj_id = scene.add_data_object(DataObject::new());
        let prod_id = scene.add_product(Product::new());
        let proc_id = scene.add_process_record(ProcessRecord::new());

        // Create references from the ids directly
        let obs_ref = ManifestRef::Observation(obs_id);
        let dobj_ref = ManifestRef::DataObject(dobj_id);
        let prod_ref = ManifestRef::Product(prod_id);
        let proc_ref = ManifestRef::ProcessRecord(proc_id);

        // All domains are distinct
        assert_ne!(obs_ref, dobj_ref);
        assert_ne!(dobj_ref, prod_ref);
        assert_ne!(prod_ref, proc_ref);
    }

    #[test]
    fn manifest_refs_value_matches_underlying_entity() {
        let mut scene = Scene::new();

        let obs_id = scene.add_observation(Observation::new());
        let dobj_id = scene.add_data_object(DataObject::new());
        let prod_id = scene.add_product(Product::new());
        let proc_id = scene.add_process_record(ProcessRecord::new());

        let obs_ref = ManifestRef::Observation(obs_id);
        let dobj_ref = ManifestRef::DataObject(dobj_id);
        let prod_ref = ManifestRef::Product(prod_id);
        let proc_ref = ManifestRef::ProcessRecord(proc_id);

        // Each reference's value matches its underlying id
        assert_eq!(obs_ref.value(), obs_id.value());
        assert_eq!(dobj_ref.value(), dobj_id.value());
        assert_eq!(prod_ref.value(), prod_id.value());
        assert_eq!(proc_ref.value(), proc_id.value());
    }
}
