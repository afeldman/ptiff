//! Backend abstraction: byte transport ↔ [`StorageModel`] ↔ tile access.
//!
//! Mirrors `ptiff::io::StorageBackend` (see
//! `libptiff/include/ptiff/io/storage_backend.hpp`).

use crate::io::backend_capabilities::BackendCapabilities;
use crate::io::{BinaryReader, BinaryWriter, ImageSink, ImageSource};
use crate::{Error, Result, StorageModel};

/// A pluggable storage backend that understands one on-disk format.
///
/// A `StorageBackend` couples a [`BinaryReader`]/[`BinaryWriter`] byte transport
/// with a [`StorageModel`] mapping and [`ImageSource`]/[`ImageSink`] tile
/// access. Because it only speaks in terms of the format-neutral
/// [`StorageModel`] and tile-level [`Tile`]s, a caller can read or write any
/// format without knowing its byte layout.
///
/// **Thread-safety:** not thread-safe by default; each file should get its own
/// backend instance. Stateless backends may relax this, documented per
/// implementation.
///
/// [`Tile`]: crate::tile::Tile
pub trait StorageBackend {
    /// Returns the canonical name of this backend (`"tiff"`, `"memory"`, ...).
    ///
    /// Used as its registry key in [`crate::io::BackendFactory`].
    ///
    /// [`crate::io::BackendFactory`]: crate::io::BackendFactory
    fn name(&self) -> &'static str;

    /// Reports which capabilities this backend supports.
    fn capabilities(&self) -> BackendCapabilities;

    /// Opens an [`ImageSource`] over `reader`.
    ///
    /// The returned source borrows `reader`, so its lifetime is bound to this
    /// borrow.
    ///
    /// # Errors
    ///
    /// Returns a backend-specific error if the image cannot be opened / parsed
    /// from `reader`.
    fn open_image_source<'a>(
        &self,
        reader: &'a mut dyn BinaryReader,
    ) -> Result<Box<dyn ImageSource + 'a>>;

    /// Opens an [`ImageSink`] over `writer`.
    ///
    /// The returned sink borrows `writer`, so its lifetime is bound to this
    /// borrow.
    ///
    /// # Errors
    ///
    /// Returns a backend-specific error if the sink cannot be prepared from
    /// `writer` / `model`.
    fn open_image_sink<'a>(
        &self,
        writer: &'a mut dyn BinaryWriter,
        model: &StorageModel,
    ) -> Result<Box<dyn ImageSink + 'a>>;

    /// Reads a document's format-neutral model from `reader`.
    ///
    /// # Errors
    ///
    /// Returns a backend-specific error if the bytes cannot be interpreted.
    fn deserialize_model(&self, reader: &mut dyn BinaryReader) -> Result<StorageModel>;

    /// Writes `model` to `writer` in this backend's format.
    ///
    /// # Errors
    ///
    /// Returns a backend-specific error if the model cannot be encoded/written.
    fn serialize_model(&self, model: &StorageModel, writer: &mut dyn BinaryWriter) -> Result<()>;

    /// Writes several images (one per model) to `writer` as a single
    /// multi-image file.
    ///
    /// Multi-image aware backends (e.g. TIFF's IFD chain) write `models` in
    /// order as one file; backends that only understand a single image return
    /// [`ErrorCode::NotImplemented`]. [`serialize_model`] on a single image is
    /// equivalent to this with `models.len() == 1`, so callers may use either
    /// path.
    ///
    /// The default implementation returns [`ErrorCode::NotImplemented`] — the
    /// C++ oracle's default behavior.
    ///
    /// # Errors
    ///
    /// The default returns [`ErrorCode::NotImplemented`].
    ///
    /// [`serialize_model`]: StorageBackend::serialize_model
    /// [`ErrorCode::NotImplemented`]: crate::ErrorCode::NotImplemented
    fn serialize_model_list(
        &self,
        _models: &[StorageModel],
        _writer: &mut dyn BinaryWriter,
    ) -> Result<()> {
        Err(Error::not_implemented(
            "StorageBackend: serializeModelList() not supported",
        ))
    }

    /// Opens an [`ImageSink`] for one image of a multi-image file.
    ///
    /// Multi-image aware backends return a sink positioned for the
    /// `image_index`-th image's data region; single-image backends return
    /// [`ErrorCode::NotImplemented`].
    ///
    /// The default implementation returns [`ErrorCode::NotImplemented`] — the
    /// C++ oracle's default behavior.
    ///
    /// # Errors
    ///
    /// The default returns [`ErrorCode::NotImplemented`].
    fn open_image_sink_at<'a>(
        &self,
        _writer: &'a mut dyn BinaryWriter,
        _models: &[StorageModel],
        _image_index: usize,
    ) -> Result<Box<dyn ImageSink + 'a>> {
        Err(Error::not_implemented(
            "StorageBackend: openImageSinkAt() not supported",
        ))
    }

    /// Opens an [`ImageSource`] for one image of a multi-image file.
    ///
    /// Multi-image aware backends return a source for the `image_index`-th
    /// directory of the IFD chain; single-image backends return
    /// [`ErrorCode::NotImplemented`].
    ///
    /// The default implementation returns [`ErrorCode::NotImplemented`] — the
    /// C++ oracle's default behavior.
    ///
    /// # Errors
    ///
    /// The default returns [`ErrorCode::NotImplemented`].
    fn open_image_source_at<'a>(
        &self,
        _reader: &'a mut dyn BinaryReader,
        _image_index: usize,
    ) -> Result<Box<dyn ImageSource + 'a>> {
        Err(Error::not_implemented(
            "StorageBackend: openImageSourceAt() not supported",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ErrorCode;

    /// A minimal in-memory writer used for trait probes.
    struct VecWriter {
        data: Vec<u8>,
    }

    impl BinaryWriter for VecWriter {
        fn write(&mut self, source: &[u8]) -> Result<usize> {
            self.data.extend_from_slice(source);
            Ok(source.len())
        }
        fn seek(&mut self, offset: u64) -> Result<()> {
            if offset as usize > self.data.len() {
                return Err(Error::out_of_range("seek beyond output"));
            }
            self.data.resize(offset as usize, 0);
            Ok(())
        }
        fn position(&self) -> Result<u64> {
            Ok(self.data.len() as u64)
        }
        fn flush(&mut self) -> Result<()> {
            Ok(())
        }
    }

    /// A trivial stateless backend to probe the trait's default methods.
    struct ProbeBackend;

    impl StorageBackend for ProbeBackend {
        fn name(&self) -> &'static str {
            "probe"
        }

        fn capabilities(&self) -> BackendCapabilities {
            BackendCapabilities::default()
        }

        fn open_image_source<'a>(
            &self,
            _reader: &'a mut dyn BinaryReader,
        ) -> Result<Box<dyn ImageSource + 'a>> {
            Err(Error::not_implemented("ProbeBackend: no sources"))
        }

        fn open_image_sink<'a>(
            &self,
            _writer: &'a mut dyn BinaryWriter,
            _model: &StorageModel,
        ) -> Result<Box<dyn ImageSink + 'a>> {
            Err(Error::not_implemented("ProbeBackend: no sinks"))
        }

        fn deserialize_model(&self, _reader: &mut dyn BinaryReader) -> Result<StorageModel> {
            Ok(StorageModel::new())
        }

        fn serialize_model(
            &self,
            _model: &StorageModel,
            _writer: &mut dyn BinaryWriter,
        ) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn default_multi_image_methods_return_not_implemented() {
        let backend = ProbeBackend;
        let mut writer = VecWriter { data: Vec::new() };
        let err = backend
            .serialize_model_list(&[], &mut writer)
            .expect_err("must be NotImplemented");
        assert_eq!(err.code(), ErrorCode::NotImplemented);
        assert_eq!(
            err.message(),
            "StorageBackend: serializeModelList() not supported"
        );
    }

    #[test]
    fn capabilities_are_structural_and_defaultable() {
        let a = BackendCapabilities {
            supports_tiling: true,
            ..BackendCapabilities::default()
        };
        let b = BackendCapabilities::default();
        assert_ne!(a, b);
        let c = a;
        assert_eq!(a, c);
        // The default is "no capabilities advertised".
        assert_eq!(
            b,
            BackendCapabilities {
                supports_tiling: false,
                supports_streaming: false,
                supports_random_access: false,
                supports_cloud_streaming: false,
            }
        );
    }
}
