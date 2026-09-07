//! CM-01 (WP-CM-01): Core Model semantic entities.
//!
//! Proves the semantic foundation of PTIFF 2.0:
//!
//! ```text
//! Observation  = measurement/acquisition event
//! DataObject   = concrete scientific data representation
//! Product      = scientific/derived result
//! ```
//!
//! with Scene-scoped typed identity, strict type separation, stability of all
//! existing id families, deterministic allocation, and **zero serialization
//! impact** on the 1.x model.

use ptiff_core::image::ImageDescriptorBuilder;
use ptiff_core::io::backend::tiff::TiffBackend;
use ptiff_core::io::{
    MemoryBinaryWriter, SceneSerializer, Serializer, StorageBackend, StorageModel,
};
use ptiff_core::{
    Camera, DataObject, DataObjectId, ErrorCode, ImageDescriptor, Observation, ObservationId,
    PixelType, Product, ProductId, Scene,
};

fn desc(width: u32, height: u32) -> ImageDescriptor {
    ImageDescriptorBuilder::new(width, height)
        .pixel_type(PixelType::UInt8)
        .channel_count(1)
        .build()
}

/// A. Entity creation: the Scene creates entities of each kind and returns
/// typed ids.
#[test]
fn scene_creates_core_entities_with_typed_ids() {
    let mut scene = Scene::new();
    let o = scene.add_observation(Observation::new());
    let d = scene.add_data_object(DataObject::new());
    let p = scene.add_product(Product::new());

    assert_eq!(scene.observation_count(), 1);
    assert_eq!(scene.data_object_count(), 1);
    assert_eq!(scene.product_count(), 1);

    // Typed id domains start their own counters at zero.
    assert_eq!(o.value(), 0);
    assert_eq!(d.value(), 0);
    assert_eq!(p.value(), 0);
}

/// B. Lookup: every entity kind is retrievable through its own typed id.
#[test]
fn core_entities_are_retrievable_by_typed_id() {
    let mut scene = Scene::new();
    let o = scene.add_observation(Observation::new());
    let d = scene.add_data_object(DataObject::new());
    let p = scene.add_product(Product::new());

    assert_eq!(scene.observation(o).unwrap(), &Observation::new());
    assert_eq!(scene.data_object(d).unwrap(), &DataObject::new());
    assert_eq!(scene.product(p).unwrap(), &Product::new());

    // Foreign/missing ids behave like the existing image/camera/geometry
    // lookups: NotFound.
    assert_eq!(
        scene.observation(ObservationId::new(9)).unwrap_err().code(),
        ErrorCode::NotFound
    );
    assert_eq!(
        scene.data_object(DataObjectId::new(9)).unwrap_err().code(),
        ErrorCode::NotFound
    );
    assert_eq!(
        scene.product(ProductId::new(9)).unwrap_err().code(),
        ErrorCode::NotFound
    );
}

/// C. Uniqueness: multiple entities of the same kind get distinct ids.
#[test]
fn same_kind_entities_get_distinct_ids() {
    let mut scene = Scene::new();
    let o0 = scene.add_observation(Observation::new());
    let o1 = scene.add_observation(Observation::new());
    let d0 = scene.add_data_object(DataObject::new());
    let d1 = scene.add_data_object(DataObject::new());
    let p0 = scene.add_product(Product::new());
    let p1 = scene.add_product(Product::new());

    assert_ne!(o0, o1);
    assert_ne!(d0, d1);
    assert_ne!(p0, p1);
    assert_eq!(scene.observation_count(), 2);
    assert_eq!(scene.data_object_count(), 2);
    assert_eq!(scene.product_count(), 2);
}

/// D. Scope: two Scenes may independently mint the same numeric local value
/// without implying shared identity (P0-05 contract carried into the Core
/// Model).
#[test]
fn local_id_scope_is_per_scene() {
    let mut scene_a = Scene::new();
    let mut scene_b = Scene::new();
    let a = scene_a.add_observation(Observation::new());
    let b = scene_b.add_observation(Observation::new());

    // Same numeric value in two Scenes — but each Scene resolves only its own
    // handle.
    assert_eq!(a.value(), b.value());
    assert!(scene_a.observation(a).is_ok());
    assert!(scene_b.observation(b).is_ok());
    // A Scene cannot resolve an ObservationId minted elsewhere when its own
    // domain is empty.
    let empty = Scene::new();
    assert_eq!(
        empty.observation(a).unwrap_err().code(),
        ErrorCode::NotFound
    );
}

/// E. Type separation: Observation/DataObject/Product are distinct Rust types
/// with distinct typed ids. Accidental interchange is a compile-time error —
/// this test demonstrates the runtime consequence: each typed id domain
/// allocates and resolves independently, and a numeric value that is valid in
/// one domain is meaningless in the others.
#[test]
fn type_separation_keeps_domains_independent() {
    let mut scene = Scene::new();
    let o = scene.add_observation(Observation::new());
    let d = scene.add_data_object(DataObject::new());
    let p = scene.add_product(Product::new());

    // Same numeric value across domains names different entities.
    assert_eq!(o.value(), d.value());
    assert_eq!(d.value(), p.value());

    // Domain-typed lookup functions cannot be crossed at compile time; the
    // following would not type-check (observation() takes ObservationId):
    //     let _ = scene.data_object(o);
    //     let _ = scene.product(d);
    // Instead, verify each handle is only valid in its own domain by value.
    assert!(scene.observation(o).is_ok());
    assert!(scene.data_object(d).is_ok());
    assert!(scene.product(p).is_ok());
}

/// F. Stability: creating Core Model entities never renumbers existing 1.x
/// ids (images/cameras/geometries), and later Core Model entities never
/// renumber earlier ones.
#[test]
fn entity_ids_are_stable_across_additions() {
    let mut scene = Scene::new();
    let image = scene.add_image(desc(8, 8)).unwrap();
    let camera = scene.add_camera(Camera::new());
    let o0 = scene.add_observation(Observation::new());

    // Unrelated additions in every family.
    scene.add_image(desc(16, 16)).unwrap();
    scene.add_camera(Camera::new());
    let o1 = scene.add_observation(Observation::new());
    let d0 = scene.add_data_object(DataObject::new());
    let p0 = scene.add_product(Product::new());

    assert_eq!(scene.image(image).unwrap().width(), 8);
    assert_eq!(scene.camera(camera).unwrap().model_name(), "pinhole");
    assert!(scene.observation(o0).is_ok());
    assert!(scene.observation(o1).is_ok());
    assert!(scene.data_object(d0).is_ok());
    assert!(scene.product(p0).is_ok());
}

/// G. Existing model preservation: a Scene with Core Model entities serializes
/// through the 1.x path exactly like the same Scene without them — no tags,
/// no manifest, no storage change.
#[test]
fn core_entities_never_change_1x_output() {
    let mut plain = Scene::new();
    plain.add_image(desc(16, 16)).unwrap();

    let mut with_core = Scene::new();
    with_core.add_image(desc(16, 16)).unwrap();
    with_core.add_observation(Observation::new());
    with_core.add_data_object(DataObject::new());
    with_core.add_product(Product::new());

    let plain_bytes = serialize_first_image(&plain);
    let core_bytes = serialize_first_image(&with_core);
    assert_eq!(plain_bytes, core_bytes);
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

/// H. Determinism: Core Model allocation is deterministic counter allocation;
/// repeated identical creation yields identical state, and entity presence
/// has no hash/address dependence (storage Vecs in insertion order).
#[test]
fn core_entity_allocation_is_deterministic() {
    fn build() -> Scene {
        let mut scene = Scene::new();
        scene.add_observation(Observation::new());
        scene.add_data_object(DataObject::new());
        scene.add_product(Product::new());
        scene
    }
    let a = build();
    let b = build();
    assert_eq!(a.observation_count(), b.observation_count());
    assert_eq!(a.data_object_count(), b.data_object_count());
    assert_eq!(a.product_count(), b.product_count());
    // Lookup by freshly minted ids is stable across identical scenes.
    assert!(a.observation(ObservationId::new(0)).is_ok());
    assert!(b.observation(ObservationId::new(0)).is_ok());
}
