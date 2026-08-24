//! [`ImageSource`] over one TIFF/BigTIFF IFD's baseline image.
//!
//! Mirrors `ptiff::io::backend::tiff::tiff_image_source.{hpp,cpp}`. Reads each
//! tile/strip's raw bytes directly from a [`BinaryReader`] on demand, decodes
//! PackBits/LZW compression if present, then undoes Predictor=2 horizontal
//! differencing -- no upfront full-image read. The returned tile's data views
//! this source's own reusable buffer, so it stays valid only until the next
//! `read_tile` call.

use crate::id::TileId;
#[cfg(feature = "tiff-codecs")]
use crate::io::backend::tiff::compression::{decode_deflate, decode_jpeg};
use crate::io::backend::tiff::compression::{
    decode_lzw, decode_pack_bits, undo_horizontal_differencing,
};
use crate::io::backend::tiff::directory::{TiffCompression, TiffDirectory};
use crate::io::backend::tiff::pixel_format::bytes_per_sample;
use crate::io::backend::tiff::Endian;
use crate::io::{BinaryReader, ImageSource};
use crate::tile::{Tile, TileIndex};
use crate::{Error, Result};

/// Multiplies `a` and `b`, rejecting the result rather than silently
/// overflowing.
fn checked_multiply(a: u64, b: u64) -> Result<u64> {
    a.checked_mul(b).ok_or_else(|| {
        Error::invalid_argument("TiffImageSource::readTile: tile size computation overflows")
    })
}

/// [`ImageSource`] over one TIFF IFD. Not thread-safe, same contract as its
/// base trait.
pub struct TiffImageSource<'a> {
    reader: &'a mut dyn BinaryReader,
    directory: TiffDirectory,
    raw_buffer: Vec<u8>,
    buffer: Vec<u8>,
}

impl<'a> TiffImageSource<'a> {
    /// Constructs the source over `reader` and the resolved directory.
    pub fn new(reader: &'a mut dyn BinaryReader, directory: TiffDirectory) -> Self {
        Self {
            reader,
            directory,
            raw_buffer: Vec::new(),
            buffer: Vec::new(),
        }
    }

    /// Snapshot of the per-tile `Copy` parameters needed to decode a tile —
    /// the owned, thread-safe projection the parallel path captures once.
    fn decode_snapshot(&self) -> TileDecodeParams {
        TileDecodeParams {
            width: self.directory.layout.tile_size.width,
            height: self.directory.layout.tile_size.height,
            samples_per_pixel: self.directory.samples_per_pixel,
            bytes_per_sample: bytes_per_sample(self.directory.pixel_type),
            predictor_horizontal: self.directory.predictor
                == crate::io::backend::tiff::TiffPredictor::HorizontalDifferencing,
            big_endian: self.directory.endian == Endian::Big,
            compression: self.directory.compression,
        }
    }
}

impl ImageSource for TiffImageSource<'_> {
    fn layout(&self) -> &crate::tile::TileLayout {
        &self.directory.layout
    }

    fn read_tile(&mut self, index: TileIndex) -> Result<Tile<'_>> {
        let region = self.directory.layout.region_for(index)?;

        let columns = self.directory.layout.columns(index.level);
        let linear_index = u64::from(index.row) * u64::from(columns) + u64::from(index.column);
        if linear_index as usize >= self.directory.tile_byte_ranges.len() {
            return Err(Error::out_of_range(
                "TiffImageSource::readTile: index outside byte range table",
            ));
        }

        let range = self.directory.tile_byte_ranges[linear_index as usize];

        let file_size = self.reader.size()?;
        if range.offset > file_size || range.byte_count > file_size - range.offset {
            return Err(Error::invalid_argument(
                "TiffImageSource::readTile: tile byte range exceeds underlying reader size",
            ));
        }

        let raw_len = range.byte_count as usize;
        self.raw_buffer.resize(raw_len, 0);
        self.reader.seek(range.offset)?;
        let n = self.reader.read(&mut self.raw_buffer)?;
        if n != raw_len {
            return Err(Error::invalid_argument(
                "TiffImageSource::readTile: truncated tile data",
            ));
        }

        // Expected decompressed size of one tile: width x height x samples x
        // bytes-per-sample. Overflow-checked.
        let expected_size = tile_decoded_size(
            self.directory.layout.tile_size.width,
            self.directory.layout.tile_size.height,
            self.directory.samples_per_pixel,
            bytes_per_sample(self.directory.pixel_type),
        )?;

        self.buffer = decode_tile_bytes(&self.raw_buffer, expected_size, self.decode_snapshot())?;

        Ok(Tile::new(
            TileId::new(linear_index),
            index,
            region,
            &self.buffer,
        ))
    }
}

/// Owned, `Copy` projection of the layout/directory parameters a single tile's
/// decoder needs. Captured once (per full-image read) and shared immutably
/// across a Rayon pool — the parallel path never borrows the `TiffImageSource`
/// or its `TiffDirectory` while workers run (Phase 5 §6.3).
#[derive(Debug, Clone, Copy)]
struct TileDecodeParams {
    width: u32,
    height: u32,
    samples_per_pixel: u32,
    bytes_per_sample: u8,
    predictor_horizontal: bool,
    big_endian: bool,
    compression: TiffCompression,
}

/// Overflow-checked expected decompressed byte count of one tile.
fn tile_decoded_size(
    width: u32,
    height: u32,
    samples_per_pixel: u32,
    bytes_per_sample: u8,
) -> Result<usize> {
    let s1 = checked_multiply(u64::from(width), u64::from(height))?;
    let s2 = checked_multiply(s1, u64::from(samples_per_pixel))?;
    let s3 = checked_multiply(s2, u64::from(bytes_per_sample))?;
    Ok(s3 as usize)
}

/// Decodes one tile's raw (possibly compressed) bytes into raw pixel bytes,
/// undoing the compression scheme and, when configured, the Predictor=2
/// horizontal differencing.
///
/// This is the **owned, `Send + Sync`** form of the decoder: a pure function
/// over `raw` and an immutable [`TileDecodeParams`] snapshot, so it is safe to
/// invoke from multiple Rayon workers concurrently (Phase 5 §6.3). `expected_size`
/// is the constraint that guards against decompression bombs (a hostile stream
/// cannot make us allocate more than the tile's true size).
///
/// # Errors
///
/// Propagates codec failures; returns [`ErrorCode::NotImplemented`] when a
/// configured codec is disabled at compile time.
fn decode_tile_bytes(raw: &[u8], expected_size: usize, p: TileDecodeParams) -> Result<Vec<u8>> {
    let decoded = match p.compression {
        TiffCompression::None => {
            if raw.len() != expected_size {
                return Err(Error::invalid_argument(
                    "TiffImageSource::readTile: uncompressed tile size does not match the expected size",
                ));
            }
            raw.to_vec()
        }
        TiffCompression::Lzw => decode_lzw(raw, expected_size)?,
        TiffCompression::PackBits => decode_pack_bits(raw, expected_size)?,
        TiffCompression::Deflate => {
            #[cfg(feature = "tiff-codecs")]
            {
                decode_deflate(raw, expected_size)?
            }
            #[cfg(not(feature = "tiff-codecs"))]
            {
                return Err(Error::not_implemented(
                    "TiffImageSource::readTile: Deflate decoding requires the tiff-codecs feature",
                ));
            }
        }
        TiffCompression::Jpeg => {
            #[cfg(feature = "tiff-codecs")]
            {
                decode_jpeg(raw, p.width, p.height, p.samples_per_pixel)?
            }
            #[cfg(not(feature = "tiff-codecs"))]
            {
                return Err(Error::not_implemented(
                    "TiffImageSource::readTile: Jpeg decoding requires the tiff-codecs feature",
                ));
            }
        }
    };

    let mut out = decoded;
    if p.predictor_horizontal {
        undo_horizontal_differencing(
            &mut out,
            p.width,
            p.samples_per_pixel,
            p.bytes_per_sample,
            p.big_endian,
        )?;
    }
    Ok(out)
}

impl<'a> TiffImageSource<'a> {
    /// Reads **all** tiles of this image and returns their raw pixel bytes in
    /// row-major order, decompressing them in parallel via the shared Rayon
    /// pool (Phase 5 §6.2 point 3: sequential I/O first — read each tile's raw
    /// bytes with the single-cursor reader — then parallel CPU
    /// decompression, one job per tile).
    ///
    /// The I/O phase is sequential because [`BinaryReader`] is a single mutable
    /// cursor (§6.3 — «pro Tile read_at/Offset bekannt»); only the CPU-bound
    /// decompression is parallelized. Each tile is an owned `Vec<u8>` read
    /// independently into its own buffer, so the decode pool races nothing.
    ///
    /// The returned buffers are **byte-identical** to calling
    /// [`ImageSource::read_tile`] for each tile in order — Rayon's ordered
    /// `collect` preserves input order, and (lossless) codecs are pure
    /// functions over an owned buffer. On error the first failing tile (by
    /// input order) is returned.
    ///
    /// # Errors
    ///
    /// A tile read or decode error, or
    /// [`ErrorCode::OutOfRange`] if a tile index is outside the byte-range
    /// table.
    #[cfg(feature = "parallel")]
    pub fn read_all_tiles_parallel(&mut self) -> Result<Vec<Vec<u8>>> {
        use crate::io::backend::tiff::parallel::{decode_all, threshold};

        let layout = self.directory.layout;
        let columns = layout.columns(0);
        let rows = layout.rows(0);
        let mut raw_tiles: Vec<Vec<u8>> = Vec::with_capacity((columns * rows) as usize);

        // Phase 5: sequential I/O — read every raw tile with the single cursor.
        let file_size = self.reader.size()?;
        let ranges = self.directory.tile_byte_ranges.clone();
        for range in ranges.iter() {
            if range.offset > file_size || range.byte_count > file_size - range.offset {
                return Err(Error::invalid_argument(
                    "TiffImageSource::readTile: tile byte range exceeds underlying reader size",
                ));
            }
            let raw_len = range.byte_count as usize;
            let mut buf = vec![0u8; raw_len];
            self.reader.seek(range.offset)?;
            let n = self.reader.read(&mut buf)?;
            if n != raw_len {
                return Err(Error::invalid_argument(
                    "TiffImageSource::readTile: truncated tile data",
                ));
            }
            raw_tiles.push(buf);
        }
        // We intentionally do not build `index`/`region`/`Tile` here — this is
        // the raw-pixel extraction path; those are re-derived by callers.

        let expected_size = tile_decoded_size(
            layout.tile_size.width,
            layout.tile_size.height,
            self.directory.samples_per_pixel,
            bytes_per_sample(self.directory.pixel_type),
        )?;
        let params = self.decode_snapshot();

        if !threshold::should_parallelize(raw_tiles.len()) {
            return raw_tiles
                .into_iter()
                .map(|raw| decode_tile_bytes(&raw, expected_size, params))
                .collect();
        }

        decode_all(raw_tiles, move |raw: &[u8]| {
            decode_tile_bytes(raw, expected_size, params)
        })
    }
}
