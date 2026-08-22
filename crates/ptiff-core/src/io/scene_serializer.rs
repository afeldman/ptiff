//! Concrete [`Serializer`]: `Scene` → `StorageModel` in the canonical PTIFF
//! field schema.
//!
//! Mirrors `ptiff::io::SceneSerializer` (see
//! `libptiff/src/io/scene_serializer.cpp`). Field names, ordering, validation
//! and error messages match the C++ oracle **exactly** so golden/round-trip
//! tests between the two cores stay green.

use crate::image::CompressionKind;
use crate::io::Serializer;
use crate::pixel_type::PixelType;
use crate::{Error, Result, Scene, StorageModel};

/// The field key under which the number of channels is serialized.
pub const FIELD_SAMPLES_PER_PIXEL: &str = "samplesPerPixel";
/// The field key under which the pixel sample type is serialized.
pub const FIELD_PIXEL_TYPE: &str = "pixelType";
/// The field key under which the compression scheme is serialized.
pub const FIELD_COMPRESSION: &str = "compression";
/// The field key under which the tile width is serialized.
pub const FIELD_TILE_WIDTH: &str = "tileWidth";
/// The field key under which the tile height is serialized.
pub const FIELD_TILE_HEIGHT: &str = "tileHeight";

/// Maps a [`PixelType`] to its canonical storage string, as the C++ oracle does.
///
/// Returns `None` for [`PixelType::Float64`], which the serializer does not
/// support.
fn pixel_type_storage_value(t: PixelType) -> Option<&'static str> {
    match t {
        PixelType::UInt8 => Some("UInt8"),
        PixelType::UInt16 => Some("UInt16"),
        PixelType::UInt32 => Some("UInt32"),
        PixelType::Float32 => Some("Float32"),
        PixelType::Float64 => None,
    }
}

/// The reference `Scene` → `StorageModel` serializer.
///
/// Stateless and therefore thread-compatible.
pub struct SceneSerializer;

impl Serializer for SceneSerializer {
    fn serialize(&self, scene: &Scene) -> Result<StorageModel> {
        let mut root = StorageModel::new();

        for i in 0..scene.image_count() {
            // `scene.image_at` is infallible within `[0, image_count())`.
            let image = scene.image_at(i).expect("image_count() bounds image_at");
            let mut child = StorageModel::new();

            child.set_field("imageWidth", image.width().to_string());
            child.set_field("imageHeight", image.height().to_string());

            // Channels must be 1 or 3 (matches the C++ oracle).
            let channels = image.channel_count();
            if channels != 1 && channels != 3 {
                return Err(Error::invalid_argument(
                    "SceneSerializer: unsupported samplesPerPixel",
                ));
            }
            child.set_field(FIELD_SAMPLES_PER_PIXEL, channels.to_string());

            // Float64 cannot be represented in the canonical schema.
            let pixel_type_value =
                pixel_type_storage_value(image.pixel_type()).ok_or_else(|| {
                    Error::invalid_argument("SceneSerializer: unsupported pixelType Float64")
                })?;
            child.set_field(FIELD_PIXEL_TYPE, pixel_type_value);

            match image.compression() {
                None => child.set_field(FIELD_COMPRESSION, "None"),
                Some(c) => {
                    // Jpeg compression requires UInt8 samples (matches C++).
                    if c == CompressionKind::Jpeg && image.pixel_type() != PixelType::UInt8 {
                        return Err(Error::invalid_argument(
                            "SceneSerializer: Jpeg compression requires UInt8 pixelType",
                        ));
                    }
                    let value = match c {
                        CompressionKind::None => "None",
                        CompressionKind::Lzw => "LZW",
                        CompressionKind::Deflate => "Deflate",
                        CompressionKind::Jpeg => "Jpeg",
                    };
                    child.set_field(FIELD_COMPRESSION, value);
                }
            }

            if let Some(tile_info) = image.tile_info() {
                child.set_field(FIELD_TILE_WIDTH, tile_info.tile_width.to_string());
                child.set_field(FIELD_TILE_HEIGHT, tile_info.tile_height.to_string());
            }

            // PTIFF extension domains (camera + CRS) round-trip through the
            // `ptiff.<domain>.<key>` fields on this image's child node. The
            // TIFF backend maps these to/from the private tags 65002 / 65003.
            if let Some(camera) = image.camera() {
                for (k, v) in crate::geometry::camera_fields(camera) {
                    child.set_field(k, v);
                }
            }
            if let Some(crs) = image.crs() {
                for (k, v) in crate::geometry::crs_fields(crs) {
                    child.set_field(k, v);
                }
            }

            root.add_child(child);
        }

        Ok(root)
    }
}

impl Default for SceneSerializer {
    fn default() -> Self {
        Self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::{ImageDescriptor, TileInfo};

    fn scene_with_two_images() -> Scene {
        let mut scene = Scene::new();
        scene
            .add_image(ImageDescriptor::new(64, 32))
            .expect("append");
        scene
            .add_image(ImageDescriptor {
                width: 640,
                height: 480,
                pixel_type: PixelType::UInt16,
                channel_count: 3,
                compression: Some(CompressionKind::Deflate),
                tile_info: Some(TileInfo::new(256, 256)),
                ..ImageDescriptor::default()
            })
            .expect("append");
        scene
    }

    #[test]
    fn serializes_reference_field_schema() {
        let scene = scene_with_two_images();
        let model = SceneSerializer.serialize(&scene).expect("serialize");

        assert_eq!(model.child_count(), 2);
        let first = &model.children()[0];
        assert_eq!(first.field("imageWidth").unwrap(), "64");
        assert_eq!(first.field("imageHeight").unwrap(), "32");
        assert_eq!(first.field("samplesPerPixel").unwrap(), "1");
        assert_eq!(first.field("pixelType").unwrap(), "UInt8");
        assert_eq!(first.field("compression").unwrap(), "None");

        let second = &model.children()[1];
        assert_eq!(second.field("imageWidth").unwrap(), "640");
        assert_eq!(second.field("pixelType").unwrap(), "UInt16");
        assert_eq!(second.field("samplesPerPixel").unwrap(), "3");
        assert_eq!(second.field("compression").unwrap(), "Deflate");
        // Only tiled images carry tileWidth/tileHeight.
        assert_eq!(second.field("tileWidth").unwrap(), "256");
        assert_eq!(second.field("tileHeight").unwrap(), "256");
        assert!(first.field("tileWidth").is_err());
    }

    #[test]
    fn empty_scene_serializes_to_empty_root() {
        let scene = Scene::new();
        let model = SceneSerializer.serialize(&scene).expect("serialize");
        assert_eq!(model.child_count(), 0);
    }

    #[test]
    fn unsupported_channel_count_is_invalid_argument() {
        let mut scene = Scene::new();
        scene
            .add_image(ImageDescriptor {
                width: 10,
                height: 10,
                channel_count: 4,
                ..ImageDescriptor::default()
            })
            .expect("append");
        let err = SceneSerializer.serialize(&scene).expect_err("must fail");
        assert_eq!(err.code(), crate::ErrorCode::InvalidArgument);
        assert_eq!(
            err.message(),
            "SceneSerializer: unsupported samplesPerPixel"
        );
    }

    #[test]
    fn float64_pixel_type_is_invalid_argument() {
        let mut scene = Scene::new();
        scene
            .add_image(ImageDescriptor {
                width: 10,
                height: 10,
                pixel_type: PixelType::Float64,
                ..ImageDescriptor::default()
            })
            .expect("append");
        let err = SceneSerializer.serialize(&scene).expect_err("must fail");
        assert_eq!(err.code(), crate::ErrorCode::InvalidArgument);
        assert_eq!(
            err.message(),
            "SceneSerializer: unsupported pixelType Float64"
        );
    }

    #[test]
    fn jpeg_requires_uint8() {
        let mut scene = Scene::new();
        scene
            .add_image(ImageDescriptor {
                width: 10,
                height: 10,
                pixel_type: PixelType::UInt16,
                compression: Some(CompressionKind::Jpeg),
                ..ImageDescriptor::default()
            })
            .expect("append");
        let err = SceneSerializer.serialize(&scene).expect_err("must fail");
        assert_eq!(err.code(), crate::ErrorCode::InvalidArgument);
        assert_eq!(
            err.message(),
            "SceneSerializer: Jpeg compression requires UInt8 pixelType"
        );
    }
}
