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
        _ => Err(Error::invalid_argument(
            "SceneDeserializer: unrecognized pixelType",
        )),
    }
}

/// Parses the canonical compression string (mirrors C++ `parseCompression`).
fn parse_compression(value: &str) -> Result<CompressionKind> {
    match value {
        "None" => Ok(CompressionKind::None),
        "LZW" | "Lzw" => Ok(CompressionKind::Lzw),
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
        // Jpeg/Deflate are write-only in the canonical schema: the C++ oracle's
        // `parseCompression` accepts only `None`/`LZW`, so we round-trip Lzw.
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
    fn deflate_writes_but_does_not_deserialize_canonical_schema() {
        // Documents the oracle asymmetry: Deflate serializes, but the canonical
        // read path cannot reconstruct it.
        let mut scene = Scene::new();
        scene
            .add_image(ImageDescriptor {
                width: 16,
                height: 16,
                pixel_type: PixelType::UInt8,
                channel_count: 1,
                compression: Some(CompressionKind::Deflate),
                ..ImageDescriptor::default()
            })
            .unwrap();

        let model = crate::io::SceneSerializer.serialize(&scene).unwrap();
        let err = SceneDeserializer
            .deserialize(&model)
            .expect_err("must fail");
        assert_eq!(err.code(), ErrorCode::InvalidArgument);
        assert_eq!(err.message(), "SceneDeserializer: unrecognized compression");
    }
}
