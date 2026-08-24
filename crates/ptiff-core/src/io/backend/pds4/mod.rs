//! NASA PDS4 (Planetary Data System 4) style [`StorageBackend`]. Registered
//! with [`crate::io::BackendFactory`] under the name `"pds4"`.
//!
//! Persists a format-neutral image as a small self-describing PDS4-looking
//! subset: a `Product_Observational` XML label (written/read via
//! `quick-xml`) followed by a seekable, uncompressed pixel block, reusing
//! the memory-pixel-tier for pixel I/O. Serializes a single image (root with
//! one child, in the Scene convention); multi-image entry points are not
//! supported (one image per document), mirroring the C++
//! `serializeModelList` / `openImageSinkAt` / `openImageSourceAt` remaining
//! `NotImplemented`.
//!
//! Mirrors `libptiff/include/ptiff/io/backend/pds4_backend.hpp` /
//! `libptiff/src/io/backend/pds4_backend.cpp`.

mod document;
mod label;

use crate::io::{
    BackendCapabilities, BinaryReader, BinaryWriter, ImageSink, ImageSource, StorageBackend,
};
use crate::{Error, Result, StorageModel};

use super::{image_info_from_model, MemoryImageSink, MemoryImageSource};
use document::{pixel_origin, read_document};
use label::{write_label, HEADER_SIZE};

/// The PDS4-style storage backend.
pub struct Pds4Backend;

impl Pds4Backend {
    /// The Writer facade passes either a flat per-image model directly, or a
    /// root model whose first child carries the image fields. Normalize to
    /// the flat image model used by [`write_label`] / [`image_info_from_model`].
    fn image_model(model: &StorageModel) -> &StorageModel {
        model.children().first().unwrap_or(model)
    }

    /// Writes `bytes` in full to `writer`, failing on a short write.
    fn write_all(writer: &mut dyn BinaryWriter, bytes: &[u8]) -> Result<()> {
        let mut written = 0usize;
        while written < bytes.len() {
            let n = writer.write(&bytes[written..])?;
            if n == 0 {
                return Err(Error::invalid_argument("pds4: short label write"));
            }
            written += n;
        }
        Ok(())
    }
}

impl StorageBackend for Pds4Backend {
    fn name(&self) -> &'static str {
        "pds4"
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            // PDS4 labels are seekable (label first, pixel block after) and the pixel block is
            // tiled/uncompressed like the memory backend.
            supports_tiling: true,
            supports_streaming: false,
            supports_random_access: true,
            supports_cloud_streaming: false,
        }
    }

    fn open_image_source<'a>(
        &self,
        reader: &'a mut dyn BinaryReader,
    ) -> Result<Box<dyn ImageSource + 'a>> {
        let doc = read_document(reader)?;
        let info = image_info_from_model(&doc.image)?;
        let pixel_start = pixel_origin(doc.label_bytes);
        Ok(Box::new(MemoryImageSource::new(reader, info, pixel_start)))
    }

    fn open_image_sink<'a>(
        &self,
        writer: &'a mut dyn BinaryWriter,
        model: &StorageModel,
    ) -> Result<Box<dyn ImageSink + 'a>> {
        let img = Self::image_model(model);
        let info = image_info_from_model(img)?;
        let label = write_label(img)?;
        let pixel_start = pixel_origin(label.len() as u64);
        writer.seek(pixel_start)?;
        Ok(Box::new(MemoryImageSink::new(writer, info, pixel_start)))
    }

    fn deserialize_model(&self, reader: &mut dyn BinaryReader) -> Result<StorageModel> {
        let doc = read_document(reader)?;
        // Scene convention: root model with one child (this image).
        let mut root = StorageModel::new();
        root.add_child(doc.image);
        Ok(root)
    }

    fn serialize_model(&self, model: &StorageModel, writer: &mut dyn BinaryWriter) -> Result<()> {
        let img = Self::image_model(model);
        let label = write_label(img)?;
        if label.len() as u64 > (1u64 << 24) {
            return Err(Error::invalid_argument("pds4: label too large to encode"));
        }

        // Seek header (label byte length, LE).
        let header = (label.len() as u64).to_le_bytes();
        debug_assert_eq!(header.len() as u64, HEADER_SIZE);
        Self::write_all(writer, &header)?;
        Self::write_all(writer, &label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::TileId;
    use crate::io::{MemoryBinaryReader, MemoryBinaryWriter};
    use crate::tile::{Tile, TileIndex, TileLayout};

    fn image_model() -> StorageModel {
        let mut m = StorageModel::new();
        m.set_field("imageWidth", "32");
        m.set_field("imageHeight", "32");
        m.set_field("tileWidth", "16");
        m.set_field("tileHeight", "16");
        m.set_field("samplesPerPixel", "1");
        m.set_field("pixelType", "UInt8");
        m.set_field("compression", "None");
        m
    }

    #[test]
    fn name_and_capabilities() {
        let backend = Pds4Backend;
        assert_eq!(backend.name(), "pds4");
        let caps = backend.capabilities();
        assert!(caps.supports_tiling);
        assert!(!caps.supports_streaming);
        assert!(caps.supports_random_access);
        assert!(!caps.supports_cloud_streaming);
    }

    #[test]
    fn factory_creates_registered_backend() {
        let backend = crate::io::BackendFactory::instance()
            .create("pds4")
            .expect("pds4 backend must self-register");
        assert_eq!(backend.name(), "pds4");
    }

    #[test]
    fn serialize_then_deserialize_round_trips_model() {
        let backend = Pds4Backend;
        let model = image_model();

        let mut writer = MemoryBinaryWriter::new();
        backend
            .serialize_model(&model, &mut writer)
            .expect("serialize");

        let bytes = writer.take_buffer();
        let mut reader = MemoryBinaryReader::from_vec(bytes);
        let parsed = backend.deserialize_model(&mut reader).expect("deserialize");
        // Scene convention: root model with one child (this image).
        assert_eq!(parsed.child_count(), 1);
        assert_eq!(parsed.children()[0].field("imageWidth").unwrap(), "32");
        assert_eq!(parsed.children()[0].field("tileWidth").unwrap(), "16");
        assert_eq!(parsed.children()[0].field("pixelType").unwrap(), "UInt8");
    }

    #[test]
    fn round_trips_a_2x2_grid_of_tiles_byte_for_byte() {
        let backend = Pds4Backend;
        let model = image_model(); // 32x32 image, 16x16 tiles -> 2x2 grid

        let mut writer = MemoryBinaryWriter::new();
        backend
            .serialize_model(&model, &mut writer)
            .expect("serialize");
        {
            let mut sink = backend.open_image_sink(&mut writer, &model).expect("sink");
            let layout: TileLayout = *sink.layout();
            let tile_bytes = (layout.tile_size.width * layout.tile_size.height) as usize;
            for level in 0..layout.level_count {
                for row in 0..layout.rows(level) {
                    for column in 0..layout.columns(level) {
                        let index = TileIndex { column, row, level };
                        let linear =
                            u64::from(row) * u64::from(layout.columns(level)) + u64::from(column);
                        let value = (linear % 256) as u8;
                        let payload = vec![value; tile_bytes];
                        let region = layout.region_for(index).expect("region");
                        let tile = Tile::new(TileId::new(linear), index, region, &payload);
                        sink.write_tile(&tile).expect("write_tile");
                    }
                }
            }
        }

        let bytes = writer.take_buffer();
        let mut reader = MemoryBinaryReader::from_vec(bytes);
        let mut out = Vec::new();
        {
            let mut source = backend.open_image_source(&mut reader).expect("source");
            let layout: TileLayout = *source.layout();
            for level in 0..layout.level_count {
                for row in 0..layout.rows(level) {
                    for column in 0..layout.columns(level) {
                        let index = TileIndex { column, row, level };
                        let tile = source.read_tile(index).expect("read_tile");
                        out.extend_from_slice(tile.data());
                    }
                }
            }
        }

        assert_eq!(out.len(), 32 * 32);
        assert_eq!(out[0], 0);
        assert_eq!(out[16 * 16], 1);
        assert_eq!(out[2 * 16 * 16], 2);
    }

    #[test]
    fn serialize_model_rejects_oversized_label() {
        // Mirrors the C++ serializeModel guard: label byte length must fit the
        // (1 << 24) plausibility bound shared with read_document.
        let backend = Pds4Backend;
        let mut model = image_model();
        model.set_field("oversized", "x".repeat((1usize << 24) + 1));

        let mut writer = MemoryBinaryWriter::new();
        let err = backend
            .serialize_model(&model, &mut writer)
            .expect_err("must fail");
        assert_eq!(err.code(), crate::ErrorCode::InvalidArgument);
    }
}
