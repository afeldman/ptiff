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
#[cfg(feature = "tiff-codecs")]
use crate::io::backend::tiff::compression::{encode_deflate, encode_jpeg};
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
                #[cfg(feature = "tiff-codecs")]
                {
                    encode_deflate(&payload)?
                }
                #[cfg(not(feature = "tiff-codecs"))]
                {
                    return Err(Error::not_implemented(
                        "TiffImageSink::writeTile: Deflate encoding requires the tiff-codecs feature",
                    ));
                }
            }
            TiffCompression::Jpeg => {
                #[cfg(feature = "tiff-codecs")]
                {
                    encode_jpeg(
                        &payload,
                        self.directory.layout.tile_size.width,
                        self.directory.layout.tile_size.height,
                        self.directory.samples_per_pixel,
                        self.directory.jpeg_quality,
                    )?
                }
                #[cfg(not(feature = "tiff-codecs"))]
                {
                    return Err(Error::not_implemented(
                        "TiffImageSink::writeTile: Jpeg encoding requires the tiff-codecs feature",
                    ));
                }
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
    use tiff::decoder::Decoder;

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

    /// Decodes a TIFF byte stream with image-rs' independent `tiff` crate,
    /// returning the raw sample bytes in row-major order. Serves as an
    /// independent (data-level) interop oracle for what we write.
    fn tiff_oracle_decode(file: &[u8]) -> (u32, u32, Vec<u8>) {
        use std::io::Cursor;
        let mut dec = Decoder::new(Cursor::new(file)).expect("tiff crate opens stream");
        let (w, h) = dec.dimensions().expect("tiff crate reads dimensions");
        let img = dec.read_image().expect("tiff crate decodes image");
        let bytes = match img {
            tiff::decoder::DecodingResult::U8(v) => v,
            other => panic!("tiff oracle: expected U8 samples, got {other:?}"),
        };
        (w, h, bytes)
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

    #[test]
    fn e2e_deflate_round_trip() {
        let width = 16u32;
        let height = 16u32;
        let payload: Vec<u8> = (0..(width * height) as usize)
            .map(|i| (i % 251) as u8)
            .collect();
        let model = strip_model(width, height, "Deflate", "None");
        let file = write_file(&model, &payload);
        let decoded = read_pixel(&file);
        assert_eq!(decoded, payload);
    }

    #[test]
    fn e2e_jpeg_round_trip() {
        let width = 16u32;
        let height = 16u32;
        // Greyscale ramp; JPEG is lossy so assert approximate equality rather
        // than exact -- low-frequency data survives 4:4:4 with modest error.
        let payload: Vec<u8> = (0..(width * height) as usize)
            .map(|i| ((i as u32 % width) * 16) as u8)
            .collect();
        let model = strip_model(width, height, "Jpeg", "None");
        let file = write_file(&model, &payload);
        let decoded = read_pixel(&file);
        assert_eq!(decoded.len(), payload.len());
        for (a, b) in decoded.iter().zip(payload.iter()) {
            let diff = i32::from(*a) - i32::from(*b);
            assert!(diff.abs() <= 30, "JPEG lossy sample off by {diff}");
        }
    }

    /// Interop: uncompressed grayscale strips written by our backend are read
    /// back correctly by image-rs' `tiff` crate (independent oracle).
    #[test]
    fn interop_tiff_crate_uncompressed_gray() {
        let width = 32u32;
        let height = 16u32;
        let payload: Vec<u8> = (0..(width * height) as usize)
            .map(|i| (i % 251) as u8)
            .collect();
        let model = strip_model(width, height, "None", "None");
        let file = write_file(&model, &payload);
        let (w, h, samples) = tiff_oracle_decode(&file);
        assert_eq!((w, h), (width, height));
        assert_eq!(samples, payload);
    }

    /// Interop: uncompressed RGB strips (Photometric=RGB, sPP=3) via oracle.
    #[test]
    fn interop_tiff_crate_uncompressed_rgb() {
        let width = 16u32;
        let height = 8u32;
        let mut model = strip_model(width, height, "None", "None");
        model.set_field("samplesPerPixel", "3");
        // RGB ramp so channels are distinct and order-verifiable.
        let payload: Vec<u8> = (0..(width * height * 3) as usize)
            .map(|i| ((i % 3) * 80 + (i / 3) % 4 * 40) as u8)
            .collect();
        let file = write_file(&model, &payload);
        let (w, h, samples) = tiff_oracle_decode(&file);
        assert_eq!((w, h), (width, height));
        assert_eq!(samples.len(), (width * height * 3) as usize);
        assert_eq!(samples, payload);
    }

    /// Interop: Deflate strips via oracle (tiff crate `deflate` feature).
    #[test]
    fn interop_tiff_crate_deflate() {
        let width = 32u32;
        let height = 16u32;
        let payload: Vec<u8> = (0..(width * height) as usize)
            .map(|i| (i % 251) as u8)
            .collect();
        let model = strip_model(width, height, "Deflate", "None");
        let file = write_file(&model, &payload);
        let (w, h, samples) = tiff_oracle_decode(&file);
        assert_eq!((w, h), (width, height));
        assert_eq!(samples, payload);
    }

    /// Interop: LZW strips via oracle (tiff crate `lzw` feature).
    #[test]
    fn interop_tiff_crate_lzw() {
        let width = 32u32;
        let height = 16u32;
        let payload: Vec<u8> = (0..(width * height) as usize)
            .map(|i| (i % 251) as u8)
            .collect();
        let model = strip_model(width, height, "LZW", "None");
        let file = write_file(&model, &payload);
        let (w, h, samples) = tiff_oracle_decode(&file);
        assert_eq!((w, h), (width, height));
        assert_eq!(samples, payload);
    }

    /// Interop: BigTIFF container (header magic 43, 8-byte offsets) written by
    /// our backend is read back by the independent tiff crate oracle.
    #[test]
    fn interop_tiff_crate_bigtiff_gray() {
        let width = 32u32;
        let height = 16u32;
        let payload: Vec<u8> = (0..(width * height) as usize)
            .map(|i| (i % 251) as u8)
            .collect();
        let mut model = strip_model(width, height, "None", "None");
        model.set_field("container", "BigTiff");
        let file = write_file(&model, &payload);
        let (w, h, samples) = tiff_oracle_decode(&file);
        assert_eq!((w, h), (width, height));
        assert_eq!(samples, payload);
    }

    /// Interop: PackBits strips via oracle (built into tiff crate).
    #[test]
    fn interop_tiff_crate_packbits() {
        let width = 32u32;
        let height = 16u32;
        let payload: Vec<u8> = (0..(width * height) as usize)
            .map(|i| (i % 251) as u8)
            .collect();
        let model = strip_model(width, height, "PackBits", "None");
        let file = write_file(&model, &payload);
        let (w, h, samples) = tiff_oracle_decode(&file);
        assert_eq!((w, h), (width, height));
        assert_eq!(samples, payload);
    }

    /// Interop: JPEG grayscale strips via oracle (tiff crate `jpeg` feature,
    /// zune-jpeg decoder). Lossy, so assert approximate equality like our
    /// own end-to-end JPEG test.
    #[test]
    fn interop_tiff_crate_jpeg_gray() {
        let width = 32u32;
        let height = 32u32;
        let payload: Vec<u8> = (0..(width * height) as usize)
            .map(|i| ((i as u32 % width) * 8) as u8)
            .collect();
        let model = strip_model(width, height, "Jpeg", "None");
        let file = write_file(&model, &payload);
        let (w, h, samples) = tiff_oracle_decode(&file);
        assert_eq!((w, h), (width, height));
        assert_eq!(samples.len(), payload.len());
        for (a, b) in samples.iter().zip(payload.iter()) {
            let diff = i32::from(*a) - i32::from(*b);
            assert!(diff.abs() <= 40, "tiff-oracle JPEG sample off by {diff}");
        }
    }
}
