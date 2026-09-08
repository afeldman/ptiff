//! CM-02 (WP-CM-02): explicit directed semantic relationship model.
//!
//! Relationships are Scene-owned, explicit graph edges between Core Model
//! entities (Observation/DataObject/Product), never implicit fields on the
//! entities. Endpoints keep typed Scene-scoped identity, foreign Scene ids
//! are rejected, duplicates are rejected, iteration is deterministic, and the
//! relationship collection has zero effect on 1.x serialization.

use ptiff_core::image::ImageDescriptorBuilder;
use ptiff_core::io::backend::tiff::TiffBackend;
use ptiff_core::io::{
    MemoryBinaryWriter, SceneSerializer, Serializer, StorageBackend, StorageModel,
};
use ptiff_core::{
    DataObject, EntityRef, ErrorCode, ImageDescriptor, Observation, PixelType, Product,
    Relationship, RelationshipKind, Scene,
};

fn desc(width: u32, height: u32) -> ImageDescriptor {
    ImageDescriptorBuilder::new(width, height)
        .pixel_type(PixelType::UInt8)
        .channel_count(1)
        .build()
}

/// Scene containing one observation, one data object and one product.
fn sample_scene() -> Scene {
    let mut scene = Scene::new();
    scene.add_observation(Observation::new());
    scene.add_data_object(DataObject::new());
    scene.add_product(Product::new());
    scene
}

/// A. Observation → DataObject (kind Produces).
#[test]
fn observation_produces_data_object() {
    let mut scene = sample_scene();
    let source = EntityRef::Observation(ptiff_core::ObservationId::new(0));
    let target = EntityRef::DataObject(ptiff_core::DataObjectId::new(0));

    scene
        .add_relationship(source, RelationshipKind::Produces, target)
        .expect("observation -> data object must be permitted");

    let rels = scene.relationships();
    assert_eq!(rels.len(), 1);
    assert_eq!(rels[0].source(), source);
    assert_eq!(rels[0].kind(), RelationshipKind::Produces);
    assert_eq!(rels[0].target(), target);
}

/// B. DataObject → Product (kind DerivedFrom, reading "product derived from
/// this data object").
#[test]
fn data_object_feeds_product() {
    let mut scene = sample_scene();
    let source = EntityRef::DataObject(ptiff_core::DataObjectId::new(0));
    let target = EntityRef::Product(ptiff_core::ProductId::new(0));

    scene
        .add_relationship(source, RelationshipKind::DerivedFrom, target)
        .expect("data object -> product must be permitted");

    let rels = scene.relationships();
    assert_eq!(rels.len(), 1);
    assert_eq!(rels[0].source(), source);
    assert_eq!(rels[0].kind(), RelationshipKind::DerivedFrom);
    assert_eq!(rels[0].target(), target);
}

/// C. Product → Product chains (kind DerivedFrom).
#[test]
fn product_derives_product_chain() {
    let mut scene = Scene::new();
    scene.add_product(Product::new()); // id 0
    scene.add_product(Product::new()); // id 1

    let first = EntityRef::Product(ptiff_core::ProductId::new(0));
    let second = EntityRef::Product(ptiff_core::ProductId::new(1));
    scene
        .add_relationship(first, RelationshipKind::DerivedFrom, second)
        .expect("product -> product chain must be permitted");
    assert_eq!(scene.relationship_count(), 1);
}

/// D. Directionality: A → B never implies B → A.
#[test]
fn relationships_are_directional() {
    let mut scene = sample_scene();
    let obs = EntityRef::Observation(ptiff_core::ObservationId::new(0));
    let data = EntityRef::DataObject(ptiff_core::DataObjectId::new(0));

    scene
        .add_relationship(obs, RelationshipKind::Produces, data)
        .unwrap();

    // Forward query sees the edge; the reverse does not.
    assert_eq!(scene.relationships_from(obs).len(), 1);
    assert_eq!(scene.relationships_to(obs).len(), 0);
    assert_eq!(scene.relationships_to(data).len(), 1);
    assert_eq!(scene.relationships_from(data).len(), 0);

    // Reversed edge is a different relationship (different endpoints), and
    // here it is also domain-invalid (data object cannot produce an
    // observation) — direction is preserved, never inverted.
    let err = scene
        .add_relationship(data, RelationshipKind::Produces, obs)
        .expect_err("reverse edge must not be silently created");
    assert_eq!(err.code(), ErrorCode::InvalidArgument);
}

/// E. Typed endpoint safety: EntityRef variants preserve typed ids.
#[test]
fn endpoints_keep_typed_identity() {
    let scene = sample_scene();
    let obs_id = ptiff_core::ObservationId::new(0);
    let data_id = ptiff_core::DataObjectId::new(0);

    // Same numeric value in two domains remains distinguishable by type.
    assert_eq!(obs_id.value(), data_id.value());
    let a = EntityRef::Observation(obs_id);
    let b = EntityRef::DataObject(data_id);
    assert_ne!(a, b);
    assert_eq!(a.value(), b.value());

    // A typed id cannot be handed to the wrong domain's lookup (compile-time
    // guarantee); EntityRef construction is the only place where domains
    // meet, and it preserves each typed id inside its variant.
    match a {
        EntityRef::Observation(id) => {
            assert_eq!(scene.observation(id).unwrap().clone(), Observation::new())
        }
        _ => panic!("variant mismatch"),
    }
    match b {
        EntityRef::DataObject(id) => {
            assert_eq!(scene.data_object(id).unwrap().clone(), DataObject::new())
        }
        _ => panic!("variant mismatch"),
    }
}

/// F. Scene ownership: relationship creation validates endpoints against the
/// Scene's own domain. Out-of-domain (foreign) ids are rejected even when the
/// caller obtained them from another Scene.
///
/// Note on handle provenance: `Id<Tag>` carries no minting provenance
/// (P0-05) — a handle from another Scene whose numeric value coincides with a
/// local id is structurally equal to the local id and therefore *denotes the
/// local entity* when used here. Cross-scene comparison is meaningless by
/// design; the graph remains internally coherent. Ids outside the local
/// domain are always rejected (NotFound).
#[test]
fn foreign_scene_ids_are_rejected() {
    let mut scene_a = sample_scene();
    let mut scene_b = sample_scene();
    scene_b.add_data_object(DataObject::new()); // scene_b data domain now 0..2

    let a_obs = EntityRef::Observation(ptiff_core::ObservationId::new(0));
    // scene_b's second data object (id 1) does not exist in scene_a.
    let b_data = EntityRef::DataObject(ptiff_core::DataObjectId::new(1));

    let err = scene_a
        .add_relationship(a_obs, RelationshipKind::Produces, b_data)
        .expect_err("foreign data object must be rejected");
    assert_eq!(err.code(), ErrorCode::NotFound);

    // Symmetric: an observation id that exceeds scene_b's observation domain.
    let a_obs_outside = EntityRef::Observation(ptiff_core::ObservationId::new(3));
    let b_data0 = EntityRef::DataObject(ptiff_core::DataObjectId::new(0));
    let err = scene_b
        .add_relationship(a_obs_outside, RelationshipKind::Produces, b_data0)
        .expect_err("foreign observation must be rejected");
    assert_eq!(err.code(), ErrorCode::NotFound);

    assert_eq!(scene_a.relationship_count(), 0);
    assert_eq!(scene_b.relationship_count(), 0);
}

/// G. Duplicate policy: an exact duplicate edge is rejected explicitly;
/// distinct edges (different source, target or kind) are independent.
#[test]
fn duplicate_relationships_are_rejected_explicitly() {
    let mut scene = Scene::new();
    scene.add_observation(Observation::new());
    scene.add_data_object(DataObject::new()); // 0
    scene.add_data_object(DataObject::new()); // 1

    let obs = EntityRef::Observation(ptiff_core::ObservationId::new(0));
    let data0 = EntityRef::DataObject(ptiff_core::DataObjectId::new(0));
    let data1 = EntityRef::DataObject(ptiff_core::DataObjectId::new(1));

    scene
        .add_relationship(obs, RelationshipKind::Produces, data0)
        .unwrap();

    // Exact duplicate -> InvalidArgument (no silent set semantics).
    let err = scene
        .add_relationship(obs, RelationshipKind::Produces, data0)
        .expect_err("exact duplicate must be rejected");
    assert_eq!(err.code(), ErrorCode::InvalidArgument);
    assert_eq!(scene.relationship_count(), 1);

    // Same source, different target -> allowed.
    scene
        .add_relationship(obs, RelationshipKind::Produces, data1)
        .unwrap();
    assert_eq!(scene.relationship_count(), 2);
}

/// H. Deterministic ordering: queries return stable insertion-order results.
#[test]
fn relationship_ordering_is_deterministic() {
    let mut scene = Scene::new();
    scene.add_observation(Observation::new());
    scene.add_data_object(DataObject::new()); // 0
    scene.add_data_object(DataObject::new()); // 1
    scene.add_data_object(DataObject::new()); // 2

    let obs = EntityRef::Observation(ptiff_core::ObservationId::new(0));
    let d0 = EntityRef::DataObject(ptiff_core::DataObjectId::new(0));
    let d1 = EntityRef::DataObject(ptiff_core::DataObjectId::new(1));
    let d2 = EntityRef::DataObject(ptiff_core::DataObjectId::new(2));

    scene
        .add_relationship(obs, RelationshipKind::Produces, d1)
        .unwrap();
    scene
        .add_relationship(obs, RelationshipKind::Produces, d0)
        .unwrap();
    scene
        .add_relationship(obs, RelationshipKind::Produces, d2)
        .unwrap();

    let expected: Vec<EntityRef> = vec![d1, d0, d2];
    let all = scene.relationships();
    let from: Vec<EntityRef> = scene
        .relationships_from(obs)
        .iter()
        .map(|r| r.target())
        .collect();
    assert_eq!(from, expected); // insertion order, not sorted/hash order

    // Repeated queries are byte-for-byte stable.
    let again: Vec<EntityRef> = scene
        .relationships_from(obs)
        .iter()
        .map(|r| r.target())
        .collect();
    assert_eq!(from, again);
    assert_eq!(all.len(), 3);
}

/// I. Adding relationships never changes existing entity ids (all families).
#[test]
fn relationships_do_not_mutate_entity_identity() {
    let mut scene = Scene::new();
    let image = scene.add_image(desc(8, 8)).unwrap();
    let obs = scene.add_observation(Observation::new());
    let data = scene.add_data_object(DataObject::new());
    let prod = scene.add_product(Product::new());

    scene
        .add_relationship(
            EntityRef::Observation(obs),
            RelationshipKind::Produces,
            EntityRef::DataObject(data),
        )
        .unwrap();
    scene
        .add_relationship(
            EntityRef::DataObject(data),
            RelationshipKind::DerivedFrom,
            EntityRef::Product(prod),
        )
        .unwrap();

    assert_eq!(scene.image(image).unwrap().width(), 8);
    assert!(scene.observation(obs).is_ok());
    assert!(scene.data_object(data).is_ok());
    assert!(scene.product(prod).is_ok());
    assert_eq!(scene.relationship_count(), 2);
}

/// J. 1.x serialization invariance: adding relationships changes zero bytes
/// of the 1.x TIFF output.
#[test]
fn relationships_never_change_1x_output() {
    let mut plain = Scene::new();
    plain.add_image(desc(16, 16)).unwrap();
    plain.add_observation(Observation::new());
    plain.add_data_object(DataObject::new());
    plain.add_product(Product::new());

    let related = build_related_scene();

    assert_eq!(
        serialize_first_image(&plain),
        serialize_first_image(&related)
    );
}

fn build_related_scene() -> Scene {
    let mut scene = Scene::new();
    scene.add_image(desc(16, 16)).unwrap();
    let obs = scene.add_observation(Observation::new());
    let data = scene.add_data_object(DataObject::new());
    let prod = scene.add_product(Product::new());
    scene
        .add_relationship(
            EntityRef::Observation(obs),
            RelationshipKind::Produces,
            EntityRef::DataObject(data),
        )
        .unwrap();
    scene
        .add_relationship(
            EntityRef::DataObject(data),
            RelationshipKind::DerivedFrom,
            EntityRef::Product(prod),
        )
        .unwrap();
    scene
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

/// Negative: disallowed endpoint combinations are rejected (never silently
/// become valid). The frozen architecture permits only the three CM-02 edges.
#[test]
fn disallowed_endpoint_combinations_are_rejected() {
    let mut scene = Scene::new();
    scene.add_observation(Observation::new());
    scene.add_data_object(DataObject::new());
    scene.add_product(Product::new());
    scene.add_product(Product::new()); // product id 1

    let obs = EntityRef::Observation(ptiff_core::ObservationId::new(0));
    let data = EntityRef::DataObject(ptiff_core::DataObjectId::new(0));
    let prod0 = EntityRef::Product(ptiff_core::ProductId::new(0));
    let prod1 = EntityRef::Product(ptiff_core::ProductId::new(1));

    let invalid = [
        (obs, RelationshipKind::Produces, obs), // Observation -> Observation
        (obs, RelationshipKind::DerivedFrom, prod0), // Observation -> Product
        (data, RelationshipKind::Produces, obs), // DataObject -> Observation
        (prod0, RelationshipKind::Produces, obs), // Product -> Observation
        (prod0, RelationshipKind::Produces, prod1), // Product -> Product via wrong kind
    ];
    for (source, kind, target) in invalid {
        let err = scene
            .add_relationship(source, kind, target)
            .expect_err("disallowed combination must fail");
        assert_eq!(err.code(), ErrorCode::InvalidArgument);
    }
    assert_eq!(scene.relationship_count(), 0);
}

/// Value-object shape: Relationship exposes typed source/kind/target.
#[test]
fn relationship_value_round_trips() {
    let r = Relationship::new(
        EntityRef::Observation(ptiff_core::ObservationId::new(7)),
        RelationshipKind::Produces,
        EntityRef::DataObject(ptiff_core::DataObjectId::new(3)),
    );
    assert_eq!(
        r.source(),
        EntityRef::Observation(ptiff_core::ObservationId::new(7))
    );
    assert_eq!(r.kind(), RelationshipKind::Produces);
    assert_eq!(
        r.target(),
        EntityRef::DataObject(ptiff_core::DataObjectId::new(3))
    );
}
