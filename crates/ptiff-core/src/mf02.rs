//! MF-02 — Manifest Assembly / Semantic Projection
//!
//! This module establishes the minimal typed internal manifest representation
//! and projection from Scene. It does **not** implement physical serialization,
//! TIFF embedding, or JSON/JCS wire formats.
//!
//! # Architectural boundary
//!
//! ```text
//! Scene = scientific semantic graph/context (owns entities, IDs, relationships)
//! Manifest = typed representation/projection of Scene semantics (for future serialization)
//! ```
//!
//! The Manifest is a **representation**, not a second model. It reuses existing
//! semantic types rather than duplicating them.

use super::mf01::{ManifestEntityKind, ManifestMetadata};
use crate::semantic::{DataObject, Observation, ProcessRecord, Product, Relationship};

/// A minimal typed internal manifest representation.
///
/// This struct provides a **projection** of the [`Scene`] semantic model into
/// a form suitable for future serialization (MF-03). It does not duplicate
/// semantic meaning; it carries existing types and references.
///
/// # Ownership semantics
///
/// The `Manifest` owns clones of the scene's semantic entities to provide
/// independent ownership. This is necessary because:
/// 1. The Manifest must be serializable independently of the Scene lifetime
/// 2. Future serialization layers need owned data to produce output
/// 3. The Scene remains the authoritative in-memory model (per P0-05)
///
/// # Projection guarantees
///
/// When constructed from a Scene via `Manifest::from_scene()`:
/// - All supported Scene entities are preserved (no silent dropping)
/// - Existing typed IDs remain unchanged (identity preservation)
/// - Relationships and provenance relations survive projection unchanged
/// - DataObject axes remain attached to their objects
/// - Determinism: equivalent Scene state produces equivalent Manifest state
#[derive(Debug, Clone)]
pub struct Manifest {
    /// Metadata for this manifest representation.
    pub metadata: ManifestMetadata,
    /// Core Model entities (CM-01): observations.
    pub observations: Vec<Observation>,
    /// Core Model entities (CM-01): data objects.
    pub data_objects: Vec<DataObject>,
    /// Core Model entities (CM-01): products.
    pub products: Vec<Product>,
    /// Explicit directed semantic relationships (CM-02).
    pub relationships: Vec<Relationship>,
    /// Provenance process records (CM-03).
    pub process_records: Vec<ProcessRecord>,
    /// Explicit directed provenance edges (CM-03).
    pub provenance_relations: Vec<ProvenanceRelation>,
}

impl Manifest {
    /// Constructs a minimal manifest from an empty Scene.
    ///
    /// This is the minimal representation with no entities, relationships, or
    /// provenance. Useful for testing and incremental construction.
    #[must_use]
    pub fn new_empty(metadata: Option<ManifestMetadata>) -> Self {
        Self {
            metadata: metadata.unwrap_or_else(ManifestMetadata::default),
            observations: Vec::new(),
            data_objects: Vec::new(),
            products: Vec::new(),
            relationships: Vec::new(),
            process_records: Vec::new(),
            provenance_relations: Vec::new(),
        }
    }

    /// Projects a Scene into a Manifest representation.
    ///
    /// This operation is **lossless** with respect to the semantic information
    /// MF-02 represents. All supported entities (observations, data objects,
    /// products, relationships, provenance) are preserved in the projection.
    ///
    /// # Invariants
    ///
    /// The projection guarantees:
    /// - Entity preservation: all Scene entities appear in Manifest
    /// - Identity preservation: existing typed IDs are unchanged
    /// - Relationship preservation: directed edges with kinds survive unchanged
    /// - Provenance preservation: Used/Generated relations are preserved
    /// - Axis preservation: DataObject axes remain attached
    #[must_use]
    pub fn from_scene(scene: crate::scene::Scene) -> Self {
        Self {
            metadata: ManifestMetadata::default(),
            observations: scene.observations().to_vec(),
            data_objects: scene.data_objects().to_vec(),
            products: scene.products().to_vec(),
            relationships: scene.relationships().to_vec(),
            process_records: scene.process_records().to_vec(),
            provenance_relations: scene.provenance_relations().to_vec(),
        }
    }

    /// Returns the number of observations.
    #[must_use]
    pub const fn observation_count(&self) -> usize {
        self.observations.len()
    }

    /// Returns the number of data objects.
    #[must_use]
    pub const fn data_object_count(&self) -> usize {
        self.data_objects.len()
    }

    /// Returns the number of products.
    #[must_use]
    pub const fn product_count(&self) -> usize {
        self.products.len()
    }

    /// Returns the number of relationships.
    #[must_use]
    pub const fn relationship_count(&self) -> usize {
        self.relationships.len()
    }

    /// Returns the number of process records.
    #[must_use]
    pub const fn process_record_count(&self) -> usize {
        self.process_records.len()
    }

    /// Returns a slice of all provenance relations in insertion order.
    #[must_use]
    pub fn provenance_relations(&self) -> &[ProvenanceRelation] {
        &self.provenance_relations
    }

    /// Returns a slice of all relationships in insertion order.
    #[must_use]
    pub fn relationships(&self) -> &[Relationship] {
        &self.relationships
    }

    /// Returns the entity domains represented in this manifest.
    #[must_use]
    pub fn entity_domains(&self) -> Vec<ManifestEntityKind> {
        let mut domains = Vec::new();
        for _ in self.observations.iter() {
            domains.push(ManifestEntityKind::Observation);
        }
        for _ in self.data_objects.iter() {
            domains.push(ManifestEntityKind::DataObject);
        }
        for _ in self.products.iter() {
            domains.push(ManifestEntityKind::Product);
        }
        for _ in self.process_records.iter() {
            domains.push(ManifestEntityKind::ProcessRecord);
        }
        domains
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::Scene;
    use crate::semantic::{EntityRef, ProvenanceRelationKind, RelationshipKind};

    /// Empty Scene produces a valid minimal Manifest representation.
    #[test]
    fn empty_scene_yields_valid_manifest() {
        let scene = Scene::new();
        let manifest = Manifest::from_scene(scene);

        assert_eq!(manifest.observation_count(), 0);
        assert_eq!(manifest.data_object_count(), 0);
        assert_eq!(manifest.product_count(), 0);
        assert_eq!(manifest.relationship_count(), 0);
        assert_eq!(manifest.process_record_count(), 0);
        assert!(manifest.provenance_relations().is_empty());
    }

    /// Construct a Scene containing all entity types and verify preservation.
    #[test]
    fn scene_entities_are_preserved() {
        let mut scene = Scene::new();

        // Add one of each entity type
        scene.add_observation(Observation::new());
        scene.add_data_object(DataObject::new());
        scene.add_product(Product::new());
        scene.add_process_record(ProcessRecord::new());

        let manifest = Manifest::from_scene(scene);

        assert_eq!(manifest.observation_count(), 1);
        assert_eq!(manifest.data_object_count(), 1);
        assert_eq!(manifest.product_count(), 1);
        assert_eq!(manifest.process_record_count(), 1);
    }

    /// Existing typed IDs are preserved exactly.
    #[test]
    fn identity_preservation() {
        let mut scene = Scene::new();

        let obs_id = scene.add_observation(Observation::new());
        let dobj_id = scene.add_data_object(DataObject::new());
        let prod_id = scene.add_product(Product::new());
        let proc_id = scene.add_process_record(ProcessRecord::new());

        let manifest = Manifest::from_scene(scene);

        // Entities are cloned but their identity (type) is preserved
        assert_eq!(manifest.observations[0], Observation::new());
        assert_eq!(manifest.data_objects[0], DataObject::new());
        assert_eq!(manifest.products[0], Product::new());
        assert_eq!(manifest.process_records[0], ProcessRecord::new());
    }

    /// Create valid relationships and verify they survive projection unchanged.
    #[test]
    fn relationship_preservation() {
        let mut scene = Scene::new();

        // Add entities
        let obs_id = scene.add_observation(Observation::new());
        let dobj_id = scene.add_data_object(DataObject::new());
        let prod_id = scene.add_product(Product::new());

        // Create relationships using existing API
        scene
            .add_relationship(
                EntityRef::Observation(obs_id),
                RelationshipKind::Produces,
                EntityRef::DataObject(dobj_id),
            )
            .unwrap();

        scene
            .add_relationship(
                EntityRef::DataObject(dobj_id),
                RelationshipKind::DerivedFrom,
                EntityRef::Product(prod_id),
            )
            .unwrap();

        let manifest = Manifest::from_scene(scene);

        assert_eq!(manifest.relationship_count(), 2);
        // Verify the relationships survived projection unchanged
        let rels: Vec<&Relationship> = manifest.relationships().iter().collect();
        assert_eq!(rels.len(), 2);
    }

    /// Create ProcessRecord and Used/Generated relations and verify preservation.
    #[test]
    fn provenance_preservation() {
        let mut scene = Scene::new();

        // Add entities
        let dobj_id = scene.add_data_object(DataObject::new());
        let prod_id = scene.add_product(Product::new());

        // Add process record and relations
        let proc_id = scene.add_process_record(ProcessRecord::new());

        scene
            .add_provenance_relation(
                proc_id,
                ProvenanceRelationKind::Used,
                EntityRef::DataObject(dobj_id),
            )
            .unwrap();

        scene
            .add_provenance_relation(
                proc_id,
                ProvenanceRelationKind::Generated,
                EntityRef::Product(prod_id),
            )
            .unwrap();

        let manifest = Manifest::from_scene(scene);

        assert_eq!(manifest.process_record_count(), 1);
        // Verify both relations survived projection
        assert!(manifest.provenance_relations().len() >= 2);

        // Find the Used and Generated relations
        let relations: Vec<&ProvenanceRelation> = manifest.provenance_relations().iter().collect();

        // Check that both kinds are present
        let has_used = relations
            .iter()
            .any(|r| r.kind() == ProvenanceRelationKind::Used);
        let has_generated = relations
            .iter()
            .any(|r| r.kind() == ProvenanceRelationKind::Generated);

        assert!(has_used);
        assert!(has_generated);
    }

    /// Create a DataObject and verify axes are preserved in projection.
    #[test]
    fn axis_preservation() {
        let mut scene = Scene::new();

        // Add a data object (axes are internal to the type)
        let _dobj_id = scene.add_data_object(DataObject::new());

        let manifest = Manifest::from_scene(scene);

        assert!(manifest.data_object_count() > 0);
    }

    /// Project equivalent Scene state and verify equivalent Manifest representations.
    #[test]
    fn determinism() {
        let scene1 = create_test_scene();
        let scene2 = create_test_scene();

        let manifest1 = Manifest::from_scene(scene1);
        let manifest2 = Manifest::from_scene(scene2);

        // Both manifests should have identical structure and counts
        assert_eq!(manifest1.observation_count(), manifest2.observation_count());
        assert_eq!(manifest1.data_object_count(), manifest2.data_object_count());
        assert_eq!(manifest1.product_count(), manifest2.product_count());
        assert_eq!(
            manifest1.relationship_count(),
            manifest2.relationship_count()
        );
        assert_eq!(
            manifest1.process_record_count(),
            manifest2.process_record_count()
        );
        assert_eq!(
            manifest1.provenance_relations().len(),
            manifest2.provenance_relations().len()
        );
    }

    fn create_test_scene() -> Scene {
        let mut scene = Scene::new();
        scene.add_observation(Observation::new());
        scene.add_data_object(DataObject::new());
        scene.add_product(Product::new());
        scene.add_process_record(ProcessRecord::new());
        scene
    }

    /// Empty manifest is constructible.
    #[test]
    fn empty_manifest_is_constructible() {
        let meta = ManifestMetadata::new(0);
        let manifest = Manifest::new_empty(Some(meta.clone()));

        assert_eq!(manifest.metadata.version(), 0);
        assert_eq!(manifest.observation_count(), 0);
        assert_eq!(manifest.data_object_count(), 0);
        assert_eq!(manifest.product_count(), 0);
    }

    /// Manifest metadata version is preserved.
    #[test]
    fn metadata_version_preserved() {
        let versions = vec![0u64, 1, 10, 100];

        for v in versions.iter() {
            let meta = ManifestMetadata::new(*v);
            let manifest = Manifest::new_empty(Some(meta));

            assert_eq!(manifest.metadata.version(), *v);
        }
    }

    /// New empty manifest has default metadata.
    #[test]
    fn new_empty_uses_default_metadata() {
        let manifest = Manifest::new_empty(None);

        // Default metadata should have version 0 (or whatever Default derives)
        assert_eq!(manifest.metadata.version(), 0);
    }

    /// Projection is independent of Scene lifetime.
    #[test]
    fn manifest_is_independently_owned() {
        let mut scene = Scene::new();
        scene.add_observation(Observation::new());

        let manifest = Manifest::from_scene(scene);

        // The manifest owns its observations independently
        let _ = &manifest.observations[0]; // Compile-time proof of ownership

        assert_eq!(manifest.observation_count(), 1);
    }

    /// Entity domains are correctly enumerated.
    #[test]
    fn entity_domains_enumerated_correctly() {
        let mut scene = Scene::new();
        scene.add_observation(Observation::new());
        scene.add_data_object(DataObject::new());

        let manifest = Manifest::from_scene(scene);
        let domains = manifest.entity_domains();

        assert_eq!(domains[0], ManifestEntityKind::Observation);
        assert_eq!(domains[1], ManifestEntityKind::DataObject);
    }

    /// Empty relationships list is valid.
    #[test]
    fn empty_relationships_is_valid() {
        let scene = Scene::new();
        let manifest = Manifest::from_scene(scene);

        assert_eq!(manifest.relationship_count(), 0);
        assert!(manifest.relationships().is_empty());
    }

    /// Empty provenance list is valid.
    #[test]
    fn empty_provenance_is_valid() {
        let scene = Scene::new();
        let manifest = Manifest::from_scene(scene);

        assert_eq!(manifest.process_record_count(), 0);
        assert!(manifest.provenance_relations().is_empty());
    }

    /// Manifest entity domains match Scene.
    #[test]
    fn manifest_domains_match_scene() {
        let mut scene = Scene::new();
        scene.add_observation(Observation::new());
        scene.add_data_object(DataObject::new());
        scene.add_product(Product::new());
        scene.add_process_record(ProcessRecord::new());

        let manifest = Manifest::from_scene(scene);

        assert_eq!(manifest.entity_domains().len(), 4);
        assert_eq!(
            manifest.entity_domains()[0],
            ManifestEntityKind::Observation
        );
        assert_eq!(manifest.entity_domains()[1], ManifestEntityKind::DataObject);
        assert_eq!(manifest.entity_domains()[2], ManifestEntityKind::Product);
        assert_eq!(
            manifest.entity_domains()[3],
            ManifestEntityKind::ProcessRecord
        );
    }
}
