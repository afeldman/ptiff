//! OpenEXR storage backend over the pure-Rust `exr` crate.
//!
//! Mirrors `ptiff::io::backend::OpenExrBackend` (see
//! `libptiff/src/io/backend/openexr_backend.cpp`). A real, standalone `.exr`
//! file written/read entirely through the pure-Rust `exr` crate (no C++). Only
//! `Float32` and `UInt32` pixel types are supported, with `samplesPerPixel` 1
//! ("Y"), 3 ("R","G","B") or 4 ("R","G","B","A"); the whole image is always
//! one tile.

pub mod document;
mod image_sink;
mod image_source;
mod io_adapter;

use crate::io::backend_capabilities::BackendCapabilities;
use crate::io::{BinaryReader, BinaryWriter, ImageSink, ImageSource, StorageBackend, StorageModel};
use crate::Result;

pub use document::OpenExrImageInfo;
pub use image_sink::OpenExrImageSink;
pub use image_source::OpenExrImageSource;

use document::{image_info_from_model, model_from_image_info, read_header_info, write_header_only};

/// The OpenEXR storage backend.
pub struct OpenExrBackend;

impl OpenExrBackend {
    /// The Writer facade passes either a flat per-image model directly, or
    /// (via `serialize_model` on a full scene root) a root model whose first
    /// child carries the image fields. Normalize both to the flat image model.
    fn image_model(model: &StorageModel) -> &StorageModel {
        model.children().first().unwrap_or(model)
    }
}

impl StorageBackend for OpenExrBackend {
    fn name(&self) -> &'static str {
        "openexr"
    }

    fn capabilities(&self) -> BackendCapabilities {
        // The whole image is always one tile this phase, so we do not advertise
        // real tile-by-tile serving; the document is otherwise seekable.
        BackendCapabilities {
            supports_tiling: false,
            supports_streaming: false,
            supports_random_access: true,
            supports_cloud_streaming: false,
        }
    }

    fn open_image_source<'a>(
        &self,
        reader: &'a mut dyn BinaryReader,
    ) -> Result<Box<dyn ImageSource + 'a>> {
        let info = read_header_info(reader)?;
        Ok(Box::new(OpenExrImageSource::new(reader, info)))
    }

    fn open_image_sink<'a>(
        &self,
        writer: &'a mut dyn BinaryWriter,
        model: &StorageModel,
    ) -> Result<Box<dyn ImageSink + 'a>> {
        let img = Self::image_model(model);
        let info = image_info_from_model(img)?;
        Ok(Box::new(OpenExrImageSink::new(writer, info)))
    }

    fn deserialize_model(&self, reader: &mut dyn BinaryReader) -> Result<StorageModel> {
        let info = read_header_info(reader)?;
        // `model_from_image_info` already returns the Scene-convention root with
        // one image child.
        Ok(model_from_image_info(&info))
    }

    fn serialize_model(&self, model: &StorageModel, writer: &mut dyn BinaryWriter) -> Result<()> {
        let img = Self::image_model(model);
        write_header_only(writer, img)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::TileId;
    use crate::io::{MemoryBinaryReader, MemoryBinaryWriter};
    use crate::tile::{Tile, TileIndex, TileLayout};

    fn image_model(width: u32, height: u32, spp: u32, pixel_type: &str) -> StorageModel {
        let mut img = StorageModel::new();
        img.set_field("imageWidth", width.to_string());
        img.set_field("imageHeight", height.to_string());
        img.set_field("samplesPerPixel", spp.to_string());
        img.set_field("pixelType", pixel_type.to_owned());
        img
    }

    #[test]
    fn name_and_capabilities() {
        let backend = OpenExrBackend;
        assert_eq!(backend.name(), "openexr");
        let caps = backend.capabilities();
        assert!(caps.supports_random_access);
        assert!(!caps.supports_tiling);
    }

    #[test]
    fn factory_creates_registered_backend() {
        let backend = crate::io::BackendFactory::instance()
            .create("openexr")
            .expect("openexr backend must self-register");
        assert_eq!(backend.name(), "openexr");
    }

    #[test]
    fn serialize_then_deserialize_round_trips_model() {
        let backend = OpenExrBackend;
        let model = image_model(4, 3, 1, "Float32");

        let mut writer = MemoryBinaryWriter::new();
        backend
            .serialize_model(&model, &mut writer)
            .expect("serialize");

        let bytes = writer.take_buffer();
        let mut reader = MemoryBinaryReader::from_vec(bytes);
        let parsed = backend.deserialize_model(&mut reader).expect("deserialize");
        // Scene convention: root model with one child (this image).
        assert_eq!(parsed.child_count(), 1);
        assert_eq!(parsed.children()[0].field("imageWidth").unwrap(), "4");
        assert_eq!(parsed.children()[0].field("imageHeight").unwrap(), "3");
        assert_eq!(parsed.children()[0].field("samplesPerPixel").unwrap(), "1");
        assert_eq!(parsed.children()[0].field("pixelType").unwrap(), "Float32");
    }

    #[test]
    fn round_trips_a_grayscale_float32_image_byte_for_byte() {
        let backend = OpenExrBackend;
        let model = image_model(4, 3, 1, "Float32");

        let pixels: Vec<f32> = (0..12).map(|i| i as f32 + 0.5).collect();
        let mut bytes = Vec::new();
        for p in &pixels {
            bytes.extend_from_slice(&p.to_ne_bytes());
        }

        let mut writer = MemoryBinaryWriter::new();
        backend
            .open_image_sink(&mut writer, &model)
            .expect("sink")
            .write_tile(&make_whole_tile(&model, &bytes))
            .expect("write_tile");

        let data = writer.take_buffer();
        let mut reader = MemoryBinaryReader::from_vec(data);
        let mut out = Vec::new();
        {
            let mut source = backend.open_image_source(&mut reader).expect("source");
            let layout: TileLayout = *source.layout();
            assert_eq!(layout.level_count, 1);
            assert_eq!(layout.tile_size.width, 4);
            assert_eq!(layout.tile_size.height, 3);
            let index = TileIndex::new(0, 0, 0);
            let tile = source.read_tile(index).expect("read_tile");
            out.extend_from_slice(tile.data());
        }

        assert_eq!(out, bytes);
    }

    #[test]
    fn round_trips_an_rgb_uint32_image_byte_for_byte() {
        let backend = OpenExrBackend;
        let model = image_model(2, 2, 3, "UInt32");

        let pixels: Vec<u32> = (0..12).map(|i| i * 7 + 1).collect();
        let mut bytes = Vec::new();
        for p in &pixels {
            bytes.extend_from_slice(&p.to_ne_bytes());
        }

        let mut writer = MemoryBinaryWriter::new();
        backend
            .open_image_sink(&mut writer, &model)
            .expect("sink")
            .write_tile(&make_whole_tile(&model, &bytes))
            .expect("write_tile");

        let data = writer.take_buffer();
        let mut reader = MemoryBinaryReader::from_vec(data);
        let mut out = Vec::new();
        {
            let mut source = backend.open_image_source(&mut reader).expect("source");
            let tile = source
                .read_tile(TileIndex::new(0, 0, 0))
                .expect("read_tile");
            out.extend_from_slice(tile.data());
        }

        assert_eq!(out, bytes);
    }

    #[test]
    fn rejects_unsupported_pixel_type() {
        let backend = OpenExrBackend;
        let model = image_model(2, 2, 1, "UInt8");
        let mut writer = MemoryBinaryWriter::new();
        let err = match backend.open_image_sink(&mut writer, &model) {
            Ok(_) => panic!("must fail"),
            Err(e) => e,
        };
        assert_eq!(err.code(), crate::ErrorCode::InvalidArgument);
    }

    #[test]
    fn rejects_unsupported_samples_per_pixel() {
        let backend = OpenExrBackend;
        let model = image_model(2, 2, 2, "Float32");
        let mut writer = MemoryBinaryWriter::new();
        let err = match backend.open_image_sink(&mut writer, &model) {
            Ok(_) => panic!("must fail"),
            Err(e) => e,
        };
        assert_eq!(err.code(), crate::ErrorCode::InvalidArgument);
    }

    /// Builds a whole-image (`index 0,0,0`) tile whose payload is `bytes`.
    fn make_whole_tile<'a>(model: &StorageModel, bytes: &'a [u8]) -> Tile<'a> {
        let info = image_info_from_model(model).expect("info");
        let layout = TileLayout::new(
            crate::tile::TileExtent::new(info.width, info.height),
            info.width,
            info.height,
            1,
        );
        let index = TileIndex::new(0, 0, 0);
        let region = layout.region_for(index).expect("region");
        Tile::new(TileId::new(0), index, region, bytes)
    }
}
