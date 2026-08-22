//! High-level TIFF/BigTIFF/PTIFF file access.
//!
//! [`Tiff`] is the idiomatic entry point for reading a PTIFF file: it parses
//! the on-disk container with the core's [`TiffBackend`], turns the resulting
//! storage model into a typed [`Scene`], and exposes that scene's images.

use std::path::Path;

use ptiff_core::io::backend::tiff::TiffBackend;
use ptiff_core::io::{MemoryBinaryReader, SceneDeserializer, SceneSerializer};
use ptiff_core::{Deserializer, Error, Result, Scene, Serializer, StorageBackend};

/// A parsed PTIFF/TIFF/BigTIFF file.
///
/// Read-only for now: `Tiff` parses a byte source into a typed [`Scene`]. The
/// pixel (tile) tier and the write path are added in a later increment.
///
/// # Examples
///
/// ```no_run
/// use ptiff::Tiff;
///
/// let tiff = Tiff::open("image.ptiff")?;
/// let scene = tiff.scene();
/// println!("{} image(s)", scene.image_count());
/// # Ok::<_, ptiff::Error>(())
/// ```
#[derive(Debug)]
pub struct Tiff {
    scene: Scene,
}

impl Tiff {
    /// Reads and parses a PTIFF/TIFF/BigTIFF file from disk.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorCode::InvalidArgument`](crate::ErrorCode::InvalidArgument) (or the underlying backend
    /// error) if the file is not a valid TIFF/BigTIFF container, or a
    /// filesystem error if the path cannot be read.
    pub fn open(path: impl AsRef<Path>) -> Result<Tiff> {
        let bytes = std::fs::read(path)
            .map_err(|e| Error::invalid_argument(format!("Tiff::open: cannot read file: {e}")))?;
        Tiff::from_bytes(&bytes)
    }

    /// Parses a PTIFF/TIFF/BigTIFF byte stream in memory.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorCode::InvalidArgument`](crate::ErrorCode::InvalidArgument) if `bytes` is not a valid
    /// TIFF/BigTIFF container.
    pub fn from_bytes(bytes: &[u8]) -> Result<Tiff> {
        let mut reader = MemoryBinaryReader::from_slice(bytes);
        let backend = TiffBackend;
        let model = backend.deserialize_model(&mut reader)?;
        let scene = SceneDeserializer.deserialize(&model)?;
        Ok(Tiff { scene })
    }

    /// Returns the parsed scene (the composition of the file's images).
    #[must_use]
    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    /// Returns an iterator over the scene's images in file order.
    ///
    /// Equivalent to `self.scene().image_count()` + `image_at(i)`.
    pub fn images(&self) -> impl Iterator<Item = &ptiff_core::Image> {
        let scene = &self.scene;
        (0..scene.image_count()).filter_map(|i| scene.image_at(i).ok())
    }

    /// Serializes a `Scene` back to PTIFF/TIFF bytes through the core's
    /// canonical schema. Pixel (tile) data is not written in this increment;
    /// the emitted bytes encode the scene's image metadata (header + IFDs).
    ///
    /// # Errors
    ///
    /// Returns [`ErrorCode::InvalidArgument`](crate::ErrorCode::InvalidArgument) if the scene cannot be
    /// represented in the canonical storage schema (e.g. `Float64` samples,
    /// JPEG with non-`UInt8` samples, or a channel count other than 1/3).
    pub fn to_bytes(scene: &Scene) -> Result<Vec<u8>> {
        let model = SceneSerializer.serialize(scene)?;
        // A scene may contain several images; written as a multi-image TIFF
        // chain (one IFD per image). `serialize_model_list` is exactly what a
        // single storage model's children represent (each child carries
        // imageWidth/imageHeight/... directly).
        let mut writer = ptiff_core::io::MemoryBinaryWriter::new();
        TiffBackend.serialize_model_list(model.children(), &mut writer)?;
        Ok(writer.take_buffer())
    }

    /// Serializes a `Scene` to a PTIFF/TIFF file on disk.
    ///
    /// See [`Tiff::to_bytes`] for the serialization semantics.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorCode::InvalidArgument`](crate::ErrorCode::InvalidArgument) if the scene cannot be
    /// serialized, or a filesystem error if the file cannot be written.
    pub fn write(path: impl AsRef<Path>, scene: &Scene) -> Result<()> {
        let bytes = Tiff::to_bytes(scene)?;
        std::fs::write(path, bytes)
            .map_err(|e| Error::invalid_argument(format!("Tiff::write: cannot write file: {e}")))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ptiff_core::{CompressionKind, ImageDescriptor, PixelType};

    fn demo_scene() -> Scene {
        let mut scene = Scene::new();
        let mut first = ImageDescriptor::new(64, 32);
        first.channel_count = 1;
        scene.add_image(first).expect("add image");

        let mut second = ImageDescriptor::new(640, 480);
        second.pixel_type = PixelType::UInt16;
        second.channel_count = 3;
        // Only None/LZW round-trip symmetrically through the core's
        // SceneSerializer/SceneDeserializer (matches the C++ oracle: the
        // deserializer recognizes "None" and "LZW"/"Lzw" only). Untiled so the
        // current writer accepts the (compressed) layout.
        second.compression = Some(CompressionKind::Lzw);
        scene.add_image(second).expect("add image");
        scene
    }

    #[test]
    fn write_then_read_round_trips_scene_metadata() {
        let scene = demo_scene();
        let bytes = Tiff::to_bytes(&scene).expect("serialize");
        let read = Tiff::from_bytes(&bytes).expect("parse");
        let out = read.scene();
        assert_eq!(out.image_count(), 2);

        let first = out.image_at(0).expect("image 0");
        assert_eq!(first.width(), 64);
        assert_eq!(first.height(), 32);
        assert_eq!(first.channel_count(), 1);

        let second = out.image_at(1).expect("image 1");
        assert_eq!(second.width(), 640);
        assert_eq!(second.height(), 480);
        assert_eq!(second.pixel_type(), PixelType::UInt16);
        assert_eq!(second.channel_count(), 3);
        assert_eq!(second.compression(), Some(CompressionKind::Lzw));
        // tile_info is not preserved through a TIFF round-trip (matches core).
        assert_eq!(second.tile_info(), None);
    }

    #[test]
    fn images_iterator_yields_all_images_in_order() {
        let scene = demo_scene();
        let bytes = Tiff::to_bytes(&scene).expect("serialize");
        let tiff = Tiff::from_bytes(&bytes).expect("parse");
        let dims: Vec<(u32, u32)> = tiff.images().map(|i| (i.width(), i.height())).collect();
        assert_eq!(dims, vec![(64, 32), (640, 480)]);
    }

    #[test]
    fn reject_arbitrary_bytes() {
        let err = Tiff::from_bytes(b"not a tiff file").expect_err("must fail");
        assert_eq!(err.code(), ptiff_core::ErrorCode::InvalidArgument);
    }

    #[test]
    fn write_then_open_on_disk_round_trips() {
        let scene = demo_scene();
        let dir = std::env::temp_dir();
        let path = dir.join(format!("ptiff_api_test_{}.tiff", std::process::id()));
        Tiff::write(&path, &scene).expect("write file");
        let tiff = Tiff::open(&path).expect("open file");
        assert_eq!(tiff.scene().image_count(), scene.image_count());
        let img = tiff.images().nth(1).expect("second image");
        assert_eq!((img.width(), img.height()), (640, 480));
        // Clean up the temp file.
        let _ = std::fs::remove_file(&path);
    }
}
