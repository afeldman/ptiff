//! Zarr-style chunked array [`StorageBackend`]. Registered with
//! [`crate::io::BackendFactory`] under the name `"zarr"`.
//!
//! Persists a format-neutral image as a compact self-describing single-file
//! container: an 8-byte seek header, a JSON array header
//! (shape/chunks/dtype/compressor via `serde_json`), then fixed-size chunk
//! slots holding zstd/zlib-compressed (or raw) chunks (chunk == tile for this
//! phase). Unlike isis/pds4 it does NOT reuse the memory-pixel-tier: chunks
//! are individually compressed, so it carries its own
//! [`ZarrImageSource`]/[`ZarrImageSink`]. Serializes a single image (root with
//! one child in the Scene convention); multi-image entry points are not
//! supported, mirroring the C++ `serializeModelList` /
//! `openImageSinkAt` / `openImageSourceAt` remaining `NotImplemented`.
//!
//! Mirrors `libptiff/include/ptiff/io/backend/zarr_backend.hpp` /
//! `libptiff/src/io/backend/zarr_backend.cpp`.

pub mod codec;
pub mod document;
mod image_sink;
mod image_source;

use crate::io::{
    BackendCapabilities, BinaryReader, BinaryWriter, ImageSink, ImageSource, StorageBackend,
};
use crate::{Error, Result, StorageModel};

use document::{build_layout, read_document, HEADER_SIZE};
use image_sink::ZarrImageSink;
use image_source::ZarrImageSource;

/// The Zarr-style storage backend.
pub struct ZarrBackend;

impl ZarrBackend {
    /// The Writer facade passes either a flat per-image model directly, or a
    /// root model whose first child carries the image fields. Normalize to the
    /// flat image model used by [`build_layout`].
    fn image_model(model: &StorageModel) -> &StorageModel {
        model.children().first().unwrap_or(model)
    }

    /// Writes the seek header + JSON array header for `layout` to `writer`,
    /// returning `(header_len, pixel_region)` where `pixel_region` is the
    /// absolute byte offset where the chunk block begins.
    fn write_header(writer: &mut dyn BinaryWriter, layout: &document::ZarrLayout) -> Result<u64> {
        if layout.header.len() as u64 > (1u64 << 24) {
            return Err(Error::invalid_argument("zarr: header too large to encode"));
        }
        let header_len = layout.header.len() as u64;
        let seek_header = header_len.to_le_bytes();
        let n = writer.write(&seek_header)?;
        if n != seek_header.len() {
            return Err(Error::invalid_argument("zarr: short seek header write"));
        }
        let n = writer.write(&layout.header)?;
        if n != layout.header.len() {
            return Err(Error::invalid_argument("zarr: short header write"));
        }
        Ok(HEADER_SIZE + header_len)
    }
}

impl StorageBackend for ZarrBackend {
    fn name(&self) -> &'static str {
        "zarr"
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            // Chunks are individually compressed and tile-addressable; the
            // document is streamable; random access is not advertised because
            // chunk slots are implicitly indexed by linear order (each read
            // needs the running slot geometry, not an explicit offset).
            supports_tiling: true,
            supports_streaming: true,
            supports_random_access: false,
            supports_cloud_streaming: false,
        }
    }

    fn open_image_source<'a>(
        &self,
        reader: &'a mut dyn BinaryReader,
    ) -> Result<Box<dyn ImageSource + 'a>> {
        let doc = read_document(reader)?;
        let pixel_region = HEADER_SIZE + doc.header.len() as u64;
        Ok(Box::new(ZarrImageSource::new(reader, doc, pixel_region)))
    }

    fn open_image_sink<'a>(
        &self,
        writer: &'a mut dyn BinaryWriter,
        model: &StorageModel,
    ) -> Result<Box<dyn ImageSink + 'a>> {
        let img = Self::image_model(model);
        let layout = build_layout(img)?;
        let pixel_region = Self::write_header(writer, &layout)?;
        writer.seek(pixel_region)?;
        Ok(Box::new(ZarrImageSink::new(writer, layout, pixel_region)))
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
        let layout = build_layout(img)?;
        Self::write_header(writer, &layout)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::{MemoryBinaryReader, MemoryBinaryWriter};
    use crate::tile::{TileIndex, TileLayout};

    /// Flat single-image model; `compression` selects the chunk compressor
    /// (None/zstd/zlib).
    fn image_model(compression: &str) -> StorageModel {
        let mut m = StorageModel::new();
        m.set_field("imageWidth", "32");
        m.set_field("imageHeight", "32");
        m.set_field("tileWidth", "16");
        m.set_field("tileHeight", "16");
        m.set_field("samplesPerPixel", "1");
        m.set_field("pixelType", "UInt8");
        m.set_field("compression", compression.to_owned());
        m
    }

    /// Writes a 2x2 grid of pattern tiles (tile linear index repeats across
    /// the whole tile so the pattern survives compression losslessly).
    fn write_pattern(sink: &mut dyn ImageSink) -> Result<()> {
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
                    let region = layout.region_for(index)?;
                    let tile = crate::tile::Tile::new(
                        crate::id::TileId::new(linear),
                        index,
                        region,
                        &payload,
                    );
                    sink.write_tile(&tile)?;
                }
            }
        }
        Ok(())
    }

    #[test]
    fn name_and_capabilities() {
        let backend = ZarrBackend;
        assert_eq!(backend.name(), "zarr");
        let caps = backend.capabilities();
        assert!(caps.supports_tiling);
        assert!(caps.supports_streaming);
        assert!(!caps.supports_random_access);
        assert!(!caps.supports_cloud_streaming);
    }

    #[test]
    fn factory_creates_registered_backend() {
        let backend = crate::io::BackendFactory::instance()
            .create("zarr")
            .expect("zarr backend must self-register");
        assert_eq!(backend.name(), "zarr");
    }

    #[test]
    fn serialize_then_deserialize_round_trips_model() {
        let backend = ZarrBackend;
        let model = image_model("zstd");

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
        assert_eq!(parsed.children()[0].field("compression").unwrap(), "zstd");
    }

    #[test]
    fn round_trips_uncompressed_chunks_byte_for_byte() {
        let backend = ZarrBackend;
        let model = image_model("None"); // 32x32, 16x16 tiles -> 2x2 grid, raw chunks

        let mut writer = MemoryBinaryWriter::new();
        backend
            .serialize_model(&model, &mut writer)
            .expect("serialize");
        {
            let mut sink = backend.open_image_sink(&mut writer, &model).expect("sink");
            write_pattern(&mut *sink).expect("write_pattern");
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
    fn round_trips_compressed_chunks_byte_for_byte() {
        for compression in ["zstd", "zlib"] {
            let backend = ZarrBackend;
            let model = image_model(compression);

            let mut writer = MemoryBinaryWriter::new();
            backend
                .serialize_model(&model, &mut writer)
                .expect("serialize");
            {
                let mut sink = backend.open_image_sink(&mut writer, &model).expect("sink");
                write_pattern(&mut *sink).expect("write_pattern");
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
    }
}
