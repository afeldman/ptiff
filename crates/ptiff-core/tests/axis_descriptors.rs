//! AX-01 (WP-AX-01): semantic axis descriptors on DataObjects.
//!
//! A DataObject declares its scientific dimensions as ordered semantic
//! [`AxisDescriptor`]s — never as bare array shape. Axes are semantic meaning
//! (kind + logical extent + deterministic order), explicitly distinct from
//! TIFF width/height, storage order and CRS. Axis content is not identity;
//! the 1.x serializer is untouched (semantic DataObjects are never written
//! through it).

use ptiff_core::image::ImageDescriptorBuilder;
use ptiff_core::io::backend::tiff::TiffBackend;
use ptiff_core::io::{
    MemoryBinaryWriter, SceneSerializer, Serializer, StorageBackend, StorageModel,
};
use ptiff_core::{
    AxisDescriptor, AxisKind, DataObject, ErrorCode, ImageDescriptor, PixelType, Scene,
};

fn desc(width: u32, height: u32) -> ImageDescriptor {
    ImageDescriptorBuilder::new(width, height)
        .pixel_type(PixelType::UInt8)
        .channel_count(1)
        .build()
}

/// An image-like 2-D + band DataObject.
fn spectral_image(extent_y: u64, extent_x: u64, bands: u64) -> DataObject {
    let mut d = DataObject::new();
    d.add_axis(AxisDescriptor::new(AxisKind::X, extent_x).unwrap())
        .unwrap();
    d.add_axis(AxisDescriptor::new(AxisKind::Y, extent_y).unwrap())
        .unwrap();
    d.add_axis(AxisDescriptor::new(AxisKind::Band, bands).unwrap())
        .unwrap();
    d
}

/// A. Axis creation: valid descriptors with every canonical kind.
#[test]
fn axis_descriptors_construct_for_all_canonical_kinds() {
    for (kind, extent) in [
        (AxisKind::X, 1024u64),
        (AxisKind::Y, 768),
        (AxisKind::Band, 7),
        (AxisKind::Time, 24),
        (AxisKind::Polarization, 4),
    ] {
        let axis = AxisDescriptor::new(kind, extent).expect("valid axis");
        assert_eq!(axis.kind(), kind);
        assert_eq!(axis.extent(), extent);
    }
}

/// C. Extent: zero extent is structurally invalid; huge extents are fine
/// (u64 arithmetic, no overflow path).
#[test]
fn axis_extent_validation() {
    let err = AxisDescriptor::new(AxisKind::X, 0).unwrap_err();
    assert_eq!(err.code(), ErrorCode::InvalidArgument);

    let big = AxisDescriptor::new(AxisKind::Band, u64::MAX).unwrap();
    assert_eq!(big.extent(), u64::MAX);
}

/// E + F. DataObject integration: axes attach and are retrieved in
/// deterministic order; a multidimensional object can declare X/Y/Band.
#[test]
fn data_object_axes_are_ordered_and_retrievable() {
    let mut d = DataObject::new();
    d.add_axis(AxisDescriptor::new(AxisKind::X, 512).unwrap())
        .unwrap();
    d.add_axis(AxisDescriptor::new(AxisKind::Y, 512).unwrap())
        .unwrap();
    d.add_axis(AxisDescriptor::new(AxisKind::Band, 3).unwrap())
        .unwrap();

    assert_eq!(d.axis_count(), 3);
    let kinds: Vec<AxisKind> = d.axes().iter().map(|a| a.kind()).collect();
    assert_eq!(kinds, vec![AxisKind::X, AxisKind::Y, AxisKind::Band]);

    // Lookup by kind returns the single declared axis.
    assert_eq!(d.axis(AxisKind::Band).unwrap().extent(), 3);
    assert_eq!(d.axis(AxisKind::Time), None);
}

/// D + H. Order preservation and deterministic repeated inspection; empty
/// axis collection is the documented "not yet populated" state.
#[test]
fn axis_order_is_deterministic_and_empty_is_allowed() {
    let empty = DataObject::new();
    assert_eq!(empty.axis_count(), 0);
    assert!(empty.axes().is_empty());

    let a = spectral_image(32, 64, 4);
    let b = spectral_image(32, 64, 4);
    // Equivalent explicit construction is deterministic.
    assert_eq!(a.axes(), b.axes());
    let kinds_a: Vec<AxisKind> = a.axes().iter().map(|x| x.kind()).collect();
    let kinds_b: Vec<AxisKind> = b.axes().iter().map(|x| x.kind()).collect();
    assert_eq!(kinds_a, kinds_b);
}

/// G. Axis content is not DataObject identity: identical axes, distinct ids
/// = distinct entities.
#[test]
fn axes_do_not_define_data_object_identity() {
    let mut scene = Scene::new();
    let first = scene.add_data_object(spectral_image(16, 16, 7));
    let second = scene.add_data_object(spectral_image(16, 16, 7));

    // Both objects are separately retrievable with the same scientific shape;
    // equal axis content does not conflate the two distinct entities (their
    // identity comes from the DataObjectId, never from axes).
    assert_ne!(first, second); // ids differ
    let a = scene.data_object(first).unwrap();
    let b = scene.data_object(second).unwrap();
    assert_eq!(a.axes(), b.axes());
    assert_eq!(a, b); // identical value content...
    assert_ne!(first, second); // ...but distinct entities by Scene id
}

/// I. Uniqueness: one axis kind per DataObject.
#[test]
fn duplicate_axis_kind_is_rejected() {
    let mut d = DataObject::new();
    d.add_axis(AxisDescriptor::new(AxisKind::X, 64).unwrap())
        .unwrap();

    let err = d
        .add_axis(AxisDescriptor::new(AxisKind::X, 128).unwrap())
        .expect_err("duplicate axis kind must be rejected");
    assert_eq!(err.code(), ErrorCode::InvalidArgument);

    // A different kind is fine.
    d.add_axis(AxisDescriptor::new(AxisKind::Y, 64).unwrap())
        .unwrap();
    assert_eq!(d.axis_count(), 2);
}

/// J. Identity stability: DataObjectId allocation is untouched by axis
/// content; adding later axis-bearing objects never renumbers earlier ids.
#[test]
fn axes_do_not_change_identity_allocation() {
    let mut scene = Scene::new();
    let plain = scene.add_data_object(DataObject::new());
    let with_axes = scene.add_data_object(spectral_image(8, 8, 3));

    assert_eq!(plain.value(), 0);
    assert_eq!(with_axes.value(), 1);
    assert!(scene.data_object(plain).is_ok());
    assert!(scene.data_object(with_axes).is_ok());
}

/// Copy semantics: cloning a DataObject copies its axis content structurally;
/// mutating the clone never mutates the original.
#[test]
fn data_object_clone_preserves_axes_without_aliasing() {
    let original = spectral_image(10, 20, 2);
    let mut clone = original.clone();

    assert_eq!(original.axes(), clone.axes());
    clone
        .add_axis(AxisDescriptor::new(AxisKind::Time, 5).unwrap())
        .unwrap();

    assert_eq!(clone.axis_count(), 4);
    assert_eq!(original.axis_count(), 3);
    assert_eq!(original.axis(AxisKind::Time), None);
}

/// K. 1.x serialization invariance: semantic DataObjects (with or without
/// axes) are never serialized through the 1.x path — a Scene carrying them
/// produces byte-identical 1.x TIFF output whether the axes exist or not.
#[test]
fn axes_never_change_1x_output() {
    let mut plain = Scene::new();
    plain.add_image(desc(16, 16)).unwrap();
    plain.add_data_object(DataObject::new());

    let mut axed = Scene::new();
    axed.add_image(desc(16, 16)).unwrap();
    axed.add_data_object(spectral_image(16, 16, 7));

    assert_eq!(serialize_first_image(&plain), serialize_first_image(&axed));
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
