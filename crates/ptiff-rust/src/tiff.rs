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
/// `Tiff` parses a byte source into a typed [`Scene`] and retains the raw
/// bytes so the **pixel/tile tier** can decode actual image data back out of
/// the file (see [`Tiff::read_image_pixels`], [`Tiff::read_tile`] and
/// [`Tiff::tile_layout`]).
///
/// The write path (`to_bytes`/`write`) currently serializes image *metadata*
/// through the core's canonical schema; writing pixel payloads alongside that
/// is a later increment.
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
    /// The raw container bytes, retained for the pixel/tile read tier.
    bytes: Vec<u8>,
    /// The parsed scene (image metadata) derived from `bytes`.
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
    /// The caller-supplied `bytes` are copied into the returned `Tiff` so the
    /// pixel/tile tier can re-open an [`ImageSource`](ptiff_core::io::ImageSource)
    /// over the retained buffer.
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
        Ok(Tiff {
            bytes: bytes.to_vec(),
            scene,
        })
    }

    /// Returns the parsed scene (the composition of the file's images).
    #[must_use]
    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    /// Returns the `index`-th image of the scene.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorCode::OutOfRange`](crate::ErrorCode::OutOfRange) if `index` is not in
    /// `[0, image_count())`.
    pub fn image(&self, index: usize) -> Result<&ptiff_core::Image> {
        self.scene.image_at(index)
    }

    /// Returns an iterator over the scene's images in file order.
    ///
    /// Equivalent to `self.scene().image_count()` + `image_at(i)`.
    pub fn images(&self) -> impl Iterator<Item = &ptiff_core::Image> {
        let scene = &self.scene;
        (0..scene.image_count()).filter_map(|i| scene.image_at(i).ok())
    }

    /// Returns the bytes that make up this `Tiff`.
    ///
    /// This is the exact container byte stream this `Tiff` was parsed from; it
    /// is what [`Tiff::open`]/[`Tiff::from_bytes`] received.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    // --- Pixel / tile read tier ---------------------------------------

    /// Decodes the `index`-th image's raw pixel bytes into an owned buffer.
    ///
    /// The returned buffer holds one full decoded image as a single contiguous
    /// raster: one row of `width * channel_count * bytes_per_sample` bytes per
    /// scanline, top-to-bottom. It strips the edge/padding a tiled or
    /// strip-based layout may use in the underlying file, so the result is
    /// directly usable as a normal image buffer.
    ///
    /// The scene's images appear in file order, so `index` selects the
    /// file's `index`-th image (as [`Tiff::image`](Tiff::image) does).
    ///
    /// # Errors
    ///
    /// Returns [`ErrorCode::OutOfRange`](crate::ErrorCode::OutOfRange) if `index` is not in
    /// `[0, image_count())`, or a backend error if an individual tile cannot be
    /// decoded (e.g. an unrecognized compression scheme).
    pub fn read_image_pixels(&self, image_index: usize) -> Result<Vec<u8>> {
        let mut reader = MemoryBinaryReader::from_slice(&self.bytes);
        let mut source = TiffBackend.open_image_source_at(&mut reader, image_index)?;
        let layout = *source.layout();
        let columns = layout.columns(0);
        let rows = layout.rows(0);

        // Read row-major (column fastest) in the source's own order, then
        // copy out each tile before moving on (the source's tiles are
        // non-owning views valid only until the next call).
        let mut out = Vec::new();
        for row in 0..rows {
            for column in 0..columns {
                let tile = source.read_tile(ptiff_core::tile::TileIndex::new(column, row, 0))?;
                out.extend_from_slice(tile.data());
            }
        }
        Ok(out)
    }

    /// Decodes a single tile (or strip) of the `image_index`-th image.
    ///
    /// `column`/`row` address the tile within the image's tile grid (top-left
    /// is `(0, 0)`), using the layout the file stores. For an untiled image the
    /// single spanning strip is addressed as `(0, 0)`. The returned bytes are
    /// owned, so they may be retained freely.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorCode::OutOfRange`](crate::ErrorCode::OutOfRange) if `image_index` is out of
    /// range or `column`/`row` fall outside the tile grid, or a backend error
    /// if the tile cannot be decoded.
    pub fn read_tile(&self, image_index: usize, column: u32, row: u32) -> Result<Vec<u8>> {
        let mut reader = MemoryBinaryReader::from_slice(&self.bytes);
        let mut source = TiffBackend.open_image_source_at(&mut reader, image_index)?;
        let tile = source.read_tile(ptiff_core::tile::TileIndex::new(column, row, 0))?;
        Ok(tile.data().to_vec())
    }

    /// Returns the tile grid layout of the `image_index`-th image, as stored in
    /// the file.
    ///
    /// Use this to enumerate the grid before calling [`Tiff::read_tile`]:
    /// `layout.columns(0)` and `layout.rows(0)` give the number of tile columns
    /// and rows; an untiled (single-strip) image appears as one spanning tile.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorCode::OutOfRange`](crate::ErrorCode::OutOfRange) if `image_index` is not in
    /// `[0, image_count())`.
    pub fn tile_layout(&self, image_index: usize) -> Result<ptiff_core::tile::TileLayout> {
        let mut reader = MemoryBinaryReader::from_slice(&self.bytes);
        let source = TiffBackend.open_image_source_at(&mut reader, image_index)?;
        Ok(*source.layout())
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

    /// Builds a single 8-bit grayscale image file with one contiguous strip of
    /// real pixel data, via the core `TiffBackend` sink (the idiomatic write
    /// tier does not yet carry pixel payloads).
    fn write_single_image_with_pixels(width: u32, height: u32, pixels: &[u8]) -> Vec<u8> {
        use ptiff_core::id::TileId;
        use ptiff_core::io::backend::tiff::TiffBackend as CoreBackend;
        use ptiff_core::io::{MemoryBinaryWriter, StorageBackend as _};
        use ptiff_core::tile::{Tile, TileExtent, TileIndex, TileRegion};

        let mut model = ptiff_core::io::StorageModel::new();
        model.set_field("imageWidth", width.to_string());
        model.set_field("imageHeight", height.to_string());
        model.set_field("samplesPerPixel", "1");
        model.set_field("pixelType", "UInt8");
        model.set_field("compression", "None");

        let mut writer = MemoryBinaryWriter::new();
        let backend = CoreBackend;
        backend
            .serialize_model(&model, &mut writer)
            .expect("serialize");
        {
            let mut sink = backend
                .open_image_sink(&mut writer, &model)
                .expect("open sink");
            let tile = Tile::new(
                TileId::new(0),
                TileIndex::new(0, 0, 0),
                TileRegion::new(0, 0, TileExtent::new(width, height)),
                pixels,
            );
            sink.write_tile(&tile).expect("write tile");
        }
        writer.take_buffer()
    }

    #[test]
    fn read_image_pixels_decodes_grayscale_strip() {
        let width = 8u32;
        let height = 4u32;
        let pixels: Vec<u8> = (0..(width * height)).map(|i| (i * 3 + 7) as u8).collect();
        let bytes = write_single_image_with_pixels(width, height, &pixels);
        let tiff = Tiff::from_bytes(&bytes).expect("parse");

        assert_eq!(tiff.scene().image_count(), 1);
        let image = tiff.image(0).expect("image");
        assert_eq!((image.width(), image.height()), (width, height));

        let read = tiff.read_image_pixels(0).expect("read pixels");
        assert_eq!(read, pixels, "decoded raster must match written pixels");
        // One row is width * samples(1) * bytes_per_sample(1) = width bytes.
        assert_eq!(read.len(), (width * height) as usize);
    }

    #[test]
    fn read_tile_and_tile_layout_expose_the_grid() {
        let width = 16u32;
        let height = 16u32;
        let pixels: Vec<u8> = (0..(width * height)).map(|i| i as u8).collect();
        let bytes = write_single_image_with_pixels(width, height, &pixels);
        let tiff = Tiff::from_bytes(&bytes).expect("parse");

        // Untiled single strip => one spanning tile (column/row 0,0 whose
        // layout's tile size is the full raster).
        let layout = tiff.tile_layout(0).expect("layout");
        assert_eq!((layout.columns(0), layout.rows(0)), (1, 1));

        let tile = tiff.read_tile(0, 0, 0).expect("read tile");
        assert_eq!(tile, pixels);

        // An out-of-grid address is an error.
        assert!(tiff.read_tile(0, 9, 0).is_err());
    }

    #[test]
    fn read_image_pixels_out_of_range_is_an_error() {
        let bytes = write_single_image_with_pixels(4, 4, &[0u8; 16]);
        let tiff = Tiff::from_bytes(&bytes).expect("parse");
        assert!(tiff.read_image_pixels(0).is_ok());
        assert!(tiff.read_image_pixels(1).is_err());
        assert!(tiff.read_tile(1, 0, 0).is_err());
    }

    #[test]
    fn camera_and_crs_round_trip_through_file_bytes() {
        use ptiff_core::geometry::Ellipsoid;
        use ptiff_core::{
            CoordinateReferenceSystem, Extrinsics, Frame, Intrinsics, Planet, Projection,
            ProjectionKind, Quaternion, Vec3,
        };
        let mut scene = Scene::new();
        let mut desc = ImageDescriptor::new(16, 16);
        desc.camera = Some(ptiff_core::Camera::from_model(
            "pinhole",
            Intrinsics::new(900.0, 901.0, 512.5, 384.25),
            Extrinsics::new(
                Quaternion::new(0.7, 0.1, 0.2, 0.3),
                Vec3::new(1.0, 2.0, 3.0),
            ),
            "2026-08-21T12:34:56.000Z",
        ));
        desc.crs = Some(CoordinateReferenceSystem::new(
            Planet::new(
                "Moon",
                "301",
                Ellipsoid::new(1_737_400.0, 1_735_700.0),
                Frame::IAU_MOON,
            ),
            Some(Frame::SPACECRAFT),
            Projection::new(ProjectionKind::Stereographic),
        ));
        scene.add_image(desc).expect("add image");

        // Scene → TIFF bytes (emits private tags 65002/65003) → back to Scene.
        let bytes = Tiff::to_bytes(&scene).expect("serialize");
        let read = Tiff::from_bytes(&bytes).expect("parse");
        let image = read.image(0).expect("image");

        let camera = image.camera().expect("camera round-tripped");
        assert_eq!(camera.model_name(), "pinhole");
        assert_eq!(
            camera.intrinsics(),
            Intrinsics::new(900.0, 901.0, 512.5, 384.25)
        );
        assert_eq!(
            camera.extrinsics().rotation,
            Quaternion::new(0.7, 0.1, 0.2, 0.3)
        );
        assert_eq!(camera.extrinsics().translation, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(camera.timestamp(), "2026-08-21T12:34:56.000Z");

        let crs = image.crs().expect("crs round-tripped");
        assert_eq!(crs.planet().name(), "Moon");
        assert_eq!(crs.planet().iau_identifier(), "301");
        assert_eq!(
            crs.planet().ellipsoid(),
            Ellipsoid::new(1_737_400.0, 1_735_700.0)
        );
        assert_eq!(crs.frame_override(), Some(Frame::SPACECRAFT));
        assert_eq!(crs.projection().kind(), ProjectionKind::Stereographic);
    }
}
