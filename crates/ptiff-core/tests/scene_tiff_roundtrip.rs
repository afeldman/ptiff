//! End-to-end proof that the PTIFF domain types are wired through the full
//! `Scene` -> `StorageModel` -> TIFF bytes -> `StorageModel` -> `Scene` path
//! (Phase 2 DoD: "Domain-Modell-Tests grün, Metadaten-Vergleich-Roundtrips
//! mit C++-Referenz grün").
//!
//! `Scene` itself owns `Image`s (mirroring the C++ `Scene::addImage` API); the
//! optional camera / CRS domains hang off each `ImageDescriptor` and must
//! survive a real TIFF round-trip (tags 65002 / 65003), not just an in-memory
//! `StorageModel` pass.
//!
//! The `SceneSerializer` emits a tree (`StorageModel` root with one child per
//! image). The TIFF backend's `serialize_model` consumes a *single-image*
//! model (fields on the node itself), so the round-trip helper passes the
//! first image child on to the backend — the single-image case these tests
//! exercise.

use ptiff_core::geometry::{
    Camera, CoordinateReferenceSystem, Extrinsics, Frame, Intrinsics, Planet, Projection,
};
use ptiff_core::io::backend::tiff::TiffBackend;
use ptiff_core::io::{
    Deserializer, MemoryBinaryReader, MemoryBinaryWriter, SceneDeserializer, SceneSerializer,
    Serializer, StorageBackend, StorageModel,
};
use ptiff_core::{CompressionKind, Ellipsoid, ImageDescriptorBuilder, Scene};

fn camera_fixture() -> Camera {
    Camera::from_model(
        "pinhole",
        Intrinsics::new(700.0, 715.0, 8.0, 8.0),
        Extrinsics::new(
            ptiff_core::geometry::Quaternion::IDENTITY,
            ptiff_core::geometry::Vec3::new(1.0, 2.0, 3.0),
        ),
        "2026-01-01T00:00:00Z",
    )
}

/// Writes a scene through the TIFF backend and reads a fresh `Scene` back.
/// Each scene under test carries exactly one image.
fn scene_tiff_roundtrip(scene: &Scene) -> Scene {
    let model: StorageModel = SceneSerializer.serialize(scene).expect("serialize scene");
    let image_child = model
        .children()
        .first()
        .expect("scene has exactly one image");

    let mut writer = MemoryBinaryWriter::new();
    TiffBackend
        .serialize_model(image_child, &mut writer)
        .expect("serialize to TIFF");
    let bytes = writer.take_buffer();

    let mut reader = MemoryBinaryReader::from_slice(&bytes);
    let back_model = TiffBackend
        .deserialize_model(&mut reader)
        .expect("deserialize TIFF");
    SceneDeserializer
        .deserialize(&back_model)
        .expect("deserialize scene")
}

#[test]
fn scene_with_camera_survives_full_tiff_roundtrip() {
    let mut scene = Scene::new();
    let descriptor = ImageDescriptorBuilder::new(200, 100)
        .channel_count(3)
        .compression(Some(CompressionKind::Lzw))
        .camera(Some(camera_fixture()))
        .build();
    scene.add_image(descriptor).unwrap();

    let back = scene_tiff_roundtrip(&scene);

    assert_eq!(back.image_count(), 1);
    let img = back.image_at(0).unwrap();
    assert_eq!(img.width(), 200);
    assert_eq!(img.height(), 100);
    assert_eq!(img.channel_count(), 3);

    // The camera domain (tag 65002) must be re-derivable as a typed value.
    let cam = img.camera().expect("camera survives round-trip");
    assert_eq!(cam.model_name(), "pinhole");
    let i = cam.intrinsics();
    assert_eq!(i.focal_length_pixels_x, 700.0);
    assert_eq!(i.focal_length_pixels_y, 715.0);
    assert_eq!(cam.timestamp(), "2026-01-01T00:00:00Z");
}

#[test]
fn scene_with_crs_survives_full_tiff_roundtrip() {
    let mut scene = Scene::new();
    let crs = CoordinateReferenceSystem::new(
        Planet::new("Moon", "301", Ellipsoid::UNSPECIFIED, Frame::IAU_MOON),
        Some(Frame::new("IAU_MOON")),
        Projection::new(ptiff_core::geometry::ProjectionKind::Equirectangular),
    );
    let descriptor = ImageDescriptorBuilder::new(64, 64).crs(Some(crs)).build();
    scene.add_image(descriptor).unwrap();

    let back = scene_tiff_roundtrip(&scene);

    assert_eq!(back.image_count(), 1);
    let img = back.image_at(0).unwrap();
    let roundtripped = img.crs().expect("CRS survives round-trip");
    assert_eq!(roundtripped.planet().iau_identifier(), "301");
    assert_eq!(roundtripped.frame().id(), "IAU_MOON");
}
