//! MF-03A — Typed Manifest Serialization Representation Layer
//!
//! This module establishes the typed intermediate representation between the
//! semantic `Manifest` (MF-02) and the future canonical JSON/JCS encoder (MF-03B).
//!
//! It is a **mapping layer**, not an encoder. No JSON bytes are produced here.
//!
//! ```text
//! Manifest (semantic projection)
//!     ↓
//! MF-03A typed serialization representation (this module)
//!     ↓
//! MF-03B RFC 8785 / JCS encoder (future increment)
//!     ↓
//! canonical JSON bytes
//! ```
//!
//! # Corrections vs the initial MF-03A implementation (`a54734a`)
//!
//! * **Identity** — relationships and provenance no longer carry anonymous
//!   `u64` placeholders (`0` sentinel). Endpoints are typed
//!   [`SerializedManifestRef`] values `{entity, id}` matching the G2-ratified
//!   ADR-010 scoped-reference grammar; domain and id are taken from the
//!   semantic endpoints (`EntityRef`, `ProcessRecordId`), never invented.
//! * **Relationship kinds** — no longer `format!("{:?}", ..)`. A typed
//!   [`SerializedRelationshipKind`] enum provides deterministic ADR-009
//!   vocabulary tokens.
//! * **Provenance** — [`SerializedProvenanceRelation`] keeps the CM-03
//!   `ProcessRecord → (Used|Generated) → entity` orientation with typed
//!   references on both endpoints.
//! * **External IDs** — [`SerializedExternalId`] mirrors the ratified
//!   ADR-010 grammar (`namespace`, `value`, optional `version`); a mapping
//!   from the existing [`crate::ExternalId`] is provided.
//! * **Metadata** — [`SerializationMetadata`] maps the existing
//!   `ManifestMetadata` faithfully (non-optional; same `version` semantics).
//! * **Axes** — [`SerializedAxisDescriptor`] uses the typed
//!   [`SerializedAxisKind`] vocabulary (x/y/band/time/polarization), keeps
//!   `extent` as `u64`, and preserves semantic axis order.
//!
//! # Architectural boundary
//!
//! This representation is typed, deterministic, independent of TIFF, of JSON
//! Schema and of the eventual JSON encoder. It is **not** the normative
//! PTIFF 2.0 manifest schema: exact field spelling and schema layout remain
//! the schema phase's authority.

use crate::id::ProcessRecordId;
use crate::semantic::{AxisKind, EntityRef, ProvenanceRelationKind, RelationshipKind};
use crate::ExternalId;

/// A typed serialization representation of a [`crate::mf02::Manifest`].
///
/// The manifest is represented by typed per-domain collections plus the
/// explicit relationship and provenance graphs. No generic
/// `HashMap<String, Value>` tree is used: every collection is strongly typed.
///
/// Top-level entity collections preserve count and insertion order. Entity
/// *identity* in this representation is carried by the typed
/// [`SerializedManifestRef`] endpoints of relationships and provenance
/// relations (matching the semantic model, where the value objects themselves
/// do not hold Scene ids).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerializationManifest {
    /// Manifest metadata mapped from `ManifestMetadata`.
    pub metadata: SerializationMetadata,
    /// Core Model entities (CM-01): observations, in insertion order.
    pub observations: Vec<SerializedObservation>,
    /// Core Model entities (CM-01): data objects, in insertion order.
    pub data_objects: Vec<SerializedDataObject>,
    /// Core Model entities (CM-01): products, in insertion order.
    pub products: Vec<SerializedProduct>,
    /// Explicit directed semantic relationships (CM-02), in insertion order.
    pub relationships: Vec<SerializedRelationship>,
    /// Provenance process records (CM-03), in insertion order.
    pub process_records: Vec<SerializedProcessRecord>,
    /// Explicit directed provenance edges (CM-03), in insertion order.
    pub provenance_relations: Vec<SerializedProvenanceRelation>,
}

/// Manifest metadata mapped faithfully from the existing `ManifestMetadata`.
///
/// `version` is the same version value carried by the semantic metadata; no
/// schema id, revision id, timestamp, authorship or lifecycle fields are
/// added (those remain deferred).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerializationMetadata {
    /// The manifest version, copied from `ManifestMetadata::version`.
    pub version: u64,
}

/// Entity domain of a typed manifest reference (ADR-010 initial domains).
///
/// Tokens follow the ratified ADR-010 domain vocabulary:
/// `observation`, `data_object`, `product`, `process_record`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SerializedEntityKind {
    /// An [`crate::Observation`] (scientific measurement/acquisition event).
    Observation,
    /// A [`crate::DataObject`] (concrete scientific data representation).
    DataObject,
    /// A [`crate::Product`] (scientific or derived result).
    Product,
    /// A [`crate::ProcessRecord`] (provenance process step).
    ProcessRecord,
}

impl SerializedEntityKind {
    /// The deterministic ADR-010 domain token for this entity kind.
    #[must_use]
    pub const fn as_token(self) -> &'static str {
        match self {
            SerializedEntityKind::Observation => "observation",
            SerializedEntityKind::DataObject => "data_object",
            SerializedEntityKind::Product => "product",
            SerializedEntityKind::ProcessRecord => "process_record",
        }
    }
}

/// A typed manifest reference: `{entity, id}` per ADR-010.
///
/// The domain is explicit, so `Observation(id=7)` is distinguishable from
/// `DataObject(id=7)`. Ids are manifest-local non-negative integers; no
/// UUID/ULID/PDS4-LID/IFD-index/byte-offset semantics are introduced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SerializedManifestRef {
    /// The referenced entity domain.
    pub entity: SerializedEntityKind,
    /// The manifest-local non-negative id within that domain.
    pub id: u64,
}

impl SerializedManifestRef {
    /// Builds a typed reference from a semantic [`EntityRef`].
    #[must_use]
    pub const fn from_entity_ref(reference: EntityRef) -> Self {
        match reference {
            EntityRef::Observation(id) => SerializedManifestRef {
                entity: SerializedEntityKind::Observation,
                id: id.value(),
            },
            EntityRef::DataObject(id) => SerializedManifestRef {
                entity: SerializedEntityKind::DataObject,
                id: id.value(),
            },
            EntityRef::Product(id) => SerializedManifestRef {
                entity: SerializedEntityKind::Product,
                id: id.value(),
            },
        }
    }

    /// Builds a typed reference to a [`ProcessRecordId`].
    #[must_use]
    pub const fn from_process_record(id: ProcessRecordId) -> Self {
        SerializedManifestRef {
            entity: SerializedEntityKind::ProcessRecord,
            id: id.value(),
        }
    }

    /// The entity domain of this reference.
    #[must_use]
    pub const fn entity(self) -> SerializedEntityKind {
        self.entity
    }

    /// The manifest-local non-negative id within the domain.
    #[must_use]
    pub const fn id(self) -> u64 {
        self.id
    }
}

/// A serialized observation entity (CM-01).
///
/// The semantic `Observation` value object currently carries no content
/// fields, so this record is an empty marker; observation identity is
/// expressed through typed [`SerializedManifestRef`] endpoints elsewhere in
/// the manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerializedObservation {}

/// A serialized data object with its ordered axis declaration (ADR-003).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerializedDataObject {
    /// Ordered semantic axis descriptors, in insertion order (never sorted).
    pub axes: Vec<SerializedAxisDescriptor>,
}

/// Serialized semantic axis role of a data-object axis (ADR-003).
///
/// The initial normative tokens are `x`, `y`, `band`, `time`,
/// `polarization`. No coordinates, units, CRS or calibration are introduced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SerializedAxisKind {
    /// Spatial sample/column axis.
    X,
    /// Spatial line/row axis.
    Y,
    /// Band axis (spectral semantics; coordinates deferred).
    Band,
    /// Time axis (no time system implied).
    Time,
    /// Polarization/stokes axis.
    Polarization,
}

impl SerializedAxisKind {
    /// The deterministic ADR-003 token for this axis kind.
    #[must_use]
    pub const fn as_token(self) -> &'static str {
        match self {
            SerializedAxisKind::X => "x",
            SerializedAxisKind::Y => "y",
            SerializedAxisKind::Band => "band",
            SerializedAxisKind::Time => "time",
            SerializedAxisKind::Polarization => "polarization",
        }
    }
}

impl From<AxisKind> for SerializedAxisKind {
    fn from(kind: AxisKind) -> Self {
        match kind {
            AxisKind::X => SerializedAxisKind::X,
            AxisKind::Y => SerializedAxisKind::Y,
            AxisKind::Band => SerializedAxisKind::Band,
            AxisKind::Time => SerializedAxisKind::Time,
            AxisKind::Polarization => SerializedAxisKind::Polarization,
        }
    }
}

/// A serialized axis descriptor: semantic role plus logical extent.
///
/// The ordered position in the containing data object's `axes` collection is
/// semantically significant and preserved verbatim; MF-03A never sorts axes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerializedAxisDescriptor {
    /// Semantic role token of the axis.
    pub kind: SerializedAxisKind,
    /// Logical cardinality along the axis (u64, preserved without narrowing).
    pub extent: u64,
}

/// A serialized product entity (CM-01).
///
/// The semantic `Product` value object currently carries no content fields,
/// so this record is an empty marker; product identity is expressed through
/// typed [`SerializedManifestRef`] endpoints elsewhere in the manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerializedProduct {}

/// A serialized process record for provenance (CM-03).
///
/// The semantic `ProcessRecord` value object currently carries no content
/// fields, so this record is an empty marker; process-record identity is
/// expressed through typed [`SerializedManifestRef`] endpoints in
/// [`SerializedProvenanceRelation`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerializedProcessRecord {}

/// Serialized PTIFF-native relationship kind (ADR-009).
///
/// Vocabulary tokens are the ADR-009 PTIFF-native edge spellings
/// (`produces`, `derived-from`). No new relationship semantics are added;
/// exact schema field spelling remains the schema phase's authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SerializedRelationshipKind {
    /// Source Observation produced the target DataObject.
    Produces,
    /// Target is derived from source (`derived-from`).
    DerivedFrom,
}

impl SerializedRelationshipKind {
    /// The deterministic ADR-009 vocabulary token.
    #[must_use]
    pub const fn as_token(self) -> &'static str {
        match self {
            SerializedRelationshipKind::Produces => "produces",
            SerializedRelationshipKind::DerivedFrom => "derived-from",
        }
    }
}

impl From<RelationshipKind> for SerializedRelationshipKind {
    fn from(kind: RelationshipKind) -> Self {
        match kind {
            RelationshipKind::Produces => SerializedRelationshipKind::Produces,
            RelationshipKind::DerivedFrom => SerializedRelationshipKind::DerivedFrom,
        }
    }
}

/// A serialized directed semantic relationship (CM-02).
///
/// Endpoints keep their typed entity domains (ADR-010); the kind uses the
/// deterministic PTIFF vocabulary (ADR-009).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerializedRelationship {
    /// Source endpoint, typed by entity domain.
    pub source: SerializedManifestRef,
    /// Relationship kind (PTIFF-native vocabulary).
    pub kind: SerializedRelationshipKind,
    /// Target endpoint, typed by entity domain.
    pub target: SerializedManifestRef,
}

/// Serialized provenance relation kind (CM-03).
///
/// Canonical orientation: the relation originates at a `ProcessRecord` and
/// points at a CM-01 entity. Kinds are `Used` (input consumed) and
/// `Generated` (output produced). No Agent nodes and no W3C PROV/RDF
/// semantics are introduced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SerializedProvenanceRelationKind {
    /// The process used the entity as an input.
    Used,
    /// The process generated the entity as an output.
    Generated,
}

impl SerializedProvenanceRelationKind {
    /// The deterministic token for this provenance kind.
    #[must_use]
    pub const fn as_token(self) -> &'static str {
        match self {
            SerializedProvenanceRelationKind::Used => "used",
            SerializedProvenanceRelationKind::Generated => "generated",
        }
    }
}

impl From<ProvenanceRelationKind> for SerializedProvenanceRelationKind {
    fn from(kind: ProvenanceRelationKind) -> Self {
        match kind {
            ProvenanceRelationKind::Used => SerializedProvenanceRelationKind::Used,
            ProvenanceRelationKind::Generated => SerializedProvenanceRelationKind::Generated,
        }
    }
}

/// A serialized provenance edge (CM-03): `process → entity`.
///
/// Both endpoints are typed references; the process endpoint always carries
/// the `ProcessRecord` domain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerializedProvenanceRelation {
    /// Process-record endpoint (source), domain `ProcessRecord`.
    pub process: SerializedManifestRef,
    /// Provenance kind (Used/Generated).
    pub kind: SerializedProvenanceRelationKind,
    /// Entity endpoint (target), typed by its entity domain.
    pub entity: SerializedManifestRef,
}

/// A serialized external identifier (ADR-010 grammar).
///
/// Mirrors `{namespace, value, version?}`: `namespace` names the external
/// authority, `value` is the identifier as issued there, and `version` is
/// optional and omitted when absent. External identifiers are opaque
/// references; PDS4 LIDs remain opaque external values, never PTIFF-native
/// identity.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SerializedExternalId {
    /// External authority/system owning the identifier.
    pub namespace: String,
    /// Identifier value as issued by the external system.
    pub value: String,
    /// Optional version (absent when the external id has no version).
    pub version: Option<String>,
}

impl From<&ExternalId> for SerializedExternalId {
    fn from(external: &ExternalId) -> Self {
        SerializedExternalId {
            namespace: external.namespace().to_owned(),
            value: external.value().to_owned(),
            version: external.version().map(str::to_owned),
        }
    }
}

impl SerializationManifest {
    /// Converts a [`crate::mf02::Manifest`] into a typed serialization
    /// representation.
    ///
    /// The conversion borrows the source manifest and produces an owned
    /// representation; the source is never mutated.
    ///
    /// # Mapping guarantees
    ///
    /// * Entity preservation — every entity collection is mapped with its
    ///   count and insertion order.
    /// * Relationship preservation — source/kind/target survive with typed
    ///   endpoint domains and ids (no anonymous placeholders).
    /// * Provenance preservation — process-record and entity endpoints keep
    ///   their domains; Used/Generated semantics are retained.
    /// * Axis preservation — kind, extent and semantic order are retained.
    /// * Metadata preservation — `ManifestMetadata::version` maps faithfully.
    /// * Determinism — equivalent input manifests produce equivalent typed
    ///   representations (order-preserving, no sorting, no hashing).
    #[must_use]
    pub fn from_manifest(manifest: &crate::mf02::Manifest) -> Self {
        let metadata = SerializationMetadata {
            version: manifest.metadata.version(),
        };

        let observations: Vec<SerializedObservation> = manifest
            .observations
            .iter()
            .map(|_| SerializedObservation {})
            .collect();

        let data_objects: Vec<SerializedDataObject> = manifest
            .data_objects
            .iter()
            .map(|dobj| SerializedDataObject {
                axes: dobj
                    .axes()
                    .iter()
                    .map(|ax| SerializedAxisDescriptor {
                        kind: SerializedAxisKind::from(ax.kind()),
                        extent: ax.extent(),
                    })
                    .collect(),
            })
            .collect();

        let products: Vec<SerializedProduct> = manifest
            .products
            .iter()
            .map(|_| SerializedProduct {})
            .collect();

        let process_records: Vec<SerializedProcessRecord> = manifest
            .process_records
            .iter()
            .map(|_| SerializedProcessRecord {})
            .collect();

        let relationships: Vec<SerializedRelationship> = manifest
            .relationships
            .iter()
            .map(|rel| SerializedRelationship {
                source: SerializedManifestRef::from_entity_ref(rel.source()),
                kind: SerializedRelationshipKind::from(rel.kind()),
                target: SerializedManifestRef::from_entity_ref(rel.target()),
            })
            .collect();

        let provenance_relations: Vec<SerializedProvenanceRelation> = manifest
            .provenance_relations
            .iter()
            .map(|rel| SerializedProvenanceRelation {
                process: SerializedManifestRef::from_process_record(rel.process()),
                kind: SerializedProvenanceRelationKind::from(rel.kind()),
                entity: SerializedManifestRef::from_entity_ref(rel.entity()),
            })
            .collect();

        Self {
            metadata,
            observations,
            data_objects,
            products,
            relationships,
            process_records,
            provenance_relations,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::Scene;
    use crate::semantic::{
        AxisDescriptor, DataObject, Observation, ProcessRecord, Product, RelationshipKind,
    };
    use crate::ManifestMetadata;

    fn build_manifest() -> crate::mf02::Manifest {
        let scene = Scene::new();
        crate::mf02::Manifest::from_scene(scene)
    }

    /// Empty manifest converts to an empty serialization representation.
    #[test]
    fn empty_manifest_yields_empty_serialization() {
        let meta = ManifestMetadata::new(7);
        let manifest = crate::mf02::Manifest::new_empty(Some(meta));
        let serialized = SerializationManifest::from_manifest(&manifest);

        assert_eq!(serialized.metadata.version, 7);
        assert!(serialized.observations.is_empty());
        assert!(serialized.data_objects.is_empty());
        assert!(serialized.products.is_empty());
        assert!(serialized.relationships.is_empty());
        assert!(serialized.process_records.is_empty());
        assert!(serialized.provenance_relations.is_empty());
    }

    /// Entity collections preserve counts and order.
    #[test]
    fn entity_preservation() {
        let mut scene = Scene::new();
        scene.add_observation(Observation::new());
        scene.add_observation(Observation::new());
        scene.add_data_object(DataObject::new());
        scene.add_product(Product::new());
        scene.add_process_record(ProcessRecord::new());

        let manifest = crate::mf02::Manifest::from_scene(scene);
        let serialized = SerializationManifest::from_manifest(&manifest);

        assert_eq!(serialized.observations.len(), 2);
        assert_eq!(serialized.data_objects.len(), 1);
        assert_eq!(serialized.products.len(), 1);
        assert_eq!(serialized.process_records.len(), 1);
    }

    /// Equal numeric ids in different domains remain distinct typed refs.
    #[test]
    fn identity_references_preserve_domains() {
        let mut scene = Scene::new();
        // Two observations and two data objects so each domain has an id 1.
        let _obs0 = scene.add_observation(Observation::new());
        let obs1 = scene.add_observation(Observation::new());
        let _dobj0 = scene.add_data_object(DataObject::new());
        let dobj1 = scene.add_data_object(DataObject::new());
        let prod = scene.add_product(Product::new());

        assert_eq!(obs1.value(), 1);
        assert_eq!(dobj1.value(), 1);

        scene
            .add_relationship(
                EntityRef::Observation(obs1),
                RelationshipKind::Produces,
                EntityRef::DataObject(dobj1),
            )
            .unwrap();
        scene
            .add_relationship(
                EntityRef::DataObject(dobj1),
                RelationshipKind::DerivedFrom,
                EntityRef::Product(prod),
            )
            .unwrap();

        let manifest = crate::mf02::Manifest::from_scene(scene);
        let serialized = SerializationManifest::from_manifest(&manifest);

        assert_eq!(serialized.relationships.len(), 2);

        let produces = &serialized.relationships[0];
        assert_eq!(produces.source.entity(), SerializedEntityKind::Observation);
        assert_eq!(produces.source.id(), 1);
        assert_eq!(produces.target.entity(), SerializedEntityKind::DataObject);
        assert_eq!(produces.target.id(), 1);
        // Observation(1) and DataObject(1) are distinct typed references.
        assert_ne!(produces.source, produces.target);
        assert_eq!(produces.source.entity().as_token(), "observation");
        assert_eq!(produces.target.entity().as_token(), "data_object");

        let derived = &serialized.relationships[1];
        assert_eq!(derived.source.entity(), SerializedEntityKind::DataObject);
        assert_eq!(derived.source.id(), 1);
        assert_eq!(derived.target.entity(), SerializedEntityKind::Product);
        assert_eq!(derived.target.id(), prod.value());
    }

    /// Relationship kinds map to the deterministic ADR-009 vocabulary.
    #[test]
    fn relationship_kinds_map_deterministically() {
        assert_eq!(
            SerializedRelationshipKind::from(RelationshipKind::Produces).as_token(),
            "produces"
        );
        assert_eq!(
            SerializedRelationshipKind::from(RelationshipKind::DerivedFrom).as_token(),
            "derived-from"
        );
    }

    /// Provenance keeps process-record references and Used/Generated kinds.
    #[test]
    fn provenance_preserves_process_and_entity_domains() {
        let mut scene = Scene::new();
        let dobj = scene.add_data_object(DataObject::new());
        let prod = scene.add_product(Product::new());
        let proc = scene.add_process_record(ProcessRecord::new());

        scene
            .add_provenance_relation(
                proc,
                crate::semantic::ProvenanceRelationKind::Used,
                EntityRef::DataObject(dobj),
            )
            .unwrap();
        scene
            .add_provenance_relation(
                proc,
                crate::semantic::ProvenanceRelationKind::Generated,
                EntityRef::Product(prod),
            )
            .unwrap();

        let manifest = crate::mf02::Manifest::from_scene(scene);
        let serialized = SerializationManifest::from_manifest(&manifest);

        assert_eq!(serialized.provenance_relations.len(), 2);

        let used = &serialized.provenance_relations[0];
        assert_eq!(used.process.entity(), SerializedEntityKind::ProcessRecord);
        assert_eq!(used.process.id(), proc.value());
        assert_eq!(used.kind, SerializedProvenanceRelationKind::Used);
        assert_eq!(used.kind.as_token(), "used");
        assert_eq!(used.entity.entity(), SerializedEntityKind::DataObject);
        assert_eq!(used.entity.id(), dobj.value());

        let generated = &serialized.provenance_relations[1];
        assert_eq!(generated.kind, SerializedProvenanceRelationKind::Generated);
        assert_eq!(generated.kind.as_token(), "generated");
        assert_eq!(generated.entity.entity(), SerializedEntityKind::Product);
        assert_eq!(generated.entity.id(), prod.value());
    }

    /// External ids map namespace/value/version and absent version.
    #[test]
    fn external_ids_preserve_namespace_value_version() {
        let plain = ExternalId::new("pds4", "urn:nasa:pds:orex:data").unwrap();
        let serialized_plain = SerializedExternalId::from(&plain);
        assert_eq!(serialized_plain.namespace, "pds4");
        assert_eq!(serialized_plain.value, "urn:nasa:pds:orex:data");
        assert_eq!(serialized_plain.version, None);

        let versioned = ExternalId::new("pds4", "urn:nasa:pds:orex:data")
            .unwrap()
            .with_version("1.0")
            .unwrap();
        let serialized_versioned = SerializedExternalId::from(&versioned);
        assert_eq!(serialized_versioned.namespace, "pds4");
        assert_eq!(serialized_versioned.value, "urn:nasa:pds:orex:data");
        assert_eq!(serialized_versioned.version.as_deref(), Some("1.0"));
    }

    /// Axes preserve kind, extent and insertion order (never sorted).
    #[test]
    fn axes_preserve_kind_extent_and_order() {
        let mut scene = Scene::new();
        let mut data_object = DataObject::new();
        data_object
            .add_axis(AxisDescriptor::new(AxisKind::Y, 512).unwrap())
            .unwrap();
        data_object
            .add_axis(AxisDescriptor::new(AxisKind::X, 1024).unwrap())
            .unwrap();
        data_object
            .add_axis(AxisDescriptor::new(AxisKind::Band, 3).unwrap())
            .unwrap();
        let _id = scene.add_data_object(data_object);

        let manifest = crate::mf02::Manifest::from_scene(scene);
        let serialized = SerializationManifest::from_manifest(&manifest);

        assert_eq!(serialized.data_objects.len(), 1);
        let axes = &serialized.data_objects[0].axes;

        // Insertion order Y, X, Band is preserved; nothing was sorted to
        // X, Y, Band.
        assert_eq!(axes.len(), 3);
        assert_eq!(axes[0].kind, SerializedAxisKind::Y);
        assert_eq!(axes[0].kind.as_token(), "y");
        assert_eq!(axes[0].extent, 512);
        assert_eq!(axes[1].kind, SerializedAxisKind::X);
        assert_eq!(axes[1].kind.as_token(), "x");
        assert_eq!(axes[1].extent, 1024);
        assert_eq!(axes[2].kind, SerializedAxisKind::Band);
        assert_eq!(axes[2].kind.as_token(), "band");
        assert_eq!(axes[2].extent, 3);
    }

    /// Axis extent is preserved without narrowing, including values beyond
    /// the JSON-safe range (2^53 - 1). MF-03A keeps the u64; MF-03B will
    /// choose the lexical encoding per ADR-011.
    #[test]
    fn numeric_fidelity_preserves_large_integers() {
        let huge: u64 = 9_007_199_254_740_993; // 2^53 + 1
        let mut scene = Scene::new();
        let mut data_object = DataObject::new();
        data_object
            .add_axis(AxisDescriptor::new(AxisKind::X, huge).unwrap())
            .unwrap();
        let _id = scene.add_data_object(data_object);

        let manifest = crate::mf02::Manifest::from_scene(scene);
        let serialized = SerializationManifest::from_manifest(&manifest);

        let axis = &serialized.data_objects[0].axes[0];
        assert_eq!(axis.extent, huge);
        // The u64 survives the mapping un-narrowed: it is not silently cast
        // to f32/f64/i64/string. Choosing the lexical encoding for 2^53+
        // integers is MF-03B's ADR-011 concern, not this mapping's.
    }

    /// Determinism: equivalent manifests produce equivalent representations.
    #[test]
    fn determinism() {
        let manifest1 = build_manifest();
        let manifest2 = build_manifest();
        assert_eq!(
            SerializationManifest::from_manifest(&manifest1),
            SerializationManifest::from_manifest(&manifest2)
        );
    }

    /// Ownership: conversion borrows and never mutates the source manifest.
    #[test]
    fn ownership_source_unchanged() {
        let mut scene = Scene::new();
        scene.add_observation(Observation::new());
        scene.add_data_object(DataObject::new());

        let manifest = crate::mf02::Manifest::from_scene(scene);
        let before_obs = manifest.observation_count();
        let before_dobj = manifest.data_object_count();
        let before_version = manifest.metadata.version();

        let serialized = SerializationManifest::from_manifest(&manifest);

        assert_eq!(manifest.observation_count(), before_obs);
        assert_eq!(manifest.data_object_count(), before_dobj);
        assert_eq!(manifest.metadata.version(), before_version);
        assert_eq!(serialized.metadata.version, before_version);
    }
}
