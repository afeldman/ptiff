//! Concrete [`Deserializer`]: `StorageModel` → `Scene` in the canonical PTIFF
//! field schema.
//!
//! Mirrors `ptiff::io::SceneDeserializer` (see
//! `libptiff/src/io/scene_deserializer.cpp`). Field names, parsing and error
//! messages match the C++ oracle **exactly** (see the oracle's documented
//! read/write asymmetry: serialization emits `"LZW"`, deserialization also
//! accepts `"Lzw"`).
//!
//! **Round-trip limitation:** the canonical schema does not carry `tileInfo` or
//! `groundSampleDistanceMeters`, so images deserialized from a backend-produced
//! model have neither option set -- matching the C++ oracle.

use crate::image::{CompressionKind, ImageDescriptor};
use crate::io::Deserializer;
use crate::pixel_type::PixelType;
use crate::{Error, Result, Scene, StorageModel};

/// Parses a `u32` scalar field, mirroring the C++ `parseU32` helper.
fn parse_u32(node: &StorageModel, key: &str) -> Result<u32> {
    let value = node
        .field(key)
        .map_err(|_| Error::invalid_argument(format!("SceneDeserializer: missing \"{key}\"")))?;

    // `u32::from_str` requires the *whole* string to be canonical digits,
    // matching `std::stoul` + full-consumption check (`pos != size()`).
    let parsed = value.parse::<u32>().map_err(|_| {
        Error::invalid_argument(format!("SceneDeserializer: non-numeric \"{key}\""))
    })?;
    Ok(parsed)
}

/// Parses the canonical pixel-type string (mirrors C++ `parsePixelType`).
fn parse_pixel_type(value: &str) -> Result<PixelType> {
    match value {
        "UInt8" => Ok(PixelType::UInt8),
        "UInt16" => Ok(PixelType::UInt16),
        "UInt32" => Ok(PixelType::UInt32),
        "Float32" => Ok(PixelType::Float32),
        "Float64" => Ok(PixelType::Float64),
        "Int16" => Ok(PixelType::Int16),
        "Int32" => Ok(PixelType::Int32),
        _ => Err(Error::invalid_argument(
            "SceneDeserializer: unrecognized pixelType",
        )),
    }
}

/// Parses the canonical compression string (mirrors C++ `parseCompression`).
///
/// Accepts every codec the typed [`CompressionKind`] vocabulary defines, in
/// both the SceneSerializer spelling ("LZW") and the TIFF directory spelling
/// ("Lzw"); the storage strings "PackBits", "Deflate" and "Jpeg" map 1:1.
fn parse_compression(value: &str) -> Result<CompressionKind> {
    match value {
        "None" => Ok(CompressionKind::None),
        "LZW" | "Lzw" => Ok(CompressionKind::Lzw),
        "PackBits" => Ok(CompressionKind::PackBits),
        "Deflate" => Ok(CompressionKind::Deflate),
        "Jpeg" => Ok(CompressionKind::Jpeg),
        _ => Err(Error::invalid_argument(
            "SceneDeserializer: unrecognized compression",
        )),
    }
}

/// The reference `StorageModel` → `Scene` deserializer.
///
/// Stateless and therefore thread-compatible.
pub struct SceneDeserializer;

impl Deserializer for SceneDeserializer {
    fn deserialize(&self, model: &StorageModel) -> Result<Scene> {
        let mut scene = Scene::new();

        for node in model.children() {
            // Scene-level cameras/geometries are stored as their own child
            // nodes carrying a `ptiff.scene.object_type` marker (see
            // `SceneSerializer`). Reconstruct and continue before the image
            // path below, which assumes image fields (`imageWidth`, ...).
            if let Ok(obj_type) = node.field("ptiff.scene.object_type") {
                match obj_type {
                    "camera" => {
                        if let Some(camera) = crate::geometry::camera_from_model(node)? {
                            scene.add_camera(camera);
                        }
                        continue;
                    }
                    "geometry" => {
                        if let Some(geometry) = crate::geometry::geometry_from_model(node)? {
                            scene.add_geometry(geometry);
                        }
                        continue;
                    }
                    unknown => {
                        return Err(Error::invalid_argument(format!(
                            "SceneDeserializer: unknown ptiff.scene.object_type \"{unknown}\""
                        )));
                    }
                }
            }

            let width = parse_u32(node, "imageWidth")?;
            let height = parse_u32(node, "imageHeight")?;
            let channels = parse_u32(node, "samplesPerPixel")?;

            let pixel_type_field = node
                .field("pixelType")
                .map_err(|_| Error::invalid_argument("SceneDeserializer: missing \"pixelType\""))?;
            let pixel_type = parse_pixel_type(pixel_type_field)?;

            let mut descriptor = ImageDescriptor {
                width,
                height,
                pixel_type,
                channel_count: channels,
                ..ImageDescriptor::default()
            };

            match node.field("compression") {
                Ok(comp) => {
                    let kind = parse_compression(comp)?;
                    descriptor.compression = Some(kind);
                }
                Err(_) => descriptor.compression = Some(CompressionKind::None),
            }

            // PTIFF extension domains (camera + CRS) reconstructed from the
            // `ptiff.<domain>.<key>` fields the TIFF backend decodes from the
            // private tags 65002 / 65003. A domain the file doesn't carry stays
            // `None`.
            descriptor.camera = crate::geometry::camera_from_model(node)?;
            descriptor.crs = crate::geometry::crs_from_model(node)?;

            // Generic PTIFF extension metadata (RFC-7002): every `ptiff.*`
            // field on the node except the raw keys of a *successfully
            // reconstructed* typed camera/CRS domain (which are owned by the
            // typed fields above). A present-but-incomplete camera/CRS domain
            // that was skipped during typed reconstruction (camera/crs ==
            // None) is preserved verbatim here, so nothing is lost and the
            // file still reads (RFC-7002 §4.3/§4.4 tolerance). This
            // round-trips spice (65001), layers (65004), provenance (65005)
            // and any unknown/future / partial `ptiff.*` keys.
            node.for_each_field(|key, value| {
                let owned_by_typed = (descriptor.camera.is_some()
                    && key.starts_with("ptiff.camera."))
                    || (descriptor.crs.is_some() && key.starts_with("ptiff.crs."));
                if key.starts_with("ptiff.") && !owned_by_typed {
                    descriptor
                        .metadata
                        .insert(key.to_string(), value.to_string());
                }
            });
            scene.add_image(descriptor)?;
        }

        Ok(scene)
    }
}

impl Default for SceneDeserializer {
    fn default() -> Self {
        Self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{Camera, Geometry};
    use crate::io::Serializer;
    use crate::ErrorCode;

    #[test]
    fn deserializes_reference_field_schema() {
        let mut model = StorageModel::new();
        {
            let mut child = StorageModel::new();
            child.set_field("imageWidth", "64");
            child.set_field("imageHeight", "32");
            child.set_field("samplesPerPixel", "1");
            child.set_field("pixelType", "UInt8");
            child.set_field("compression", "None");
            model.add_child(child);
        }
        {
            let mut child = StorageModel::new();
            child.set_field("imageWidth", "640");
            child.set_field("imageHeight", "480");
            child.set_field("samplesPerPixel", "3");
            child.set_field("pixelType", "Float32");
            child.set_field("compression", "LZW");
            model.add_child(child);
        }

        let scene = SceneDeserializer.deserialize(&model).expect("deserialize");
        assert_eq!(scene.image_count(), 2);
        let first = scene.image_at(0).unwrap();
        assert_eq!(first.width(), 64);
        assert_eq!(first.height(), 32);
        assert_eq!(first.pixel_type(), PixelType::UInt8);
        assert_eq!(first.channel_count(), 1);
        assert_eq!(first.compression(), Some(CompressionKind::None));

        let second = scene.image_at(1).unwrap();
        assert_eq!(second.width(), 640);
        assert_eq!(second.pixel_type(), PixelType::Float32);
        assert_eq!(second.channel_count(), 3);
        assert_eq!(second.compression(), Some(CompressionKind::Lzw));
        // Round-trip limitation: tileInfo / gsd are not in the schema.
        assert_eq!(second.tile_info(), None);
        assert_eq!(second.ground_sample_distance_meters(), None);
    }

    #[test]
    fn missing_field_is_invalid_argument() {
        let mut model = StorageModel::new();
        let mut child = StorageModel::new();
        child.set_field("imageWidth", "64");
        // no imageHeight
        child.set_field("samplesPerPixel", "1");
        child.set_field("pixelType", "UInt8");
        model.add_child(child);

        let err = SceneDeserializer
            .deserialize(&model)
            .expect_err("must fail");
        assert_eq!(err.code(), ErrorCode::InvalidArgument);
        assert_eq!(err.message(), "SceneDeserializer: missing \"imageHeight\"");
    }

    #[test]
    fn non_numeric_field_is_invalid_argument() {
        let mut model = StorageModel::new();
        let mut child = StorageModel::new();
        child.set_field("imageWidth", "abc");
        child.set_field("imageHeight", "32");
        child.set_field("samplesPerPixel", "1");
        child.set_field("pixelType", "UInt8");
        model.add_child(child);

        let err = SceneDeserializer
            .deserialize(&model)
            .expect_err("must fail");
        assert_eq!(err.code(), ErrorCode::InvalidArgument);
        assert_eq!(
            err.message(),
            "SceneDeserializer: non-numeric \"imageWidth\""
        );
    }

    #[test]
    fn unrecognized_pixel_type_is_invalid_argument() {
        let mut model = StorageModel::new();
        let mut child = StorageModel::new();
        child.set_field("imageWidth", "64");
        child.set_field("imageHeight", "32");
        child.set_field("samplesPerPixel", "1");
        child.set_field("pixelType", "Int8");
        model.add_child(child);

        let err = SceneDeserializer
            .deserialize(&model)
            .expect_err("must fail");
        assert_eq!(err.code(), ErrorCode::InvalidArgument);
        assert_eq!(err.message(), "SceneDeserializer: unrecognized pixelType");
    }

    #[test]
    fn lzw_accepts_both_case_spellings() {
        for spelling in ["LZW", "Lzw"] {
            let mut model = StorageModel::new();
            let mut child = StorageModel::new();
            child.set_field("imageWidth", "16");
            child.set_field("imageHeight", "16");
            child.set_field("samplesPerPixel", "1");
            child.set_field("pixelType", "UInt8");
            child.set_field("compression", spelling);
            model.add_child(child);

            let scene = SceneDeserializer.deserialize(&model).unwrap();
            assert_eq!(
                scene.image_at(0).unwrap().compression(),
                Some(CompressionKind::Lzw)
            );
        }
    }

    #[test]
    fn round_trip_scene_serializer_deserializer() {
        // Every codec in the typed vocabulary round-trips through the
        // canonical schema (the C++ oracle's `parseCompression` accepted only
        // None/LZW; that asymmetry is closed by P0-03).
        let mut scene = Scene::new();
        scene
            .add_image(ImageDescriptor {
                width: 200,
                height: 100,
                pixel_type: PixelType::UInt8,
                channel_count: 3,
                compression: Some(CompressionKind::Lzw),
                ..ImageDescriptor::default()
            })
            .unwrap();

        let model = crate::io::SceneSerializer.serialize(&scene).unwrap();
        let back = SceneDeserializer.deserialize(&model).unwrap();

        assert_eq!(back.image_count(), 1);
        let first = back.image_at(0).unwrap();
        assert_eq!(first.width(), 200);
        assert_eq!(first.height(), 100);
        assert_eq!(first.pixel_type(), PixelType::UInt8);
        assert_eq!(first.channel_count(), 3);
        assert_eq!(first.compression(), Some(CompressionKind::Lzw));
    }

    #[test]
    fn every_typed_codec_round_trips_through_the_canonical_schema() {
        for codec in [
            CompressionKind::None,
            CompressionKind::Lzw,
            CompressionKind::PackBits,
            CompressionKind::Deflate,
            CompressionKind::Jpeg,
        ] {
            let mut scene = Scene::new();
            scene
                .add_image(ImageDescriptor {
                    width: 16,
                    height: 16,
                    pixel_type: PixelType::UInt8,
                    channel_count: 1,
                    compression: Some(codec),
                    ..ImageDescriptor::default()
                })
                .unwrap();

            let model = crate::io::SceneSerializer.serialize(&scene).unwrap();
            let back = SceneDeserializer.deserialize(&model).unwrap();
            assert_eq!(
                back.image_at(0).unwrap().compression(),
                Some(codec),
                "codec {codec:?} must round-trip through the canonical schema"
            );
        }
    }

    #[test]
    fn unknown_compression_is_invalid_argument() {
        let mut model = StorageModel::new();
        let mut child = StorageModel::new();
        child.set_field("imageWidth", "16");
        child.set_field("imageHeight", "16");
        child.set_field("samplesPerPixel", "1");
        child.set_field("pixelType", "UInt8");
        child.set_field("compression", "Zstd"); // never silently downgraded
        model.add_child(child);

        let err = SceneDeserializer
            .deserialize(&model)
            .expect_err("must fail");
        assert_eq!(err.code(), ErrorCode::InvalidArgument);
        assert_eq!(err.message(), "SceneDeserializer: unrecognized compression");
    }

    #[test]
    fn generic_metadata_domains_deserialize_into_image_metadata() {
        // spice / layers / provenance (and an unknown key) must surface through
        // ImageDescriptor.metadata, while the typed camera/CRS domains are
        // excluded.
        let mut model = StorageModel::new();
        let mut child = StorageModel::new();
        child.set_field("imageWidth", "16");
        child.set_field("imageHeight", "16");
        child.set_field("samplesPerPixel", "1");
        child.set_field("pixelType", "UInt8");
        child.set_field("ptiff.spice.frame", "IAU_MOON");
        child.set_field("ptiff.spice.time_system", "TDB");
        child.set_field("ptiff.layers.dem", "dem");
        child.set_field("ptiff.provenance.software", "libptiff");
        child.set_field("ptiff.provenance.future_field", "keep-me");
        // A complete typed camera domain (reconstructed into Image.camera) must
        // be excluded from the generic metadata map.
        child.set_field("ptiff.camera.model", "pinhole");
        child.set_field("ptiff.camera.focal_length_x", "100.0");
        child.set_field("ptiff.camera.focal_length_y", "100.0");
        child.set_field("ptiff.camera.principal_x", "8.0");
        child.set_field("ptiff.camera.principal_y", "8.0");
        model.add_child(child);

        let scene = SceneDeserializer.deserialize(&model).unwrap();
        let image = scene.image_at(0).unwrap();
        let metadata = image.metadata();
        assert_eq!(
            metadata.get("ptiff.spice.frame").map(String::as_str),
            Some("IAU_MOON")
        );
        assert_eq!(
            metadata.get("ptiff.spice.time_system").map(String::as_str),
            Some("TDB")
        );
        assert_eq!(
            metadata.get("ptiff.layers.dem").map(String::as_str),
            Some("dem")
        );
        assert_eq!(
            metadata
                .get("ptiff.provenance.software")
                .map(String::as_str),
            Some("libptiff")
        );
        assert_eq!(
            metadata
                .get("ptiff.provenance.future_field")
                .map(String::as_str),
            Some("keep-me")
        );
        // The typed camera key is reconstructed into the typed field and is
        // deliberately absent from the generic map.
        assert!(metadata.keys().all(|k| !k.starts_with("ptiff.camera.")));
        assert_eq!(image.camera().expect("camera").model_name(), "pinhole");
    }

    #[test]
    fn serializer_preserves_generic_metadata_bytes() {
        // Scene → StorageModel → Scene: spice/layers/provenance/unknown keys
        // survive a full serializer round-trip and re-emit the same
        // `ptiff.*` storage fields.
        let mut scene = Scene::new();
        scene
            .add_image(ImageDescriptor {
                width: 16,
                height: 16,
                pixel_type: PixelType::UInt8,
                channel_count: 1,
                compression: Some(CompressionKind::None),
                metadata: [
                    ("ptiff.spice.frame", "IAU_MOON"),
                    ("ptiff.provenance.software", "libptiff"),
                    ("ptiff.provenance.future_field", "keep-me"),
                ]
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
                ..ImageDescriptor::default()
            })
            .unwrap();

        let model = crate::io::SceneSerializer.serialize(&scene).unwrap();
        let child = &model.children()[0];

        // The emitted child node carries the generic `ptiff.*` fields verbatim,
        // ready for the TIFF backend's append_ptiff_tags to encode into tags
        // 65001/65005.
        assert_eq!(child.field("ptiff.spice.frame").unwrap(), "IAU_MOON");
        assert_eq!(
            child.field("ptiff.provenance.software").unwrap(),
            "libptiff"
        );
        assert_eq!(
            child.field("ptiff.provenance.future_field").unwrap(),
            "keep-me"
        );

        // And a full deserialize round-trips them back into Image.metadata.
        let back = SceneDeserializer.deserialize(&model).unwrap();
        let meta = back.image_at(0).unwrap().metadata();
        assert_eq!(
            meta.get("ptiff.spice.frame").map(String::as_str),
            Some("IAU_MOON")
        );
        assert_eq!(
            meta.get("ptiff.provenance.future_field")
                .map(String::as_str),
            Some("keep-me")
        );
    }

    #[test]
    fn partial_typed_domains_do_not_fail_deserialize_and_are_preserved() {
        // RFC-7002 §3.3 / §4.4: a file carrying a present-but-incomplete
        // camera/CRS domain (e.g. a foreign or future writer's partial field
        // set) must NOT fail the whole file. The typed reconstruction is
        // skipped (`camera`/`crs` == None) and the raw `ptiff.*` fields are
        // preserved verbatim in the generic metadata map.
        let mut model = StorageModel::new();
        let mut child = StorageModel::new();
        child.set_field("imageWidth", "16");
        child.set_field("imageHeight", "16");
        child.set_field("samplesPerPixel", "1");
        child.set_field("pixelType", "UInt8");
        child.set_field("ptiff.camera.model", "pinhole"); // partial: no intrinsics
        child.set_field("ptiff.crs.planet_name", "Moon"); // partial: incomplete extended schema
        model.add_child(child);

        let scene = SceneDeserializer
            .deserialize(&model)
            .expect("must not fail the file");
        let image = scene.image_at(0).unwrap();
        assert_eq!(
            image.camera(),
            None,
            "partial camera must degrade typed reconstruction"
        );
        assert_eq!(
            image.crs(),
            None,
            "partial crs must degrade typed reconstruction"
        );
        // The absent-typed-domain raw keys are preserved verbatim.
        assert_eq!(
            image
                .metadata()
                .get("ptiff.camera.model")
                .map(String::as_str),
            Some("pinhole")
        );
        assert_eq!(
            image
                .metadata()
                .get("ptiff.crs.planet_name")
                .map(String::as_str),
            Some("Moon")
        );
    }

    #[test]
    fn scene_level_camera_and_geometry_round_trip() {
        // Scene-level cameras/geometries serialize as their own child nodes
        // (marked `ptiff.scene.object_type`) and must round-trip through the
        // format-neutral Serializer/Deserializer, distinct from images.
        use crate::geometry::{Extrinsics, GeometryKind, Intrinsics, Quaternion, Vec3};

        let mut scene = Scene::new();
        scene
            .add_image(ImageDescriptor::new(16, 16))
            .expect("append image");
        let cam_id = scene.add_camera(Camera::from_model(
            "pinhole",
            Intrinsics::new(500.0, 501.0, 8.0, 8.0),
            Extrinsics::new(Quaternion::IDENTITY, Vec3::new(1.0, 2.0, 3.0)),
            "2026-08-21T12:00:00Z",
        ));
        let mut geom = Geometry::new(GeometryKind::Unspecified);
        geom.set_parameter("units", "meters");
        let geom_id = scene.add_geometry(geom);

        let model = crate::io::SceneSerializer.serialize(&scene).unwrap();
        // Three child nodes: one image + one camera + one geometry.
        assert_eq!(model.child_count(), 3);
        let camelike = model
            .children()
            .iter()
            .find(|c| c.field("ptiff.scene.object_type") == Ok("camera"))
            .expect("camera child present");
        assert!(camelike.field("ptiff.camera.model").is_ok());
        let geometry_child = model
            .children()
            .iter()
            .find(|c| c.field("ptiff.scene.object_type") == Ok("geometry"))
            .expect("geometry child present");
        assert_eq!(
            geometry_child.field("ptiff.scene.geometry.kind").unwrap(),
            "unspecified"
        );
        assert_eq!(
            geometry_child
                .field("ptiff.scene.geometry.param.units")
                .unwrap(),
            "meters"
        );

        let back = SceneDeserializer.deserialize(&model).unwrap();
        assert_eq!(back.image_count(), 1);
        assert_eq!(back.camera_count(), 1);
        assert_eq!(back.geometry_count(), 1);
        let cam = back.camera(cam_id).unwrap();
        assert_eq!(cam.model_name(), "pinhole");
        assert_eq!(cam.intrinsics().focal_length_pixels_x, 500.0);
        let geom = back.geometry(geom_id).unwrap();
        assert_eq!(geom.kind(), GeometryKind::Unspecified);
        assert_eq!(geom.parameter("units").unwrap(), "meters");
    }

    #[test]
    fn unknown_scene_object_type_is_invalid_argument() {
        let mut model = StorageModel::new();
        let mut child = StorageModel::new();
        child.set_field("ptiff.scene.object_type", "mesh3d");
        model.add_child(child);
        let err = SceneDeserializer
            .deserialize(&model)
            .expect_err("must fail");
        assert_eq!(err.code(), ErrorCode::InvalidArgument);
        assert_eq!(
            err.message(),
            "SceneDeserializer: unknown ptiff.scene.object_type \"mesh3d\""
        );
    }
}
