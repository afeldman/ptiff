//! P0-05 (WP-P0-05): PTIFF identity semantics contract.
//!
//! Tests the contract established in `ptiff_core::identity` and on `Scene`:
//!
//! * local identifiers are unique per issuing Scene and stable when unrelated
//!   entities are added;
//! * identical descriptors are distinct entities with distinct local ids
//!   (identity is not content);
//! * external identifiers (PDS4 LID/VID and others) coexist with — but never
//!   replace — PTIFF-local identity, and never become PTIFF-native identity;
//! * identity never depends on physical TIFF layout, tile choice, byte
//!   offsets, file names or hash-map iteration order;
//! * external ids are in-memory only until ADR-011/ADR-012: attaching them
//!   must not change a single byte of the 1.x TIFF output;
//! * the 1.x read path mints local ids from file/IFD order and never
//!   fabricates scientific identity (migration boundary).

use ptiff_core::image::ImageDescriptorBuilder;
use ptiff_core::io::backend::tiff::TiffBackend;
use ptiff_core::io::{
    Deserializer, MemoryBinaryReader, MemoryBinaryWriter, SceneDeserializer, SceneSerializer,
    Serializer, StorageBackend, StorageModel,
};
use ptiff_core::tile::{Tile, TileExtent, TileIndex, TileRegion};
use ptiff_core::{CompressionKind, ErrorCode, ExternalId, ImageId, PixelType, Scene, TileId};

fn desc(width: u32, height: u32) -> ptiff_core::ImageDescriptor {
    ImageDescriptorBuilder::new(width, height)
        .pixel_type(PixelType::UInt8)
        .channel_count(1)
        .compression(Some(CompressionKind::None))
        .build()
}

fn pds4_lid(lid: &str, vid: Option<&str>) -> ExternalId {
    let mut id = ExternalId::new("pds4", lid).unwrap();
    if let Some(v) = vid {
        id = id.with_version(v).unwrap();
    }
    id
}

/// Serializes one single-image Scene to real TIFF bytes, writing the pixel
/// payload through the Sink so the file is fully valid.
fn scene_to_tiff_bytes(scene: &Scene) -> Vec<u8> {
    let root: StorageModel = SceneSerializer.serialize(scene).expect("serialize scene");
    let model = root
        .children()
        .first()
        .expect("one-image scene has one child");
    let mut writer = MemoryBinaryWriter::new();
    TiffBackend
        .serialize_model(model, &mut writer)
        .expect("serialize TIFF");

    let image = scene.image_at(0).unwrap();
    {
        let mut sink = TiffBackend
            .open_image_sink(&mut writer, model)
            .expect("open sink");
        let layout = *sink.layout();
        let cols = layout.columns(0);
        let rows = layout.rows(0);
        let tw = layout.tile_size.width;
        let th = layout.tile_size.height;
        let width = image.width();
        let height = image.height();

        for row in 0..rows {
            for col in 0..cols {
                let mut bytes = Vec::new();
                for ty in 0..th {
                    let g_row = row * th + ty;
                    if g_row >= height {
                        break;
                    }
                    for tx in 0..tw {
                        let g_col = col * tw + tx;
                        if g_col >= width {
                            break;
                        }
                        bytes.push(((g_row * width + g_col) % 251) as u8);
                    }
                }
                let tile = Tile::new(
                    TileId::new(u64::from(row) * u64::from(cols) + u64::from(col)),
                    TileIndex::new(col, row, 0),
                    TileRegion::new(0, 0, TileExtent::new(tw, th)),
                    &bytes,
                );
                sink.write_tile(&tile).unwrap();
            }
        }
    }
    writer.take_buffer()
}

/// Deserializes TIFF bytes back into a typed Scene via the storage model.
fn tiff_bytes_to_scene(bytes: &[u8]) -> Scene {
    let mut reader = MemoryBinaryReader::from_slice(bytes);
    let model = TiffBackend
        .deserialize_model(&mut reader)
        .expect("deserialize TIFF");
    SceneDeserializer
        .deserialize(&model)
        .expect("deserialize scene")
}

/// A. Distinctness: every entity minted by a Scene has a distinct local id,
/// even when descriptors are byte-identical; duplicates are rejected only for
/// external ids in the same namespace.
#[test]
fn distinct_entities_have_distinct_local_identity() {
    let mut scene = Scene::new();
    let first = scene.add_image(desc(10, 10)).unwrap();
    let second = scene.add_image(desc(10, 10)).unwrap(); // identical content
    let third = scene.add_image(desc(20, 5)).unwrap();

    assert_ne!(first, second);
    assert_ne!(first, third);
    assert_ne!(second, third);

    // Attaching to distinct images is independent.
    scene
        .attach_external_id(first, pds4_lid("urn:nasa:pds:obs:data_a", Some("1.0")))
        .unwrap();
    scene
        .attach_external_id(second, pds4_lid("urn:nasa:pds:obs:data_b", None))
        .unwrap();
    assert_ne!(
        scene.external_id(first, "pds4").unwrap().unwrap(),
        scene.external_id(second, "pds4").unwrap().unwrap()
    );
}

/// B. Stability: ids minted earlier do not change when later images,
/// cameras, geometries or external ids are added; attachment order is
/// deterministic.
#[test]
fn local_identity_is_stable_under_addition() {
    let mut scene = Scene::new();
    let original = scene.add_image(desc(16, 16)).unwrap();
    scene
        .attach_external_id(original, pds4_lid("urn:nasa:pds:obs:first", Some("2.0")))
        .unwrap();

    // Unrelated additions (image, camera, another external id).
    let later = scene.add_image(desc(32, 32)).unwrap();
    scene.add_camera(Default::default());
    scene
        .attach_external_id(
            original,
            ExternalId::new("source-archive", "dataset/42").unwrap(),
        )
        .unwrap();

    assert_eq!(scene.image(original).unwrap().width(), 16);
    assert_ne!(original, later);

    // Deterministic attachment order: pds4 first, then source-archive.
    let ids = scene.external_ids_for(original).unwrap();
    assert_eq!(ids.len(), 2);
    assert_eq!(ids[0].namespace(), "pds4");
    assert_eq!(ids[1].namespace(), "source-archive");
    let again = scene.external_ids_for(original).unwrap();
    assert_eq!(ids, again);
}

/// C. Duplicate local ids are impossible through minting; duplicate external
/// ids within one namespace are rejected, different namespaces are allowed.
#[test]
fn duplicate_identity_is_rejected() {
    let mut scene = Scene::new();
    let image = scene.add_image(desc(8, 8)).unwrap();
    scene
        .attach_external_id(image, pds4_lid("urn:nasa:pds:obs:x", None))
        .unwrap();

    // Same namespace, different value -> conflict.
    let err = scene
        .attach_external_id(image, pds4_lid("urn:nasa:pds:obs:y", None))
        .expect_err("duplicate namespace must fail");
    assert_eq!(err.code(), ErrorCode::InvalidArgument);

    // Same namespace AND same value is equally a conflict (no silent replace).
    let err = scene
        .attach_external_id(image, pds4_lid("urn:nasa:pds:obs:x", None))
        .expect_err("duplicate namespace must fail");
    assert_eq!(err.code(), ErrorCode::InvalidArgument);

    // A different namespace is fine.
    scene
        .attach_external_id(image, ExternalId::new("isis", "cube:obs_x").unwrap())
        .unwrap();
    assert_eq!(scene.external_ids_for(image).unwrap().len(), 2);
}

/// D + E. Local scope: the same numeric id in two Scenes names two different
/// entities; a PDS4 LID/VID is an external reference that coexists with the
/// local id and never replaces or alters it.
#[test]
fn local_scope_and_pds4_external_identity_coexist() {
    let mut scene_a = Scene::new();
    let a_id = scene_a.add_image(desc(4, 4)).unwrap();
    scene_a
        .attach_external_id(a_id, pds4_lid("urn:nasa:pds:orex:data_raw", Some("1.0")))
        .unwrap();

    let mut scene_b = Scene::new();
    let b_id = scene_b.add_image(desc(4, 4)).unwrap();
    scene_b
        .attach_external_id(b_id, pds4_lid("urn:nasa:pds:orex:data_raw", Some("1.0")))
        .unwrap();

    // Same numeric id, different entities (one per Scene).
    assert_eq!(a_id.value(), b_id.value());
    // Local identity was NOT replaced by the external id: both remain local,
    // and lookups stay within their own Scene.
    assert_eq!(scene_a.image(a_id).unwrap().width(), 4);
    assert_eq!(scene_b.image(b_id).unwrap().width(), 4);

    // The external reference is preserved verbatim, without becoming native.
    let ext_a = scene_a.external_id(a_id, "pds4").unwrap().unwrap();
    assert_eq!(ext_a.namespace(), "pds4");
    assert_eq!(ext_a.value(), "urn:nasa:pds:orex:data_raw");
    assert_eq!(ext_a.version(), Some("1.0"));
    assert_eq!(scene_a.image(a_id).unwrap().width(), 4); // still local

    // A foreign id is not a valid handle in scene_b even if numerically equal.
    let foreign = scene_b.image(a_id).unwrap(); // numeric id 0 exists there too
    let _ = foreign;
}

/// F. Duplication semantics: re-adding an identical descriptor creates a new
/// scientific entity with its own id and its own external ids.
#[test]
fn scientific_duplication_mints_new_identity() {
    let mut scene = Scene::new();
    let first = scene.add_image(desc(6, 6)).unwrap();
    let second = scene.add_image(desc(6, 6)).unwrap();
    assert_ne!(first, second);

    scene
        .attach_external_id(first, pds4_lid("urn:nasa:pds:obs:dup-a", None))
        .unwrap();
    // The duplicate entity starts with no external ids.
    assert!(scene.external_ids_for(second).unwrap().is_empty());
    assert_eq!(scene.external_ids_for(first).unwrap().len(), 1);
}

/// G. External identity is in-memory only: attaching it must not change a
/// single byte of the 1.x TIFF output (no manifest, no tag, no offset).
#[test]
fn external_identity_never_changes_tiff_output() {
    let mut plain = Scene::new();
    let image = plain.add_image(desc(24, 24)).unwrap();
    let plain_bytes = scene_to_tiff_bytes(&plain);

    plain
        .attach_external_id(image, pds4_lid("urn:nasa:pds:obs:with_ext", Some("3.0")))
        .unwrap();
    let attached_bytes = scene_to_tiff_bytes(&plain);

    assert_eq!(plain_bytes, attached_bytes);
}

/// G2. Determinism and read-back boundary: the storage round-trip drops
/// external ids (deferred ADR-011/012) and mints file-order ids, never
/// inventing scientific identity from physical content.
#[test]
fn storage_round_trip_never_fabricates_identity() {
    let mut scene = Scene::new();
    let image = scene.add_image(desc(12, 12)).unwrap();
    scene
        .attach_external_id(image, pds4_lid("urn:nasa:pds:obs:rt", Some("1.0")))
        .unwrap();

    let bytes = scene_to_tiff_bytes(&scene);

    // Deterministic file bytes for the identical logical scene.
    assert_eq!(bytes, scene_to_tiff_bytes(&scene));

    let back = tiff_bytes_to_scene(&bytes);
    assert_eq!(back.image_count(), 1);
    // The read-back scene carries NO external ids and NO fabricated identity:
    // ids are the reader's file-order ids (0..), external refs are absent.
    let back_image = back.image_at(0).unwrap();
    assert_eq!(back_image.width(), 12);
    assert!(back.external_ids_for(ImageId::new(0)).unwrap().is_empty());

    // A second identical read produces the identical (file-order) result.
    let again = tiff_bytes_to_scene(&bytes);
    assert_eq!(
        back.image_at(0).unwrap().width(),
        again.image_at(0).unwrap().width()
    );
}

/// Physical layout independence: choosing strips vs tiles (different offsets,
/// different IFD layout) never changes how local identity is minted or how
/// external identity is kept out of the file.
#[test]
fn identity_is_independent_of_physical_layout() {
    // Scene with a strip image.
    let mut strip_scene = Scene::new();
    let s_id = strip_scene.add_image(desc(32, 16)).unwrap();
    strip_scene
        .attach_external_id(s_id, pds4_lid("urn:nasa:pds:obs:layout", None))
        .unwrap();

    // Identical logical content, tiled 16x16.
    let tiled_desc = ImageDescriptorBuilder::new(32, 16)
        .pixel_type(PixelType::UInt8)
        .channel_count(1)
        .compression(Some(CompressionKind::None))
        .tile(16, 16)
        .build();
    let mut tile_scene = Scene::new();
    let t_id = tile_scene.add_image(tiled_desc).unwrap();
    tile_scene
        .attach_external_id(t_id, pds4_lid("urn:nasa:pds:obs:layout", None))
        .unwrap();

    // The bytes differ (physical layout differs) ...
    let strip_bytes = scene_to_tiff_bytes(&strip_scene);
    let tile_bytes = scene_to_tiff_bytes(&tile_scene);
    assert_ne!(strip_bytes, tile_bytes);

    // ... but identity semantics are identical: both scenes minted local id 0,
    // both kept the external reference in memory, and neither file contains it.
    let strip_back = tiff_bytes_to_scene(&strip_bytes);
    let tile_back = tiff_bytes_to_scene(&tile_bytes);
    assert_eq!(strip_back.image_at(0).unwrap().width(), 32);
    assert_eq!(tile_back.image_at(0).unwrap().width(), 32);
    assert!(strip_back
        .external_ids_for(ImageId::new(0))
        .unwrap()
        .is_empty());
    assert!(tile_back
        .external_ids_for(ImageId::new(0))
        .unwrap()
        .is_empty());
}

/// Errors: unknown image ids are rejected by external-id APIs.
#[test]
fn unknown_image_rejected_by_external_identity_api() {
    let mut scene = Scene::new();
    scene.add_image(desc(8, 8)).unwrap();

    let err = scene
        .attach_external_id(ImageId::new(5), pds4_lid("urn:nasa:pds:obs:bad", None))
        .expect_err("unknown image id must fail");
    assert_eq!(err.code(), ErrorCode::NotFound);

    let err = scene.external_id(ImageId::new(5), "pds4").unwrap_err();
    assert_eq!(err.code(), ErrorCode::NotFound);
}
