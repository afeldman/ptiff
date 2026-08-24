//! In-memory ("PMEM") [`StorageBackend`].
//!
//! Rust-first implementation (no byte-for-byte C++ PMEM binary format): the
//! format-neutral [`StorageModel`] is encoded to/from JSON via `serde_json`
//! (§4.4/§4.6 of PTIFF-1.0-RUST-CORE-PLAN.md — "Eigenes JSON-Parsing → serde").
//! A JSON-encoded model occupies the "document header" region before the
//! contiguous per-image pixel regions, which [`MemoryImageSource`] /
//! [`MemoryImageSink`] address at linear tile offsets.
//!
//! This module is only compiled when the `memory-backend` feature is enabled.

use crate::io::{
    BackendCapabilities, BinaryReader, BinaryWriter, ImageSink, ImageSource, StorageBackend,
};
use crate::{Error, Result, StorageModel};

use super::memory_image_sink::MemoryImageSink;
use super::memory_image_source::MemoryImageSource;
use super::memory_layout::{image_info_from_model, image_pixel_offset, MemoryImageInfo};

/// The in-memory storage backend.
///
/// Persistent behavior is provided via the [`BinaryReader`]/[`BinaryWriter`]
/// transports (typically [`crate::io::MemoryBinaryReader`] /
/// [`crate::io::MemoryBinaryWriter`]) with the model encoded as JSON.
///
/// Capabilities: the format supports tiled layouts and arbitrary seek (random
/// access); it is not cloud-safe — it streams over a whole in-memory document.
pub struct MemoryBackend;

impl MemoryBackend {
    /// Encodes `model` as the JSON bytes that precede its pixel region.
    fn model_json_bytes(model: &StorageModel) -> Result<Vec<u8>> {
        serde_json::to_vec(model)
            .map_err(|e| Error::unknown(format!("MemoryBackend: encode failed: {e}")))
    }

    /// Writes `bytes` in full to `writer`, failing on a short write.
    fn write_all(writer: &mut dyn BinaryWriter, bytes: &[u8]) -> Result<()> {
        let mut written = 0usize;
        while written < bytes.len() {
            let n = writer.write(&bytes[written..])?;
            if n == 0 {
                return Err(Error::unknown(
                    "MemoryBackend: transport refused to accept bytes",
                ));
            }
            written += n;
        }
        writer.flush()?;
        Ok(())
    }

    /// Reads the entire transport into a buffer until read returns 0 bytes.
    fn read_whole(reader: &mut dyn BinaryReader) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            let n = reader.read(&mut chunk)?;
            buf.extend_from_slice(&chunk[..n]);
            if n == 0 {
                break;
            }
        }
        Ok(buf)
    }

    /// Derives per-image info for a list of flat image models.
    fn image_infos(models: &[StorageModel]) -> Result<Vec<MemoryImageInfo>> {
        models.iter().map(image_info_from_model).collect()
    }
}

impl StorageBackend for MemoryBackend {
    fn name(&self) -> &'static str {
        "memory"
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            supports_tiling: true,
            supports_streaming: true,
            supports_random_access: true,
            supports_cloud_streaming: false,
        }
    }

    fn open_image_source<'a>(
        &self,
        reader: &'a mut dyn BinaryReader,
    ) -> Result<Box<dyn ImageSource + 'a>> {
        // A single-image document has a JSON-object header (from `serialize_model`)
        // whose children are the image models. Decode the leading object and
        // open image 0.
        let root = self.deserialize_model(reader)?;
        let children = root.children();
        if children.is_empty() {
            return Err(Error::invalid_argument(
                "memory: openImageSource requires a model with images",
            ));
        }
        let infos = Self::image_infos(children)?;
        let doc_prefix = Self::model_json_bytes(&root)?.len() as u64;
        let pixel_start = image_pixel_offset(&infos, 0, doc_prefix);
        Ok(Box::new(MemoryImageSource::new(
            reader,
            infos.into_iter().next().unwrap(),
            pixel_start,
        )))
    }

    fn open_image_sink<'a>(
        &self,
        writer: &'a mut dyn BinaryWriter,
        model: &StorageModel,
    ) -> Result<Box<dyn ImageSink + 'a>> {
        let children = model.children();
        if children.is_empty() {
            return Err(Error::invalid_argument(
                "memory: openImageSink requires a model with images",
            ));
        }
        // The pixel region begins right after this model's JSON header bytes.
        let doc_prefix = Self::model_json_bytes(model)?.len() as u64;
        let info = image_info_from_model(&children[0])?;
        writer.seek(doc_prefix)?;
        Ok(Box::new(MemoryImageSink::new(writer, info, doc_prefix)))
    }

    fn deserialize_model(&self, reader: &mut dyn BinaryReader) -> Result<StorageModel> {
        // A memory document is "JSON model header" followed by the contiguous
        // per-image pixel regions. Read the whole transport then decode only
        // the leading JSON object, ignoring any trailing pixel bytes.
        let buf = Self::read_whole(reader)?;
        let mut stream = serde_json::Deserializer::from_slice(&buf).into_iter::<StorageModel>();
        stream
            .next()
            .ok_or_else(|| Error::invalid_argument("MemoryBackend: bad model: empty document"))?
            .map_err(|e| Error::invalid_argument(format!("MemoryBackend: bad model: {e}")))
    }

    fn serialize_model(&self, model: &StorageModel, writer: &mut dyn BinaryWriter) -> Result<()> {
        let bytes = Self::model_json_bytes(model)?;
        Self::write_all(writer, &bytes)
    }

    fn serialize_model_list(
        &self,
        models: &[StorageModel],
        writer: &mut dyn BinaryWriter,
    ) -> Result<()> {
        let bytes = serde_json::to_vec(models)
            .map_err(|e| Error::unknown(format!("MemoryBackend: encode failed: {e}")))?;
        Self::write_all(writer, &bytes)
    }

    fn open_image_sink_at<'a>(
        &self,
        writer: &'a mut dyn BinaryWriter,
        models: &[StorageModel],
        image_index: usize,
    ) -> Result<Box<dyn ImageSink + 'a>> {
        if image_index >= models.len() {
            return Err(Error::out_of_range(
                "MemoryBackend::openImageSinkAt: image index out of range",
            ));
        }
        let infos = Self::image_infos(models)?;
        // The multi-image document's pixel regions begin after the JSON array
        // that encodes every model.
        let header_bytes = serde_json::to_vec(models)
            .map_err(|e| Error::unknown(format!("MemoryBackend: encode failed: {e}")))?;
        let doc_prefix = header_bytes.len() as u64;
        let pixel_start = image_pixel_offset(&infos, image_index, doc_prefix);
        writer.seek(pixel_start)?;
        Ok(Box::new(MemoryImageSink::new(
            writer,
            infos.into_iter().nth(image_index).unwrap(),
            pixel_start,
        )))
    }

    fn open_image_source_at<'a>(
        &self,
        reader: &'a mut dyn BinaryReader,
        image_index: usize,
    ) -> Result<Box<dyn ImageSource + 'a>> {
        // A multi-image document has a JSON-array header of models followed by
        // the contiguous per-image pixel regions. Decode only the leading array
        // value, ignoring any trailing pixel bytes.
        let buf = Self::read_whole(reader)?;
        let mut stream =
            serde_json::Deserializer::from_slice(&buf).into_iter::<Vec<StorageModel>>();
        let models = stream
            .next()
            .ok_or_else(|| Error::invalid_argument("MemoryBackend: bad model: empty document"))?
            .map_err(|e| Error::invalid_argument(format!("MemoryBackend: bad model: {e}")))?;
        if image_index >= models.len() {
            return Err(Error::out_of_range(
                "MemoryBackend::openImageSourceAt: image index out of range",
            ));
        }
        let infos = Self::image_infos(&models)?;
        let doc_prefix = serde_json::to_vec(&models)
            .map_err(|e| Error::unknown(format!("MemoryBackend: encode failed: {e}")))?
            .len() as u64;
        let pixel_start = image_pixel_offset(&infos, image_index, doc_prefix);
        Ok(Box::new(MemoryImageSource::new(
            reader,
            infos.into_iter().nth(image_index).unwrap(),
            pixel_start,
        )))
    }
}

impl Default for MemoryBackend {
    fn default() -> Self {
        Self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::TileId;
    use crate::io::MemoryBinaryReader;
    use crate::io::MemoryBinaryWriter;
    use crate::tile::{Tile, TileExtent, TileIndex, TileLayout, TileRegion};
    use crate::ErrorCode;

    fn sample_model() -> StorageModel {
        let mut root = StorageModel::new();
        root.set_field("producer", "ptiff-core");
        let mut image = StorageModel::new();
        image.set_field("imageWidth", "64");
        image.set_field("imageHeight", "32");
        image.set_field("tileWidth", "16");
        image.set_field("tileHeight", "16");
        image.set_field("samplesPerPixel", "1");
        image.set_field("pixelType", "UInt8");
        image.set_field("compression", "None");
        root.add_child(image);
        root
    }

    fn tiled_sink_payload() -> Vec<u8> {
        vec![0u8; 256]
    }

    #[test]
    fn name_and_capabilities() {
        let backend = MemoryBackend;
        assert_eq!(backend.name(), "memory");
        let caps = backend.capabilities();
        assert!(caps.supports_streaming);
        assert!(caps.supports_random_access);
        assert!(caps.supports_tiling);
        assert!(!caps.supports_cloud_streaming);
    }

    #[test]
    fn serialize_then_deserialize_round_trips_model() {
        let backend = MemoryBackend;
        let model = sample_model();

        let mut writer = MemoryBinaryWriter::new();
        backend
            .serialize_model(&model, &mut writer)
            .expect("serialize");
        assert!(!writer.buffer().is_empty());

        let bytes = writer.take_buffer();
        let mut reader = MemoryBinaryReader::from_vec(bytes);
        let back = backend.deserialize_model(&mut reader).expect("deserialize");

        // Fields round-trip (BTreeMap keeps canonical field order in JSON).
        assert_eq!(back.field("producer").unwrap(), "ptiff-core");
        assert_eq!(back.child_count(), 1);
        assert_eq!(back.children()[0].field("imageWidth").unwrap(), "64");
    }

    #[test]
    fn deserialize_garbage_is_invalid_argument() {
        let backend = MemoryBackend;
        let mut writer = MemoryBinaryWriter::new();
        writer.write(b"not-json").unwrap();
        let bytes = writer.take_buffer();
        let mut reader = MemoryBinaryReader::from_vec(bytes);
        let err = backend
            .deserialize_model(&mut reader)
            .expect_err("must fail");
        assert_eq!(err.code(), ErrorCode::InvalidArgument);
    }

    #[test]
    fn open_image_sink_requires_a_model_with_images() {
        let backend = MemoryBackend;
        let mut writer = MemoryBinaryWriter::new();
        let empty = StorageModel::new();
        let err = match backend.open_image_sink(&mut writer, &empty) {
            Ok(_) => panic!("expected error"),
            Err(e) => e,
        };
        assert_eq!(err.code(), ErrorCode::InvalidArgument);
        assert_eq!(
            err.message(),
            "memory: openImageSink requires a model with images"
        );
    }

    #[test]
    fn pixel_sink_writes_and_source_reads_back() {
        let backend = MemoryBackend;
        let model = sample_model();

        // 1) serialize the model header so the document header exists.
        let mut writer = MemoryBinaryWriter::new();
        backend.serialize_model(&model, &mut writer).unwrap();
        // Append the (zero-filled) pixel region right after the JSON header so
        // the sink can seek to an in-range tile offset. The cursor is already
        // past the header, so writing the pixel-region bytes appends them.
        let info = {
            use crate::io::backend::memory_layout::image_info_from_model;
            image_info_from_model(&model.children()[0]).unwrap()
        };
        {
            use crate::io::BinaryWriter as _;
            let zeros = vec![0u8; info.image_pixel_bytes as usize];
            writer.write(&zeros).unwrap();
        }

        // 2) open a sink over the same (already header-holding) writer.
        let mut sink = backend.open_image_sink(&mut writer, &model).unwrap();
        let mut payload = tiled_sink_payload();
        payload[0] = 0x42;
        payload[255] = 0x24;
        let tile = Tile::new(
            TileId::new(0),
            TileIndex::new(1, 0, 0),
            TileRegion::new(16, 0, TileExtent::new(16, 16)),
            &payload,
        );
        sink.write_tile(&tile).unwrap();
        assert_eq!(sink.layout().image_width, 64);
        drop(sink);

        // 3) read back via source. Tile (1,0) is the 2nd tile, at the JSON
        // header length + 256 bytes; the round-trip values prove the offset.
        let bytes = writer.take_buffer();
        let mut reader = MemoryBinaryReader::from_vec(bytes);
        let mut source = backend.open_image_source(&mut reader).unwrap();
        assert_eq!(
            source.layout(),
            &TileLayout::new(TileExtent::new(16, 16), 64, 32, 1)
        );
        let back = source.read_tile(TileIndex::new(1, 0, 0)).unwrap();
        assert_eq!(back.data()[0], 0x42);
        assert_eq!(back.data()[255], 0x24);
        assert_eq!(back.index(), TileIndex::new(1, 0, 0));
    }

    #[test]
    fn open_image_source_at_index_out_of_range() {
        let backend = MemoryBackend;
        let model = sample_model();
        // A multi-image document is written as a JSON array of models.
        let list = vec![model];
        let header = serde_json::to_vec(&list).unwrap();
        let bytes = header; // header alone is a valid array document (no pixels)
        let mut reader = MemoryBinaryReader::from_vec(bytes);
        let err = match backend.open_image_source_at(&mut reader, 99) {
            Ok(_) => panic!("expected error"),
            Err(e) => e,
        };
        assert_eq!(err.code(), ErrorCode::OutOfRange);
    }

    #[test]
    fn multi_image_sink_at_and_source_at_use_strided_offsets() {
        let backend = MemoryBackend;
        // Two images; the second's pixel region follows the first's.
        let mut a = StorageModel::new();
        a.set_field("imageWidth", "64");
        a.set_field("imageHeight", "32");
        a.set_field("tileWidth", "16");
        a.set_field("tileHeight", "16");
        a.set_field("samplesPerPixel", "1");
        a.set_field("pixelType", "UInt8");
        a.set_field("compression", "None");
        let mut b = StorageModel::new();
        b.set_field("imageWidth", "32");
        b.set_field("imageHeight", "16");
        b.set_field("tileWidth", "16");
        b.set_field("tileHeight", "16");
        b.set_field("samplesPerPixel", "1");
        b.set_field("pixelType", "UInt8");
        b.set_field("compression", "None");

        let models = vec![a, b];
        let infos = {
            use crate::io::backend::memory_layout::image_info_from_model;
            models
                .iter()
                .map(image_info_from_model)
                .collect::<Result<Vec<_>>>()
                .unwrap()
        };
        // Multi-image header is the JSON array of all models.
        let header = serde_json::to_vec(&models).unwrap();
        let doc_prefix = header.len() as u64;
        let pixel_start_a = doc_prefix;
        let pixel_start_b = doc_prefix + infos[0].image_pixel_bytes;

        // Pre-size the buffer: write the real JSON-array header, then
        // zero-fill the two pixel regions after it so the sink can seek in-range.
        let total = doc_prefix as usize
            + infos[0].image_pixel_bytes as usize
            + infos[1].image_pixel_bytes as usize;
        let mut writer = MemoryBinaryWriter::new();
        {
            use crate::io::BinaryWriter as _;
            writer.write(&header).unwrap();
            let zeros = vec![0u8; total - header.len()];
            writer.write(&zeros).unwrap();
        }

        // Write the *second* image's tile (0,0) via sink_at.
        let mut sink_b = backend.open_image_sink_at(&mut writer, &models, 1).unwrap();
        let mut payload_b = vec![0u8; 256];
        payload_b[0] = 0xEE;
        sink_b
            .write_tile(&Tile::new(
                TileId::new(0),
                TileIndex::new(0, 0, 0),
                TileRegion::new(0, 0, TileExtent::new(16, 16)),
                &payload_b,
            ))
            .unwrap();
        drop(sink_b);

        let bytes = writer.take_buffer();
        // Read back image index 1 from the source_at path.
        let mut reader = MemoryBinaryReader::from_vec(bytes.clone());
        let mut source_b = backend.open_image_source_at(&mut reader, 1).unwrap();
        let back_b = source_b.read_tile(TileIndex::new(0, 0, 0)).unwrap();
        assert_eq!(back_b.data()[0], 0xEE);
        // Verify indexing into the raw buffer: image 1's first tile sits at
        // pixel_start_b.
        assert_eq!(
            bytes[pixel_start_b as usize], 0xEE,
            "image 1 tile must land at its strided pixel offset"
        );
        // Verify image 0's region is still all zeros (untouched).
        assert_eq!(bytes[pixel_start_a as usize], 0);

        // open_image_sink_at out-of-range guard.
        let mut w2 = MemoryBinaryWriter::new();
        let err = match backend.open_image_sink_at(&mut w2, &models, 99) {
            Ok(_) => panic!("expected error"),
            Err(e) => e,
        };
        assert_eq!(err.code(), ErrorCode::OutOfRange);
    }
}
