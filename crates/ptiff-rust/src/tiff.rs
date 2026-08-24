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
/// The write path mirrors the read tier: [`Tiff::to_bytes`]/[`Tiff::write`]
/// serialize image *metadata* through the core's canonical schema, while
/// [`Tiff::to_bytes_with_pixels`] additionally embeds each image's raw pixel
/// payload (round-tripping through [`Tiff::read_image_pixels`]).
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

/// Selects how [`Tiff`]'s pixel-write kernel encodes tiles.
///
/// [`WriteMode::Sequential`] is always available and writes each tile through
/// [`ptiff_core::io::ImageSink::write_tile`]. [`WriteMode::Parallel`] only
/// exists when the `parallel` feature is enabled and hands the CPU-bound
/// encoding of tiled + compressed images to the shared Rayon pool (the byte
/// layout is identical to sequential).
#[derive(Debug, Clone, Copy)]
enum WriteMode {
    /// Sequential `write_tile` encoding (available in every build).
    Sequential,
    /// Rayon-parallel tile encoding (only compiled with the `parallel` feature).
    #[cfg(feature = "parallel")]
    Parallel,
}

impl Tiff {
    /// Reads and parses a PTIFF/TIFF/BigTIFF file from disk.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorCode::InvalidArgument`](crate::ErrorCode::InvalidArgument) if the file is not a
    /// valid TIFF/BigTIFF container, [`ErrorCode::NotFound`](crate::ErrorCode::NotFound)
    /// if the path does not exist, or [`ErrorCode::InvalidArgument`](crate::ErrorCode::InvalidArgument)
    /// for other filesystem errors reading the path.
    pub fn open(path: impl AsRef<Path>) -> Result<Tiff> {
        let bytes = std::fs::read(path).map_err(|e| {
            // A missing/unreadable path is a "not found" condition, not a
            // malformed-input error: surface it as `NotFound` so the C-ABI
            // `ptiff_open_path` / `ptiff_source_open` report
            // `-PTIFF_ERROR_NOT_FOUND` to foreign runtimes (matching the
            // C++ oracle's semantics), instead of conflating it with an
            // invalid TIFF stream.
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::not_found(format!("Tiff::open: file not found: {e}"))
            } else {
                Error::invalid_argument(format!("Tiff::open: cannot read file: {e}"))
            }
        })?;
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

    /// Decodes **every** tile of the `image_index`-th image in parallel (Phase
    /// 5), returning one owned `Vec<u8>` per tile in row-major order (tile row,
    /// then tile column — the same order [`Tiff::read_image_pixels`] concatenates).
    ///
    /// The on-disk tile bytes are read **sequentially** with a single cursor
    /// (I/O is not the bottleneck), then the CPU-bound decompression of every
    /// tile is handed to the shared Rayon pool — one job per tile over owned,
    /// race-free buffers. The output is **byte-identical** to calling
    /// [`Tiff::read_tile`] for every grid cell in order: Rayon's ordered
    /// `collect` preserves input order and the (lossless) codecs are pure
    /// functions. This is the low-level building block for
    /// [`Tiff::read_image_pixels_parallel`].
    ///
    /// Only compiled with the `parallel` feature.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorCode::NotFound`](crate::ErrorCode::NotFound) if `image_index` is out of
    /// range, or a backend error if a tile cannot be read/decompressed (the
    /// first failing tile by input order is returned).
    #[cfg(feature = "parallel")]
    pub fn read_all_tiles_parallel(&self, image_index: usize) -> Result<Vec<Vec<u8>>> {
        use ptiff_core::io::backend::tiff::TiffImageSource;

        let directory = Self::directory_at(&self.bytes, image_index)?;
        let mut reader = MemoryBinaryReader::from_slice(&self.bytes);
        let mut source = TiffImageSource::new(&mut reader, directory);
        source.read_all_tiles_parallel()
    }

    /// Parallel counterpart to [`Tiff::read_image_pixels`]: decodes the
    /// `index`-th image's full raster, decompressing every tile through the
    /// shared Rayon pool. The returned buffer is **byte-identical** to
    /// [`Tiff::read_image_pixels`] — only the CPU-bound decompression path is
    /// parallelized.
    ///
    /// Only compiled with the `parallel` feature.
    ///
    /// # Errors
    ///
    /// Same as [`Tiff::read_all_tiles_parallel`].
    #[cfg(feature = "parallel")]
    pub fn read_image_pixels_parallel(&self, image_index: usize) -> Result<Vec<u8>> {
        let tiles = self.read_all_tiles_parallel(image_index)?;
        let total: usize = tiles.iter().map(Vec::len).sum();
        let mut out = Vec::with_capacity(total);
        for tile in tiles {
            out.extend_from_slice(&tile);
        }
        Ok(out)
    }

    /// Resolves the [`TiffDirectory`](ptiff_core::io::backend::tiff::TiffDirectory)
    /// for `image_index` by walking `bytes`' IFD chain. The parallel read tier
    /// builds its concrete [`TiffImageSource`](ptiff_core::io::backend::tiff::TiffImageSource)
    /// from it (whose `read_all_tiles_parallel` is an inherent method, not part
    /// of the boxed `ImageSource` trait object).
    #[cfg(feature = "parallel")]
    fn directory_at(
        bytes: &[u8],
        image_index: usize,
    ) -> Result<ptiff_core::io::backend::tiff::TiffDirectory> {
        use ptiff_core::io::backend::tiff::{interpret_tiff_ifd, read_tiff_header, read_tiff_ifd};

        let mut reader = MemoryBinaryReader::from_slice(bytes);
        let header = read_tiff_header(&mut reader)?;
        let mut ifd_offset = header.first_ifd_offset;
        let mut directory = None;
        for i in 0..=image_index {
            let ifd = read_tiff_ifd(&mut reader, ifd_offset, header.endian, header.is_big_tiff)?;
            if i == image_index {
                let mut d = interpret_tiff_ifd(&ifd)?;
                d.endian = header.endian;
                directory = Some(d);
                break;
            }
            ifd_offset = ifd.next_ifd_offset();
            if ifd_offset == 0 {
                break;
            }
        }
        directory.ok_or_else(|| {
            Error::not_found(format!(
                "Tiff::directory_at: image index {image_index} out of range"
            ))
        })
    }

    // --- Pixel write tier (parallel) ----------------------------------

    /// Shared write kernel for both [`Tiff::to_bytes_with_pixels`]
    /// (`parallel = false`) and [`Tiff::to_bytes_with_pixels_parallel`]
    /// (`parallel = true`): serializes the scene's images, writes the header +
    /// IFD chain, forks each raster into the layout's tiles and writes them
    /// through a concrete [`TiffImageSink`](ptiff_core::io::backend::tiff::TiffImageSink).
    ///
    /// For tiled + compressed images the CPU-bound tile encoding is
    /// parallelized via `write_compressed_tiles_parallel`; other layouts use
    /// the sequential `write_tile` path. The on-disk byte layout is
    /// deterministic and identical between the two modes.
    ///
    /// A single-image scene goes through the single-image planner
    /// ([`plan_tiff_write`](ptiff_core::io::backend::tiff::plan_tiff_write)),
    /// which supports tiled + compressed — the multi-image planner
    /// ([`plan_tiff_write_multi`](ptiff_core::io::backend::tiff::plan_tiff_write_multi))
    /// does not. Multi-image scenes cannot combine tiled + compressed images
    /// (matching the core).
    fn write_pixels_to_writer(
        scene: &Scene,
        rasters: &[&[u8]],
        writer: &mut ptiff_core::io::MemoryBinaryWriter,
        mode: WriteMode,
    ) -> Result<()> {
        use ptiff_core::id::TileId;
        use ptiff_core::io::backend::tiff::header::write_tiff_header;
        use ptiff_core::io::backend::tiff::ifd_writer::write_tiff_ifd;
        use ptiff_core::io::backend::tiff::plan_tiff_write;
        use ptiff_core::io::backend::tiff::plan_tiff_write_multi;
        use ptiff_core::io::backend::tiff::tiff_ifd_byte_size;
        use ptiff_core::io::backend::tiff::TiffImageSink;
        use ptiff_core::io::BinaryWriter as _;
        use ptiff_core::io::ImageSink as _;
        use ptiff_core::tile::{Tile, TileExtent, TileIndex, TileRegion};

        let root = SceneSerializer.serialize(scene)?;
        let children: Vec<ptiff_core::io::StorageModel> = root.children().to_vec();
        if rasters.len() != children.len() {
            return Err(Error::invalid_argument(format!(
                "Tiff::to_bytes_with_pixels: expected {} raster(s) for {} image(s), got {}",
                children.len(),
                children.len(),
                rasters.len()
            )));
        }

        // Use the single-image planner for one image (supports tiled +
        // compressed), the multi-image planner otherwise.
        let plans: Vec<ptiff_core::io::backend::tiff::TiffWritePlan> = if children.len() == 1 {
            vec![plan_tiff_write(&children[0])?]
        } else {
            plan_tiff_write_multi(&children)?.images
        };
        let is_big = plans.first().map(|p| p.is_big_tiff).unwrap_or(false);
        let header_size: u64 = if is_big { 16 } else { 8 };

        // Header + IFD chain, each IFD linked to the next; last links to 0.
        write_tiff_header(writer, header_size, is_big)?;
        let mut cursor = header_size;
        for (i, plan) in plans.iter().enumerate() {
            let ifd_size = tiff_ifd_byte_size(&plan.entries, is_big);
            let next = if i + 1 < plans.len() {
                cursor + ifd_size
            } else {
                0
            };
            write_tiff_ifd(writer, plan.entries.clone(), is_big, next)?;
            cursor += ifd_size;
        }

        for (image_index, raster) in rasters.iter().enumerate() {
            let image = scene.image_at(image_index)?;
            let bps = image.pixel_type().bytes_per_sample() as u64;
            let channels = u64::from(image.channel_count());

            // Concrete sink (parallel encoding requires the inherent
            // `write_compressed_tiles_parallel`, which a boxed `Box<dyn
            // ImageSink>` does not expose).
            let plan = plans.get(image_index).ok_or_else(|| {
                Error::out_of_range("Tiff::write_pixels_to_writer: image out of range")
            })?;
            let data_offset = plan.directory.tile_byte_ranges.first().ok_or_else(|| {
                Error::invalid_argument("Tiff::write_pixels_to_writer: image has no byte ranges")
            })?;
            writer.seek(data_offset.offset)?;
            let mut sink = TiffImageSink::new(writer, plan.directory.clone());
            let layout = *sink.layout();
            let tile_w = u64::from(layout.tile_size.width);
            let tile_h = u64::from(layout.tile_size.height);
            let tile_bytes = tile_w * tile_h * channels * bps;
            let columns = u64::from(layout.columns(0));
            let rows = u64::from(layout.rows(0));
            let expected = rows * columns * tile_bytes;
            if raster.len() as u64 != expected {
                let msg = format!(
                    "Tiff::to_bytes_with_pixels: image {image_index} needs {expected} pixel bytes ({}x{} grid of {}x{} tiles x {} samples), got {}",
                    layout.columns(0), layout.rows(0),
                    layout.tile_size.width, layout.tile_size.height, channels * bps, raster.len()
                );
                return Err(Error::invalid_argument(msg));
            }

            // Tiled + compressed => optional parallel encoding; otherwise
            // sequential write_tile. The parallel arm is only compiled when
            // the `parallel` feature is on; the sequential arm is always
            // available and produces the exact same byte layout.
            let tile_usize = tile_bytes as usize;
            let mut offset = 0usize;
            match mode {
                WriteMode::Sequential => {
                    for row in 0..rows {
                        for column in 0..columns {
                            let start = offset;
                            let end = start + tile_usize;
                            let tile = Tile::new(
                                TileId::new(0),
                                TileIndex::new(column as u32, row as u32, 0),
                                TileRegion::new(
                                    (column as u32) * layout.tile_size.width,
                                    (row as u32) * layout.tile_size.height,
                                    TileExtent::new(
                                        layout.tile_size.width,
                                        layout.tile_size.height,
                                    ),
                                ),
                                &raster[start..end],
                            );
                            sink.write_tile(&tile)?;
                            offset = end;
                        }
                    }
                }
                #[cfg(feature = "parallel")]
                WriteMode::Parallel => {
                    let compressed_tiled = image.tile_info().is_some()
                        && image.compression().is_some()
                        && image.compression() != Some(ptiff_core::CompressionKind::None);
                    if compressed_tiled {
                        // Hand the CPU-bound encoding to the Rayon pool
                        // (deterministic byte layout; the write stays
                        // sequential). Small tile sets fall back internally.
                        let mut raw_tiles: Vec<Vec<u8>> =
                            Vec::with_capacity((rows * columns) as usize);
                        for _row in 0..rows {
                            for _column in 0..columns {
                                let start = offset;
                                let end = start + tile_usize;
                                raw_tiles.push(raster[start..end].to_vec());
                                offset = end;
                            }
                        }
                        sink.write_compressed_tiles_parallel(&raw_tiles)?;
                    } else {
                        for row in 0..rows {
                            for column in 0..columns {
                                let start = offset;
                                let end = start + tile_usize;
                                let tile = Tile::new(
                                    TileId::new(0),
                                    TileIndex::new(column as u32, row as u32, 0),
                                    TileRegion::new(
                                        (column as u32) * layout.tile_size.width,
                                        (row as u32) * layout.tile_size.height,
                                        TileExtent::new(
                                            layout.tile_size.width,
                                            layout.tile_size.height,
                                        ),
                                    ),
                                    &raster[start..end],
                                );
                                sink.write_tile(&tile)?;
                                offset = end;
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Parallel counterpart to [`Tiff::to_bytes_with_pixels`]: encodes each
    /// image's tile payloads through the shared Rayon pool when the image is
    /// tiled + compressed, and writes the header + IFD chain plus every
    /// raster. The emitted bytes are **byte-identical** to
    /// [`Tiff::to_bytes_with_pixels`] and round-trip through
    /// [`Tiff::read_image_pixels`] / [`Tiff::read_image_pixels_parallel`].
    ///
    /// Only compiled with the `parallel` feature.
    ///
    /// # Errors
    ///
    /// Same as [`Tiff::to_bytes_with_pixels`].
    #[cfg(feature = "parallel")]
    pub fn to_bytes_with_pixels_parallel(scene: &Scene, rasters: &[&[u8]]) -> Result<Vec<u8>> {
        use ptiff_core::io::MemoryBinaryWriter;

        let mut writer = MemoryBinaryWriter::new();
        Self::write_pixels_to_writer(scene, rasters, &mut writer, WriteMode::Parallel)?;
        Ok(writer.take_buffer())
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
    /// canonical schema. The emitted bytes encode the scene's image metadata
    /// (header + IFDs); use [`Tiff::to_bytes_with_pixels`] to also embed each
    /// image's raw pixel payload.
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

    /// Encodes a `Scene` — including each image's raw pixel payload — into
    /// PTIFF/TIFF bytes.
    ///
    /// This is the pixel-carrying counterpart to [`Tiff::to_bytes`]: it writes
    /// the same metadata (header + IFD chain), but additionally fills each
    /// image's data region with the caller-supplied raster, so the returned
    /// bytes round-trip through [`Tiff::from_bytes`] + [`Tiff::read_image_pixels`]
    /// back to the exact same raster length and contents.
    ///
    /// **Raster layout.** `rasters` supplies one contiguous, row-major,
    /// channel-interleaved raster per image, in file order (`index` `i`
    /// ↔ `scene.image(i)`). The layout of `rasters[i]` is **exactly what
    /// [`Tiff::read_image_pixels`] yields** when the same `Scene` is written by
    /// this method: the grid's tiles concatenated row-by-row (tile row, then
    /// tile column), where each tile carries its full tile size
    /// (`tile_width * tile_height * channel_count * bytes_per_sample` bytes,
    /// edge tiles padded). For the common untile case that is simply the
    /// natural image raster, `width * height * channel_count *
    /// bytes_per_sample` bytes in one spanning tile.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorCode::InvalidArgument`](crate::ErrorCode::InvalidArgument) if the number of
    /// rasters does not match `scene.image_count()`, a raster's length does not
    /// match its image's pixel grid, or the scene cannot be represented (see
    /// [`Tiff::to_bytes`]).
    pub fn to_bytes_with_pixels(scene: &Scene, rasters: &[&[u8]]) -> Result<Vec<u8>> {
        use ptiff_core::io::MemoryBinaryWriter;

        // Shared kernel: writes header + IFD chain and every image's tiles
        // through a concrete sink (so single-image tiled + compressed scenes
        // are supported and the output is byte-identical to the parallel
        // variant). Sequential mode always uses the sequential write_tile path.
        let mut writer = MemoryBinaryWriter::new();
        Self::write_pixels_to_writer(scene, rasters, &mut writer, WriteMode::Sequential)?;
        Ok(writer.take_buffer())
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
    fn write_image_pixels_round_trips_a_grayscale_strip() {
        // Idiomatic write tier carries real pixels: build a scene whose single
        // image describes an 8-bit grayscale raster, write pixels, read back.
        let width = 40u32;
        let height = 30u32;
        let mut scene = Scene::new();
        let mut desc = ImageDescriptor::new(width, height);
        desc.channel_count = 1; // UInt8, uncompressed, untiled
        scene.add_image(desc).expect("add image");

        let raster: Vec<u8> = (0..(width * height) as usize)
            .map(|i| (i * 31 % 251) as u8)
            .collect();
        let bytes = Tiff::to_bytes_with_pixels(&scene, &[raster.as_slice()]).expect("write");

        let tiff = Tiff::from_bytes(&bytes).expect("parse");
        assert_eq!(tiff.scene().image_count(), 1);
        let read = tiff.read_image_pixels(0).expect("read pixels");
        assert_eq!(read, raster, "pixel round-trip must preserve the raster");
    }

    #[test]
    fn write_image_pixels_round_trips_multi_image_scene() {
        // Two images of different width/height/sample-depth and channel count:
        // the first is uncompressed 8-bit grayscale, the second uncompressed
        // 16-bit RGB (interleaved). The raster layout matches
        // read_image_pixels' output for every image.
        let mut scene = Scene::new();
        let mut gray = ImageDescriptor::new(24, 18);
        gray.channel_count = 1;
        scene.add_image(gray).expect("gray");

        let mut rgb = ImageDescriptor::new(20, 16);
        rgb.pixel_type = PixelType::UInt16;
        rgb.channel_count = 3;
        scene.add_image(rgb).expect("rgb");

        let gray_raster: Vec<u8> = (0..(24 * 18) as usize).map(|i| i as u8).collect();
        let rgb_raster: Vec<u16> = (0..(20usize * 16 * 3))
            .map(|i| (i * 7 % 65521) as u16)
            .collect();
        let rgb_bytes: Vec<u8> = rgb_raster.iter().flat_map(|v| v.to_le_bytes()).collect();

        let bytes =
            Tiff::to_bytes_with_pixels(&scene, &[gray_raster.as_slice(), rgb_bytes.as_slice()])
                .expect("write");
        let tiff = Tiff::from_bytes(&bytes).expect("parse");
        assert_eq!(tiff.scene().image_count(), 2);

        let gray_read = tiff.read_image_pixels(0).expect("gray");
        assert_eq!(gray_read, gray_raster);
        let rgb_read = tiff.read_image_pixels(1).expect("rgb");
        assert_eq!(
            rgb_read, rgb_bytes,
            "interleaved 16-bit RGB must round-trip"
        );
    }

    #[test]
    fn write_image_pixels_round_trips_a_tiled_image_with_padding() {
        // A tiled image of 38x34 pixels with 16x16 tiles: tiles at the right
        // and bottom edges are padded out to the full 16x16 tile size. The
        // idiomatic write tier must lay the supplied raster out as
        // `read_image_pixels` reads it back (concatenated full-size tiles).
        use ptiff_core::TileInfo;

        let tile = 16u32;
        let width = 38u32;
        let height = 34u32;
        let mut scene = Scene::new();
        let mut desc = ImageDescriptor::new(width, height);
        desc.channel_count = 1;
        desc.tile_info = Some(TileInfo::new(tile, tile));
        scene.add_image(desc).expect("add image");

        // Grid is ceil(38/16) x ceil(34/16) = 3x3 tiles, each 16x16 = 256 bytes,
        // so the raster is 3*3*256 = 2304 bytes (edge tiles padded).
        let raster: Vec<u8> = (0..2304u32).map(|i| (i * 3 + 1) as u8).collect();
        let bytes = Tiff::to_bytes_with_pixels(&scene, &[raster.as_slice()]).expect("write");

        let tiff = Tiff::from_bytes(&bytes).expect("parse");
        let image = tiff.image(0).expect("image");
        assert_eq!((image.width(), image.height()), (width, height));
        // The on-disk layout reflects the 3x3 tile grid requested via
        // `tile_info` (column/row counts come from the file's TileWidth/Length).
        assert_eq!(
            (
                tiff.tile_layout(0).expect("layout").columns(0),
                tiff.tile_layout(0).expect("layout").rows(0)
            ),
            (3, 3)
        );

        let read = tiff.read_image_pixels(0).expect("read pixels");
        assert_eq!(read, raster, "tiled raster must round-trip with padding");
    }

    #[test]
    fn write_image_pixels_validates_raster_count_and_size() {
        let mut scene = Scene::new();
        let mut desc = ImageDescriptor::new(8, 8);
        desc.channel_count = 1;
        scene.add_image(desc).expect("add image");

        // No rasters supplied for one image => error.
        let err = Tiff::to_bytes_with_pixels(&scene, &[]).expect_err("must reject empty rasters");
        assert_eq!(err.code(), ptiff_core::ErrorCode::InvalidArgument);
        // Too many rasters => error.
        let err = Tiff::to_bytes_with_pixels(&scene, &[&[0u8; 64][..], &[0u8; 64][..]])
            .expect_err("must reject too many rasters");
        assert_eq!(err.code(), ptiff_core::ErrorCode::InvalidArgument);
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
        // RFC-0004 carries body / projection / reference_frame only: the name
        // is re-derived from the NAIF body id, and the ellipsoid (not part of
        // the registered schema) is deliberately lossy → UNSPECIFIED.
        assert_eq!(crs.planet().name(), "Moon");
        assert_eq!(crs.planet().iau_identifier(), "301");
        assert_eq!(crs.planet().ellipsoid(), Ellipsoid::UNSPECIFIED);
        assert_eq!(crs.frame_override(), Some(Frame::SPACECRAFT));
        assert_eq!(crs.projection().kind(), ProjectionKind::Stereographic);
    }

    #[test]
    fn generic_extension_metadata_round_trips_through_file_bytes() {
        // Exercises the non-typed PTIFF domains (spice 65001, layers 65004,
        // provenance 65005) plus an unknown/future key: each carried as a
        // generic `ptiff.<domain>.<key>` metadata record and round-tripped
        // byte-identically through the facade.
        let mut scene = Scene::new();
        let mut desc = ImageDescriptor::new(16, 16);
        desc.metadata
            .insert("ptiff.spice.frame".to_string(), "IAU_MOON".to_string());
        desc.metadata
            .insert("ptiff.spice.time_system".to_string(), "TDB".to_string());
        desc.metadata
            .insert("ptiff.layers.dem".to_string(), "dem".to_string());
        desc.metadata.insert(
            "ptiff.provenance.software".to_string(),
            "libptiff".to_string(),
        );
        // Unknown/future key must survive verbatim (RFC-7002 §4.3).
        desc.metadata.insert(
            "ptiff.provenance.future_field".to_string(),
            "keep-me".to_string(),
        );
        scene.add_image(desc).expect("add image");

        let bytes = Tiff::to_bytes(&scene).expect("serialize");
        let read = Tiff::from_bytes(&bytes).expect("parse");
        let image = read.image(0).expect("image");

        let metadata = image.metadata();
        assert_eq!(
            metadata.get("ptiff.spice.frame").map(String::as_str),
            Some("IAU_MOON")
        );
        assert_eq!(
            metadata.get("ptiff.spice.time_system").map(String::as_str),
            Some("TDB")
        );
        assert_eq!(
            metadata.get("ptiff.layers.dem").map(String::as_str),
            Some("dem")
        );
        assert_eq!(
            metadata
                .get("ptiff.provenance.software")
                .map(String::as_str),
            Some("libptiff")
        );
        assert_eq!(
            metadata
                .get("ptiff.provenance.future_field")
                .map(String::as_str),
            Some("keep-me")
        );
        // The typed camera/CRS domains are NOT surfaced through the generic
        // metadata map when absent.
        assert_eq!(image.camera(), None);
        assert_eq!(image.crs(), None);
    }

    #[test]
    fn generic_and_typed_metadata_coexist_without_collision() {
        // A scene carrying both a typed camera (65002) and generic spice
        // (65001) / provenance (65005) metadata must round-trip BOTH: the
        // generic map excludes the reserved camera/CRS keys, so neither the
        // IFD entries nor the Scene fields collide.
        use ptiff_core::geometry::Ellipsoid;
        use ptiff_core::{CoordinateReferenceSystem, Frame, Planet, Projection, ProjectionKind};

        let mut scene = Scene::new();
        let mut desc = ImageDescriptor::new(16, 16);
        desc.camera = Some(ptiff_core::Camera::from_model(
            "pinhole",
            ptiff_core::Intrinsics::new(100.0, 100.0, 8.0, 8.0),
            ptiff_core::Extrinsics::new(
                ptiff_core::Quaternion::new(1.0, 0.0, 0.0, 0.0),
                ptiff_core::Vec3::new(0.0, 0.0, 0.0),
            ),
            "t0",
        ));
        desc.crs = Some(CoordinateReferenceSystem::new(
            Planet::new(
                "Moon",
                "301",
                Ellipsoid::new(1_737_400.0, 1_737_400.0),
                Frame::IAU_MOON,
            ),
            None,
            Projection::new(ProjectionKind::Equirectangular),
        ));
        desc.metadata
            .insert("ptiff.spice.frame".to_string(), "IAU_MOON".to_string());
        desc.metadata.insert(
            "ptiff.provenance.software".to_string(),
            "ptiff-rust".to_string(),
        );
        scene.add_image(desc).expect("add image");

        let bytes = Tiff::to_bytes(&scene).expect("serialize");
        let read = Tiff::from_bytes(&bytes).expect("parse");
        let image = read.image(0).expect("image");

        // Typed domains still round-trip.
        assert_eq!(image.camera().expect("camera").model_name(), "pinhole");
        assert_eq!(image.crs().expect("crs").planet().name(), "Moon");
        // Generic domains round-trip too.
        assert_eq!(
            image
                .metadata()
                .get("ptiff.spice.frame")
                .map(String::as_str),
            Some("IAU_MOON")
        );
        assert_eq!(
            image
                .metadata()
                .get("ptiff.provenance.software")
                .map(String::as_str),
            Some("ptiff-rust")
        );
        // And no generic record collides with the typed camera/CRS domains.
        assert!(image
            .metadata()
            .keys()
            .all(|k| { !k.starts_with("ptiff.camera.") && !k.starts_with("ptiff.crs.") }));
    }

    #[test]
    fn partial_metadata_domains_do_not_fail_file_read() {
        // A foreign/future writer may leave only part of a typed metadata
        // domain (RFC-7002 §3.3/§4.4 tolerance): an incomplete `ptiff.camera.*`
        // / `ptiff.crs.*` payload must not fail `Tiff::from_bytes`. The typed
        // fields stay absent and the partial raw fields survive in the generic
        // metadata map.
        let mut model = ptiff_core::io::StorageModel::new();
        model.set_field("imageWidth", "16");
        model.set_field("imageHeight", "16");
        model.set_field("samplesPerPixel", "1");
        model.set_field("pixelType", "UInt8");
        model.set_field("ptiff.camera.model", "pinhole"); // partial: no intrinsics
        model.set_field("ptiff.spice.frame", "IAU_MOON");

        use ptiff_core::io::backend::tiff::TiffBackend as CoreBackend;
        use ptiff_core::io::{MemoryBinaryWriter, StorageBackend as _};
        let mut writer = MemoryBinaryWriter::new();
        CoreBackend
            .serialize_model(&model, &mut writer)
            .expect("serialize model with partial tags");

        let bytes = writer.take_buffer();
        // Read back through the idiomatic facade: must succeed, not fail.
        let tiff = Tiff::from_bytes(&bytes).expect("must not fail on partial metadata");
        let image = tiff.image(0).expect("image");

        // Typed camera is absent (incomplete), its raw field is preserved.
        assert_eq!(image.camera(), None);
        assert_eq!(
            image
                .metadata()
                .get("ptiff.camera.model")
                .map(String::as_str),
            Some("pinhole")
        );
        // And a complete generic domain still round-trips.
        assert_eq!(
            image
                .metadata()
                .get("ptiff.spice.frame")
                .map(String::as_str),
            Some("IAU_MOON")
        );
    }

    // --- Phase 5: parallel facade (feature = "parallel") --------------

    /// Builds a tiled + LZW scene whose full-tile raster holds deterministic
    /// banded data (compressible so LZW + differencing exercise the codec).
    #[cfg(feature = "parallel")]
    fn tiled_lzw_scene(width: u32, height: u32, tile: u32, rows_bands: u32) -> (Scene, Vec<u8>) {
        use ptiff_core::TileInfo;

        let mut scene = Scene::new();
        let mut desc = ImageDescriptor::new(width, height);
        desc.channel_count = 1;
        desc.tile_info = Some(TileInfo::new(tile, tile));
        desc.compression = Some(CompressionKind::Lzw);
        scene.add_image(desc).expect("add image");

        // Raster is ceil(w/tile) x ceil(h/tile) full-size (padded) tiles.
        let cols = width.div_ceil(tile) as usize;
        let rows = height.div_ceil(tile) as usize;
        let tile_bytes = (tile * tile) as usize;
        let raster: Vec<u8> = (0..(rows * cols * tile_bytes) as u32)
            .map(|i| ((i / (tile * rows_bands)) % 17) as u8)
            .collect();
        (scene, raster)
    }

    #[test]
    #[cfg(feature = "parallel")]
    fn parallel_write_equals_sequential_write_bytes() {
        // A scene large enough to cross the parallelize threshold (>= 8 tiles).
        let (scene, raster) = tiled_lzw_scene(64, 48, 16, 3); // 4 x 3 = 12 tiles

        let seq = Tiff::to_bytes_with_pixels(&scene, &[raster.as_slice()]).expect("sequential");
        let par =
            Tiff::to_bytes_with_pixels_parallel(&scene, &[raster.as_slice()]).expect("parallel");

        assert_eq!(
            seq, par,
            "parallel write must be byte-identical to sequential write"
        );
    }

    #[test]
    #[cfg(feature = "parallel")]
    fn parallel_read_equals_sequential_read() {
        let (scene, raster) = tiled_lzw_scene(64, 48, 16, 3); // 12 tiles
        let bytes = Tiff::to_bytes_with_pixels(&scene, &[raster.as_slice()]).expect("write");
        let tiff = Tiff::from_bytes(&bytes).expect("parse");

        let seq = tiff.read_image_pixels(0).expect("sequential read");
        let par = tiff.read_image_pixels_parallel(0).expect("parallel read");

        assert_eq!(par, seq, "parallel read must equal sequential read");
        assert_eq!(par, raster, "decoded raster must match what was written");

        // Per-tile parallel read returns the same ordered tile set as the
        // concatenated sequential read.
        let tiles = tiff.read_all_tiles_parallel(0).expect("parallel tiles");
        let concatenated: Vec<u8> = tiles.into_iter().flatten().collect();
        assert_eq!(concatenated, raster);
    }

    #[test]
    #[cfg(feature = "parallel")]
    fn parallel_write_then_parallel_read_round_trips() {
        let (scene, raster) = tiled_lzw_scene(80, 64, 16, 4); // 5 x 4 = 20 tiles
        let par = Tiff::to_bytes_with_pixels_parallel(&scene, &[raster.as_slice()])
            .expect("parallel write");
        let tiff = Tiff::from_bytes(&par).expect("parse");

        assert_eq!(
            tiff.read_image_pixels_parallel(0).expect("parallel read"),
            raster,
            "parallel write -> parallel read must round-trip the raster"
        );
    }

    #[test]
    #[cfg(feature = "parallel")]
    fn parallel_read_out_of_range_is_an_error() {
        let (scene, raster) = tiled_lzw_scene(32, 32, 16, 2); // 2 x 2
        let bytes = Tiff::to_bytes_with_pixels(&scene, &[raster.as_slice()]).expect("write");
        let tiff = Tiff::from_bytes(&bytes).expect("parse");

        assert!(tiff.read_all_tiles_parallel(0).is_ok());
        assert!(tiff.read_all_tiles_parallel(1).is_err());
    }

    #[test]
    #[cfg(feature = "parallel")]
    fn parallel_write_small_tile_set_falls_back_to_sequential() {
        // Below the parallelize threshold (8 tiles) the parallel write must
        // still produce the same bytes (sequential fallback inside the core).
        let (scene, raster) = tiled_lzw_scene(32, 32, 16, 2); // 2 x 2 = 4 tiles
        let seq = Tiff::to_bytes_with_pixels(&scene, &[raster.as_slice()]).expect("sequential");
        let par =
            Tiff::to_bytes_with_pixels_parallel(&scene, &[raster.as_slice()]).expect("parallel");
        assert_eq!(seq, par);
    }
}
