//! [`ImageSink`] over one TiffDirectory's baseline layout.
//!
//! Mirrors `ptiff::io::backend::tiff::tiff_image_sink.{hpp,cpp}`. Writes each
//! tile's raw bytes at the pre-computed file offset; for compressed layouts it
//! applies the predictor, encodes (PackBits/LZW), and back-patches the
//! StripByteCounts in the IFD it was handed. Not thread-safe, same contract as
//! its base trait.

use crate::io::backend::tiff::compression::{
    apply_horizontal_differencing, encode_lzw, encode_pack_bits,
};
use crate::io::backend::tiff::directory::{TiffCompression, TiffDirectory};
use crate::io::backend::tiff::pixel_format::bytes_per_sample;
use crate::io::backend::tiff::{write_u32, Endian};
use crate::io::{BinaryWriter, ImageSink};
use crate::tile::Tile;
use crate::{Error, Result};

/// [`ImageSink`] over one TiffDirectory. Not thread-safe.
pub struct TiffImageSink<'a> {
    writer: &'a mut dyn BinaryWriter,
    directory: TiffDirectory,
}

impl<'a> TiffImageSink<'a> {
    /// Constructs the sink over `writer` and the resolved directory.
    pub fn new(writer: &'a mut dyn BinaryWriter, directory: TiffDirectory) -> Self {
        Self { writer, directory }
    }
}

impl ImageSink for TiffImageSink<'_> {
    fn layout(&self) -> &crate::tile::TileLayout {
        &self.directory.layout
    }

    fn write_tile(&mut self, tile: &Tile<'_>) -> Result<()> {
        let columns = self.directory.layout.columns(tile.index().level);
        let linear_index =
            u64::from(tile.index().row) * u64::from(columns) + u64::from(tile.index().column);
        if linear_index as usize >= self.directory.tile_byte_ranges.len() {
            return Err(Error::out_of_range(
                "TiffImageSink::writeTile: index outside byte range table",
            ));
        }

        let range = self.directory.tile_byte_ranges[linear_index as usize];
        if tile.data().len() as u64 != range.byte_count {
            return Err(Error::invalid_argument(
                "TiffImageSink::writeTile: tile data size doesn't match the strip's expected byte count",
            ));
        }

        if self.directory.compression == TiffCompression::None {
            // Uncompressed path: seek to the strip offset and write raw bytes.
            self.writer.seek(range.offset)?;
            let n = self.writer.write(tile.data())?;
            if n != tile.data().len() {
                return Err(Error::invalid_argument(
                    "TiffImageSink::writeTile: short write",
                ));
            }
            return Ok(());
        }

        // Compressed path: build the byte stream to encode.
        let mut payload = tile.data().to_vec();
        if self.directory.predictor
            == crate::io::backend::tiff::TiffPredictor::HorizontalDifferencing
        {
            let big_endian = self.directory.endian == Endian::Big;
            apply_horizontal_differencing(
                &mut payload,
                self.directory.image_width,
                self.directory.samples_per_pixel,
                bytes_per_sample(self.directory.pixel_type),
                big_endian,
            )?;
        }

        let encoded: Vec<u8> = match self.directory.compression {
            TiffCompression::Lzw => encode_lzw(&payload)?,
            TiffCompression::PackBits => encode_pack_bits(&payload)?,
            TiffCompression::Deflate => {
                return Err(Error::not_implemented(
                    "TiffImageSink::writeTile: Deflate encoding requires the tiff-codecs feature",
                ));
            }
            TiffCompression::Jpeg => {
                return Err(Error::not_implemented(
                    "TiffImageSink::writeTile: Jpeg encoding requires the tiff-codecs feature",
                ));
            }
            TiffCompression::None => unreachable!("None handled above"),
        };
        if encoded.len() as u64 > u32::MAX as u64 {
            return Err(Error::invalid_argument(
                "TiffImageSink::writeTile: compressed strip exceeds uint32 size",
            ));
        }

        // Back-patch StripByteCounts with the compressed byte count.
        let mut count_bytes = [0u8; 4];
        write_u32(&mut count_bytes, encoded.len() as u32, Endian::Little);
        self.writer
            .seek(self.directory.strip_byte_counts_patch_offset)?;
        self.writer.write(&count_bytes)?;

        // Write the compressed strip at the deterministic strip offset.
        self.writer.seek(range.offset)?;
        let n = self.writer.write(&encoded)?;
        if n != encoded.len() {
            return Err(Error::invalid_argument(
                "TiffImageSink::writeTile: short write",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::TileId;
    use crate::io::backend::tiff::directory_writer::plan_tiff_write;
    use crate::io::backend::tiff::header::{read_tiff_header, write_tiff_header};
    use crate::io::backend::tiff::ifd_writer::write_tiff_ifd;
    use crate::io::backend::tiff::{interpret_tiff_ifd, read_tiff_ifd, Endian};
    use crate::io::{ImageSource as _, MemoryBinaryReader, MemoryBinaryWriter, StorageModel};
    use crate::tile::{TileExtent, TileIndex, TileRegion};

    fn strip_model(width: u32, height: u32, compression: &str, predictor: &str) -> StorageModel {
        let mut m = StorageModel::new();
        m.set_field("imageWidth", width.to_string());
        m.set_field("imageHeight", height.to_string());
        m.set_field("samplesPerPixel", "1");
        m.set_field("pixelType", "UInt8");
        m.set_field("compression", compression);
        m.set_field("predictor", predictor);
        m
    }

    fn tile(index: TileIndex, bytes: &[u8]) -> Tile<'_> {
        Tile::new(
            TileId::new(u64::from(index.row) * 4 + u64::from(index.column)),
            index,
            TileRegion::new(0, 0, TileExtent::new(16, 16)),
            bytes,
        )
    }

    /// Serializes a fully-laid-out single-IFD file (header + IFD + pixel data)
    /// and returns the bytes, for both uncompressed and compressed strips.
    fn write_file(model: &StorageModel, payload: &[u8]) -> Vec<u8> {
        let plan = plan_tiff_write(model).unwrap();
        let is_big = plan.is_big_tiff;
        let header_size = if is_big { 16u64 } else { 8u64 };
        let mut w = MemoryBinaryWriter::new();
        write_tiff_header(&mut w, header_size, is_big).unwrap();
        write_tiff_ifd(&mut w, plan.entries.clone(), is_big, 0).unwrap();
        let mut sink = TiffImageSink::new(&mut w, plan.directory.clone());
        // Single-strip layout: tile (0,0) covers the whole image.
        sink.write_tile(&tile(TileIndex::new(0, 0, 0), payload))
            .unwrap();
        w.take_buffer()
    }

    fn read_pixel(file: &[u8]) -> Vec<u8> {
        let mut r = MemoryBinaryReader::from_slice(file);
        let header = read_tiff_header(&mut r).unwrap();
        let ifd = read_tiff_ifd(
            &mut r,
            header.first_ifd_offset,
            Endian::Little,
            header.is_big_tiff,
        )
        .unwrap();
        let directory = interpret_tiff_ifd(&ifd).unwrap();
        let mut source = crate::io::backend::tiff::TiffImageSource::new(&mut r, directory);
        let tile = source.read_tile(TileIndex::new(0, 0, 0)).unwrap();
        tile.data().to_vec()
    }

    #[test]
    fn e2e_uncompressed_round_trip() {
        let width = 16u32;
        let height = 16u32;
        let payload: Vec<u8> = (0..(width * height) as usize)
            .map(|i| (i % 251) as u8)
            .collect();
        let model = strip_model(width, height, "None", "None");
        let file = write_file(&model, &payload);
        let decoded = read_pixel(&file);
        assert_eq!(decoded, payload);
    }

    #[test]
    fn e2e_lzw_with_predictor_round_trip() {
        let width = 16u32;
        let height = 16u32;
        // Create data with horizontal runs so LZW + differencing both matter.
        let payload: Vec<u8> = (0..(width * height) as usize)
            .map(|i| (((i as u32) / width) % 32) as u8)
            .collect();
        let model = strip_model(width, height, "LZW", "HorizontalDifferencing");
        let file = write_file(&model, &payload);
        let decoded = read_pixel(&file);
        assert_eq!(decoded, payload);
    }

    #[test]
    fn e2e_packbits_round_trip() {
        let width = 16u32;
        let height = 16u32;
        // Highly repetitive data exercises PackBits.
        let payload = vec![0xAAu8; (width * height) as usize];
        let model = strip_model(width, height, "PackBits", "None");
        let file = write_file(&model, &payload);
        let decoded = read_pixel(&file);
        assert_eq!(decoded, payload);
    }
}
