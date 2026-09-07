//! CM-03 (WP-CM-03): explicit provenance semantics (directed process graph).
//!
//! Provenance = per-producing-step [`ProcessRecord`] nodes connected to CM-01
//! entities by explicit directed [`ProvenanceRelation`] edges
//! (`process --Used--> entity`, `process --Generated--> entity`). It is a
//! separate semantic layer from CM-02 Relationships — no automatic
//! conversion, no inference, no hidden synchronization. Records are
//! immutable/append-only, ordering is deterministic, foreign ids are
//! rejected, duplicates are rejected, cycles are permitted (no arbitrary
//! DAG restriction), and provenance never changes 1.x TIFF output.

use ptiff_core::image::ImageDescriptorBuilder;
use ptiff_core::io::backend::tiff::TiffBackend;
use ptiff_core::io::{
    MemoryBinaryWriter, SceneSerializer, Serializer, StorageBackend, StorageModel,
};
use ptiff_core::{
    DataObject, EntityRef, ErrorCode, ImageDescriptor, Observation, PixelType, ProcessRecord,
    ProcessRecordId, Product, ProvenanceRelationKind, Scene,
};

fn desc(width: u32, height: u32) -> ImageDescriptor {
    ImageDescriptorBuilder::new(width, height)
        .pixel_type(PixelType::UInt8)
        .channel_count(1)
        .build()
}

/// Scene with one observation, one data object, one product and one process
/// record; returns (scene, ids).
fn sample_scene() -> (Scene, ProcessRecordId) {
    let mut scene = Scene::new();
    scene.add_observation(Observation::new());
    scene.add_data_object(DataObject::new());
    scene.add_product(Product::new());
    let process = scene.add_process_record(ProcessRecord::new());
    (scene, process)
}

fn data0() -> EntityRef {
    EntityRef::DataObject(ptiff_core::DataObjectId::new(0))
}

fn product0() -> EntityRef {
    EntityRef::Product(ptiff_core::ProductId::new(0))
}

/// A. Activity/Process creation: Scene mints typed, Scene-scoped process ids.
#[test]
fn process_records_are_created_and_retrievable() {
    let mut scene = Scene::new();
    let p0 = scene.add_process_record(ProcessRecord::new());
    let p1 = scene.add_process_record(ProcessRecord::new());

    assert_ne!(p0, p1);
    assert_eq!(scene.process_record_count(), 2);
    assert_eq!(
        scene.process_record(p0).unwrap().clone(),
        ProcessRecord::new()
    );
    assert_eq!(
        scene.process_record(p1).unwrap().clone(),
        ProcessRecord::new()
    );

    let err = scene.process_record(ProcessRecordId::new(9)).unwrap_err();
    assert_eq!(err.code(), ErrorCode::NotFound);
}

/// B. Minimal valid provenance graph: record used the DataObject and
/// generated the Product.
#[test]
fn minimal_provenance_graph_used_generated() {
    let (mut scene, process) = sample_scene();

    scene
        .add_provenance_relation(process, ProvenanceRelationKind::Used, data0())
        .expect("record may use an entity");
    scene
        .add_provenance_relation(process, ProvenanceRelationKind::Generated, product0())
        .expect("record may generate an entity");

    assert_eq!(scene.provenance_relation_count(), 2);
    let rels = scene.provenance_relations_of_process(process);
    assert_eq!(rels.len(), 2);
    assert_eq!(rels[0].process(), process);
    assert_eq!(rels[0].kind(), ProvenanceRelationKind::Used);
    assert_eq!(rels[0].entity(), data0());
    assert_eq!(rels[1].kind(), ProvenanceRelationKind::Generated);
    assert_eq!(rels[1].entity(), product0());
}

/// C. Directionality: process --Used--> entity never implies the reverse.
#[test]
fn provenance_is_directional() {
    let (mut scene, process) = sample_scene();
    scene
        .add_provenance_relation(process, ProvenanceRelationKind::Used, data0())
        .unwrap();

    // Queries are directional: the entity sees only the incoming edge; there
    // is no reverse "entity used process" relation anywhere.
    assert_eq!(scene.provenance_relations_of_process(process).len(), 1);
    let by_entity = scene.provenance_relations_of_entity(data0());
    assert_eq!(by_entity.len(), 1);
    assert_eq!(by_entity[0].kind(), ProvenanceRelationKind::Used);
    assert_eq!(by_entity[0].process(), process);

    // A second record that "uses" the same data is a different edge.
    let other = scene.add_process_record(ProcessRecord::new());
    scene
        .add_provenance_relation(other, ProvenanceRelationKind::Used, data0())
        .unwrap();
    assert_eq!(scene.provenance_relations_of_entity(data0()).len(), 2);
}

/// D. Typed endpoints: provenance edges always originate at a ProcessRecord
/// and point at a CM-01 entity — the API makes an entity-to-entity or
/// process-to-process provenance edge impossible at the type level.
#[test]
fn provenance_endpoints_are_typed() {
    let (mut scene, process) = sample_scene();
    scene
        .add_provenance_relation(process, ProvenanceRelationKind::Generated, product0())
        .unwrap();

    // Endpoint kinds are checked by construction: process endpoints are
    // ProcessRecordId, entity endpoints are EntityRef.
    let rel = scene.provenance_relations()[0].clone();
    assert!(matches!(
        rel.entity(),
        EntityRef::Product(id) if id.value() == 0
    ));
    assert!(matches!(
        rel.process(),
        id if id == process
    ));
}

/// E. Scene ownership: foreign process records and foreign entities are
/// rejected.
#[test]
fn foreign_scene_ids_are_rejected() {
    let (mut scene_a, _) = sample_scene();
    let (mut scene_b, _) = sample_scene();

    // scene_a's process domain is [0]; id 1 does not exist in scene_a.
    let foreign_process = ProcessRecordId::new(1);
    let err = scene_a
        .add_provenance_relation(foreign_process, ProvenanceRelationKind::Used, data0())
        .expect_err("foreign process record must be rejected");
    assert_eq!(err.code(), ErrorCode::NotFound);

    // A data-object id beyond scene_a's domain is rejected.
    let foreign_data = EntityRef::DataObject(ptiff_core::DataObjectId::new(5));
    let err = scene_a
        .add_provenance_relation(
            ProcessRecordId::new(0),
            ProvenanceRelationKind::Used,
            foreign_data,
        )
        .expect_err("foreign entity must be rejected");
    assert_eq!(err.code(), ErrorCode::NotFound);

    // Symmetric: a process id beyond scene_b's domain is rejected there.
    let far_process = ProcessRecordId::new(9);
    let err = scene_b
        .add_provenance_relation(far_process, ProvenanceRelationKind::Generated, data0())
        .expect_err("foreign process record must be rejected");
    assert_eq!(err.code(), ErrorCode::NotFound);

    assert_eq!(scene_a.provenance_relation_count(), 0);
    assert_eq!(scene_b.provenance_relation_count(), 0);
}

/// F. Duplicate semantics: exact duplicates rejected; same entities with a
/// different kind are distinct relations.
#[test]
fn duplicate_provenance_relations_are_rejected_explicitly() {
    let (mut scene, process) = sample_scene();
    scene
        .add_provenance_relation(process, ProvenanceRelationKind::Used, data0())
        .unwrap();

    let err = scene
        .add_provenance_relation(process, ProvenanceRelationKind::Used, data0())
        .expect_err("exact duplicate must be rejected");
    assert_eq!(err.code(), ErrorCode::InvalidArgument);

    // Same process + entity, different kind: distinct relation.
    scene
        .add_provenance_relation(process, ProvenanceRelationKind::Generated, data0())
        .unwrap();
    assert_eq!(scene.provenance_relation_count(), 2);
}

/// G. Determinism: insertion-ordered storage and stable repeated queries.
#[test]
fn provenance_ordering_is_deterministic() {
    let (mut scene, process) = sample_scene();
    let d1 = EntityRef::DataObject(ptiff_core::DataObjectId::new(0));
    let p0 = EntityRef::Product(ptiff_core::ProductId::new(0));

    scene
        .add_provenance_relation(process, ProvenanceRelationKind::Used, d1)
        .unwrap();
    scene
        .add_provenance_relation(process, ProvenanceRelationKind::Generated, p0)
        .unwrap();

    let first: Vec<(ProvenanceRelationKind, EntityRef)> = scene
        .provenance_relations_of_process(process)
        .iter()
        .map(|r| (r.kind(), r.entity()))
        .collect();
    let second: Vec<(ProvenanceRelationKind, EntityRef)> = scene
        .provenance_relations_of_process(process)
        .iter()
        .map(|r| (r.kind(), r.entity()))
        .collect();
    assert_eq!(first, second);
    assert_eq!(first.len(), 2);
}

/// H. Coexistence with CM-02: ordinary Relationships and provenance relations
/// coexist without conversion or inference.
#[test]
fn relationships_and_provenance_coexist_without_inference() {
    let (mut scene, process) = sample_scene();
    let obs = EntityRef::Observation(ptiff_core::ObservationId::new(0));

    // Ordinary semantic layer.
    scene
        .add_relationship(obs, ptiff_core::RelationshipKind::Produces, data0())
        .unwrap();
    scene
        .add_relationship(
            data0(),
            ptiff_core::RelationshipKind::DerivedFrom,
            product0(),
        )
        .unwrap();

    // Provenance layer (explicit, separate).
    scene
        .add_provenance_relation(process, ProvenanceRelationKind::Used, data0())
        .unwrap();
    scene
        .add_provenance_relation(process, ProvenanceRelationKind::Generated, product0())
        .unwrap();

    // No hidden synchronization: relationship count untouched by provenance,
    // provenance count untouched by relationships.
    assert_eq!(scene.relationship_count(), 2);
    assert_eq!(scene.provenance_relation_count(), 2);
    assert_eq!(scene.provenance_relations_of_process(process).len(), 2);
    assert_eq!(scene.relationships_from(data0()).len(), 1);
}

/// H2. No automatic provenance inference: a DerivedFrom relationship alone
/// never creates provenance edges.
#[test]
fn derived_from_relationship_does_not_create_provenance() {
    let mut scene = Scene::new();
    scene.add_data_object(DataObject::new());
    scene.add_product(Product::new());
    scene
        .add_relationship(
            data0(),
            ptiff_core::RelationshipKind::DerivedFrom,
            product0(),
        )
        .unwrap();

    assert_eq!(scene.provenance_relation_count(), 0);
    assert_eq!(scene.process_record_count(), 0);
}

/// I. Identity stability: adding provenance never renumbers existing ids.
#[test]
fn provenance_does_not_renumber_existing_ids() {
    let mut scene = Scene::new();
    let image = scene.add_image(desc(8, 8)).unwrap();
    let obs = scene.add_observation(Observation::new());
    let data = scene.add_data_object(DataObject::new());
    let prod = scene.add_product(Product::new());
    let process = scene.add_process_record(ProcessRecord::new());

    scene
        .add_provenance_relation(
            process,
            ProvenanceRelationKind::Used,
            EntityRef::DataObject(data),
        )
        .unwrap();
    scene
        .add_provenance_relation(
            process,
            ProvenanceRelationKind::Generated,
            EntityRef::Product(prod),
        )
        .unwrap();

    assert_eq!(scene.image(image).unwrap().width(), 8);
    assert!(scene.observation(obs).is_ok());
    assert!(scene.data_object(data).is_ok());
    assert!(scene.product(prod).is_ok());
    assert!(scene.process_record(process).is_ok());
}

/// J. Serialization boundary: adding provenance changes zero 1.x TIFF bytes.
#[test]
fn provenance_never_changes_1x_output() {
    let mut plain = Scene::new();
    plain.add_image(desc(16, 16)).unwrap();
    plain.add_observation(Observation::new());
    plain.add_data_object(DataObject::new());
    plain.add_product(Product::new());

    let mut prov = Scene::new();
    prov.add_image(desc(16, 16)).unwrap();
    prov.add_observation(Observation::new());
    prov.add_data_object(DataObject::new());
    prov.add_product(Product::new());
    let process = prov.add_process_record(ProcessRecord::new());
    prov.add_provenance_relation(process, ProvenanceRelationKind::Used, data0())
        .unwrap();
    prov.add_provenance_relation(process, ProvenanceRelationKind::Generated, product0())
        .unwrap();

    assert_eq!(serialize_first_image(&plain), serialize_first_image(&prov));
}

fn serialize_first_image(scene: &Scene) -> Vec<u8> {
    let root: StorageModel = SceneSerializer.serialize(scene).expect("serialize scene");
    let model = root
        .children()
        .first()
        .expect("one-image scene has one child");
    let mut writer = MemoryBinaryWriter::new();
    TiffBackend
        .serialize_model(model, &mut writer)
        .expect("serialize TIFF");
    writer.take_buffer()
}

/// Cycles are permitted: the chosen contract does not impose an arbitrary DAG
/// restriction; a record may use an entity produced by another record and
/// entities may participate in multiple records.
#[test]
fn provenance_cycles_are_permitted() {
    let mut scene = Scene::new();
    scene.add_data_object(DataObject::new());
    let p1 = scene.add_process_record(ProcessRecord::new());
    let p2 = scene.add_process_record(ProcessRecord::new());

    // p1 generates the data object; p2 uses it, and p2 also "uses" the data
    // generated by p1 — ordinary chain. A cycle would require an entity
    // endpoint, which the typed API prevents; the graph simply permits any
    // explicitly declared combination and never rejects a record for
    // participating twice.
    scene
        .add_provenance_relation(p1, ProvenanceRelationKind::Generated, data0())
        .unwrap();
    scene
        .add_provenance_relation(p2, ProvenanceRelationKind::Used, data0())
        .unwrap();
    scene
        .add_provenance_relation(p1, ProvenanceRelationKind::Used, data0())
        .unwrap();
    assert_eq!(scene.provenance_relation_count(), 3);
}

/// Invalid process reference rejected (negative).
#[test]
fn invalid_process_reference_is_rejected() {
    let (mut scene, _) = sample_scene();
    let err = scene
        .add_provenance_relation(
            ProcessRecordId::new(7),
            ProvenanceRelationKind::Generated,
            product0(),
        )
        .expect_err("unknown process id must fail");
    assert_eq!(err.code(), ErrorCode::NotFound);
}
