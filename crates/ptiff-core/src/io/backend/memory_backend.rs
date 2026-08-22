//! In-memory ("PMEM") [`StorageBackend`].
//!
//! Rust-first implementation (no byte-for-byte C++ PMEM binary format): the
//! format-neutral [`StorageModel`] is encoded to/from JSON via `serde_json`
//! (§4.4/§4.6 of PTIFF-1.0-RUST-CORE-PLAN.md — "Eigenes JSON-Parsing → serde").
//!
//! This module is only compiled when the `memory-backend` feature is enabled.

use crate::io::{BackendCapabilities, StorageBackend};
use crate::io::{BinaryReader, BinaryWriter, ImageSink, ImageSource};
use crate::{Error, Result, StorageModel};

/// The in-memory storage backend.
///
/// Persistent behavior is provided via the [`BinaryReader`]/[`BinaryWriter`]
/// transports (typically [`crate::io::MemoryBinaryReader`] /
/// [`crate::io::MemoryBinaryWriter`]) with the model encoded as JSON.
///
/// Capabilities: the format supports arbitrary seek (random access) and
/// incremental writes over a memory transport; it neither tiling-layouts nor
/// is it cloud-safe.
pub struct MemoryBackend;

impl StorageBackend for MemoryBackend {
    fn name(&self) -> &'static str {
        "memory"
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            supports_tiling: false,
            supports_streaming: true,
            supports_random_access: true,
            supports_cloud_streaming: false,
        }
    }

    fn open_image_source<'a>(
        &self,
        _reader: &'a mut dyn BinaryReader,
    ) -> Result<Box<dyn ImageSource + 'a>> {
        Err(Error::not_implemented(
            "MemoryBackend: openImageSource() not yet ported (tile source deferred)",
        ))
    }

    fn open_image_sink<'a>(
        &self,
        _writer: &'a mut dyn BinaryWriter,
        _model: &StorageModel,
    ) -> Result<Box<dyn ImageSink + 'a>> {
        Err(Error::not_implemented(
            "MemoryBackend: openImageSink() not yet ported (tile sink deferred)",
        ))
    }

    fn deserialize_model(&self, reader: &mut dyn BinaryReader) -> Result<StorageModel> {
        // Read the whole transport into memory (the memory backend is
        // whole-document by nature; pixel data is a later phase).
        let mut buf = Vec::new();
        // Loop until read returns 0 bytes (end of transport).
        let mut chunk = [0u8; 4096];
        loop {
            let n = reader.read(&mut chunk)?;
            buf.extend_from_slice(&chunk[..n]);
            if n == 0 {
                break;
            }
        }
        serde_json::from_slice(&buf)
            .map_err(|e| Error::invalid_argument(format!("MemoryBackend: bad model: {e}")))
    }

    fn serialize_model(&self, model: &StorageModel, writer: &mut dyn BinaryWriter) -> Result<()> {
        let bytes = serde_json::to_vec(model)
            .map_err(|e| Error::unknown(format!("MemoryBackend: encode failed: {e}")))?;
        // Write whole, or fail on short write.
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
}

impl Default for MemoryBackend {
    fn default() -> Self {
        Self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::MemoryBinaryReader;
    use crate::io::MemoryBinaryWriter;

    fn sample_model() -> StorageModel {
        let mut root = StorageModel::new();
        root.set_field("producer", "ptiff-core");
        let mut image = StorageModel::new();
        image.set_field("imageWidth", "64");
        image.set_field("imageHeight", "32");
        root.add_child(image);
        root
    }

    #[test]
    fn name_and_capabilities() {
        let backend = MemoryBackend;
        assert_eq!(backend.name(), "memory");
        let caps = backend.capabilities();
        assert!(caps.supports_streaming);
        assert!(caps.supports_random_access);
        assert!(!caps.supports_tiling);
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
        assert_eq!(err.code(), crate::ErrorCode::InvalidArgument);
    }

    #[test]
    fn pixel_source_and_sink_are_deferred_not_implemented() {
        let backend = MemoryBackend;
        let mut writer = MemoryBinaryWriter::new();
        let model = sample_model();

        // `.expect_err` requires `Debug` on the Box<dyn ...>, so match on the
        // Result and inspect the error code instead.
        let sink_err = match backend.open_image_sink(&mut writer, &model) {
            Ok(_) => panic!("expected NotImplemented"),
            Err(e) => e,
        };
        assert_eq!(sink_err.code(), crate::ErrorCode::NotImplemented);

        let mut reader = MemoryBinaryReader::from_vec(Vec::new());
        let source_err = match backend.open_image_source(&mut reader) {
            Ok(_) => panic!("expected NotImplemented"),
            Err(e) => e,
        };
        assert_eq!(source_err.code(), crate::ErrorCode::NotImplemented);
    }
}
