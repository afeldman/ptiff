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
use crate::io::backend::tiff::compression::{decode_lzw, decode_pack_bits};
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

        let expected_size_step1 = checked_multiply(
            u64::from(self.directory.layout.tile_size.width),
            u64::from(self.directory.layout.tile_size.height),
        )?;
        let expected_size_step2 = checked_multiply(
            expected_size_step1,
            u64::from(self.directory.samples_per_pixel),
        )?;
        let expected_size = checked_multiply(
            expected_size_step2,
            u64::from(bytes_per_sample(self.directory.pixel_type)),
        )? as usize;

        match self.directory.compression {
            TiffCompression::None => {
                if self.raw_buffer.len() != expected_size {
                    return Err(Error::invalid_argument(
                        "TiffImageSource::readTile: uncompressed tile size does not match the expected size",
                    ));
                }
                self.buffer = self.raw_buffer.clone();
            }
            TiffCompression::Lzw => {
                let decoded = decode_lzw(&self.raw_buffer, expected_size)?;
                self.buffer = decoded;
            }
            TiffCompression::PackBits => {
                let decoded = decode_pack_bits(&self.raw_buffer, expected_size)?;
                self.buffer = decoded;
            }
            TiffCompression::Deflate => {
                #[cfg(feature = "tiff-codecs")]
                {
                    let decoded = decode_deflate(&self.raw_buffer, expected_size)?;
                    self.buffer = decoded;
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
                    let decoded = decode_jpeg(
                        &self.raw_buffer,
                        self.directory.layout.tile_size.width,
                        self.directory.layout.tile_size.height,
                        self.directory.samples_per_pixel,
                    )?;
                    self.buffer = decoded;
                }
                #[cfg(not(feature = "tiff-codecs"))]
                {
                    return Err(Error::not_implemented(
                        "TiffImageSource::readTile: Jpeg decoding requires the tiff-codecs feature",
                    ));
                }
            }
        }

        if self.directory.predictor
            == crate::io::backend::tiff::TiffPredictor::HorizontalDifferencing
        {
            let big_endian = self.directory.endian == Endian::Big;
            crate::io::backend::tiff::compression::undo_horizontal_differencing(
                &mut self.buffer,
                self.directory.layout.tile_size.width,
                self.directory.samples_per_pixel,
                bytes_per_sample(self.directory.pixel_type),
                big_endian,
            )?;
        }

        Ok(Tile::new(
            TileId::new(linear_index),
            index,
            region,
            &self.buffer,
        ))
    }
}
