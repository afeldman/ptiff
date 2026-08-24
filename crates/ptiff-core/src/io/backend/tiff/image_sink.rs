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
    /// True when this image is both tiled and compressed. Compressed tiles
    /// have variable sizes, so they are packed back-to-back from the first
    /// tile's offset and each tile's actual offset/byte-count is back-patched
    /// into the IFD's TileOffsets/TileByteCounts arrays.
    compressed_tiled: bool,
    /// Running write cursor for the compressed-tiled path: the next tile's
    /// file offset. Initialized to the first tile's planned offset.
    next_write_offset: u64,
    /// Expected linear (row-major) index of the next tile written by the
    /// compressed-tiled path, which requires descending order.
    next_linear_index: usize,
}

impl<'a> TiffImageSink<'a> {
    /// Constructs the sink over `writer` and the resolved directory.
    pub fn new(writer: &'a mut dyn BinaryWriter, directory: TiffDirectory) -> Self {
        let compressed_tiled = directory.compression != TiffCompression::None
            && !directory.tile_offsets_value_addresses.is_empty();
        let next_write_offset = directory
            .tile_byte_ranges
            .first()
            .map(|r| r.offset)
            .unwrap_or(0);
        Self {
            writer,
            directory,
            compressed_tiled,
            next_write_offset,
            next_linear_index: 0,
        }
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

        if self.compressed_tiled {
            return self.write_compressed_tile(linear_index as usize, tile);
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

        // Compressed-strip path: encode, patch the single StripByteCounts, and
        // write at the deterministic strip offset.
        let encoded = self.encode_tile(tile)?;
        if encoded.len() as u64 > u32::MAX as u64 {
            return Err(Error::invalid_argument(
                "TiffImageSink::writeTile: compressed strip exceeds uint32 size",
            ));
        }
        let mut count_bytes = [0u8; 4];
        write_u32(&mut count_bytes, encoded.len() as u32, Endian::Little);
        self.writer
            .seek(self.directory.strip_byte_counts_patch_offset)?;
        self.writer.write(&count_bytes)?;

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

impl<'a> TiffImageSink<'a> {
    /// Encodes `tile`'s pixel bytes through the image's compression scheme
    /// (applying horizontal differencing first when configured). Shared by the
    /// compressed-strip and compressed-tiled write paths.
    ///
    /// A thin adapter over the free [`encode_tile_bytes`] so the same logic is
    /// reusable (as an owned, `Send + Sync` closure in parallel batches).
    fn encode_tile(&self, tile: &Tile<'_>) -> Result<Vec<u8>> {
        encode_tile_bytes(tile.data(), self.directory_layout_snapshot())
    }

    /// Snapshot of the per-tile `Copy` parameters needed to encode a tile —
    /// the owned, thread-safe projection the parallel path captures once.
    fn directory_layout_snapshot(&self) -> TileEncodeParams {
        TileEncodeParams {
            width: self.directory.layout.tile_size.width,
            height: self.directory.layout.tile_size.height,
            samples_per_pixel: self.directory.samples_per_pixel,
            bytes_per_sample: bytes_per_sample(self.directory.pixel_type),
            predictor_horizontal: self.directory.predictor
                == crate::io::backend::tiff::TiffPredictor::HorizontalDifferencing,
            big_endian: self.directory.endian == Endian::Big,
            compression: self.directory.compression,
            jpeg_quality: self.directory.jpeg_quality,
        }
    }

    /// Writes one tile of a tiled-and-compressed image: validates write order,
    /// encodes, packs the tile back-to-back at the running cursor, then
    /// back-patches the tile's actual offset/byte-count into the IFD.
    ///
    /// Single-tile entry point (sequential). Use
    /// [`Self::write_compressed_tiles_parallel`] to parallelize the CPU-bound
    /// encoding across a whole tile set while keeping the write sequential.
    fn write_compressed_tile(&mut self, linear_index: usize, tile: &Tile<'_>) -> Result<()> {
        let encoded = self.encode_tile(tile)?;
        self.write_pre_encoded_compressed_tile(linear_index, &encoded)
    }

    /// Writes an already-encoded (compressed) tile: validates descending order,
    /// packs the bytes back-to-back at the running cursor, then back-patches
    /// the tile's actual offset/byte-count into the IFD.
    ///
    /// This is the sequential write half of the Phase 5 pipeline (§6.2): the
    /// *encoding* (CPU-bound) can be done in parallel via Rayon, but the
    /// on-disk byte layout must be written in row-major order so the running
    /// cursor and back-patches stay deterministic.
    fn write_pre_encoded_compressed_tile(
        &mut self,
        linear_index: usize,
        encoded: &[u8],
    ) -> Result<()> {
        if linear_index != self.next_linear_index {
            return Err(Error::invalid_argument(format!(
                "TiffImageSink::writeTile: tiled+compressed requires tiles in row-major order; expected tile {}, got {linear_index}",
                self.next_linear_index
            )));
        }
        if encoded.len() as u64 > u32::MAX as u64 {
            return Err(Error::invalid_argument(
                "TiffImageSink::writeTile: compressed tile exceeds uint32 size",
            ));
        }
        let tile_count = self.directory.tile_offsets_value_addresses.len();
        if linear_index >= tile_count {
            return Err(Error::out_of_range(
                "TiffImageSink::writeTile: tile index outside TileOffsets table",
            ));
        }
        // Write the encoded tile at the running cursor, then advance it.
        self.writer.seek(self.next_write_offset)?;
        let n = self.writer.write(encoded)?;
        if n != encoded.len() {
            return Err(Error::invalid_argument(
                "TiffImageSink::writeTile: short write",
            ));
        }
        let offset = self.next_write_offset;
        let byte_count = encoded.len() as u32;
        self.next_write_offset = self
            .next_write_offset
            .checked_add(encoded.len() as u64)
            .ok_or_else(|| {
                Error::invalid_argument("TiffImageSink::writeTile: tile layout overflows")
            })?;

        // Back-patch TileOffsets and TileByteCounts for this tile.
        let mut offset_bytes = [0u8; 4];
        write_u32(&mut offset_bytes, offset as u32, Endian::Little);
        let offset_slot = self.directory.tile_offsets_value_addresses[linear_index];
        self.writer.seek(offset_slot)?;
        self.writer.write(&offset_bytes)?;

        let mut count_bytes = [0u8; 4];
        write_u32(&mut count_bytes, byte_count, Endian::Little);
        let count_slot = self.directory.tile_byte_counts_value_addresses[linear_index];
        self.writer.seek(count_slot)?;
        self.writer.write(&count_bytes)?;

        self.writer.seek(self.next_write_offset)?;
        self.next_linear_index += 1;
        Ok(())
    }
}

/// Owned, `Copy` projection of the TiffLayout/directory parameters a single
/// tile's encoder needs. Captured once (per write) and shared immutably across
/// a Rayon pool — the parallel path never touches a `&mut` TiffImageSink or its
/// `TiffDirectory` while workers run.
#[derive(Debug, Clone, Copy)]
struct TileEncodeParams {
    width: u32,
    height: u32,
    samples_per_pixel: u32,
    bytes_per_sample: u8,
    predictor_horizontal: bool,
    big_endian: bool,
    compression: TiffCompression,
    jpeg_quality: u32,
}

/// Encodes one tile's raw pixel bytes through the image's compression scheme,
/// applying horizontal differencing first when the predictor requires it.
///
/// This is the **owned, `Send + Sync`** form of the encoder: it takes a raw
/// `&[u8]` view into one tile buffer plus a [`TileEncodeParams`] snapshot, and
/// returns the compressed bytes. Being a pure function over its inputs, it is
/// safe to invoke from multiple Rayon workers concurrently (Phase 5 §6.3: «one
/// job per tile, no shared &mut»).
///
/// # Errors
///
/// Propagates codec failures; returns [`ErrorCode::NotImplemented`] when a
/// configured codec is disabled at compile time.
fn encode_tile_bytes(raw: &[u8], p: TileEncodeParams) -> Result<Vec<u8>> {
    let mut payload = raw.to_vec();
    if p.predictor_horizontal {
        apply_horizontal_differencing(
            &mut payload,
            p.width,
            p.samples_per_pixel,
            p.bytes_per_sample,
            p.big_endian,
        )?;
    }

    match p.compression {
        TiffCompression::Lzw => encode_lzw(&payload),
        TiffCompression::PackBits => encode_pack_bits(&payload),
        TiffCompression::Deflate => {
            #[cfg(feature = "tiff-codecs")]
            {
                encode_deflate(&payload)
            }
            #[cfg(not(feature = "tiff-codecs"))]
            {
                Err(Error::not_implemented(
                    "TiffImageSink::writeTile: Deflate encoding requires the tiff-codecs feature",
                ))
            }
        }
        TiffCompression::Jpeg => {
            #[cfg(feature = "tiff-codecs")]
            {
                encode_jpeg(
                    &payload,
                    p.width,
                    p.height,
                    p.samples_per_pixel,
                    p.jpeg_quality,
                )
            }
            #[cfg(not(feature = "tiff-codecs"))]
            {
                Err(Error::not_implemented(
                    "TiffImageSink::writeTile: Jpeg encoding requires the tiff-codecs feature",
                ))
            }
        }
        TiffCompression::None => unreachable!("None is handled by the caller"),
    }
}

impl<'a> TiffImageSink<'a> {
    /// Parallel write path for a whole compressed tile set (Phase 5 §6.2
    /// point 4): the CPU-bound **encoding** of every tile is handed to the
    /// shared Rayon pool — one job per tile, each an owned `Vec<u8>` — while
    /// the actual on-disk write stays sequential in row-major order so the
    /// running cursor and IFD back-patches remain deterministic.
    ///
    /// `raw_tiles` must be given **in row-major order** (the same order the
    /// sequential [`write_tile`](crate::io::ImageSink::write_tile) API expects);
    /// the output byte layout is guaranteed identical to writing each tile
    /// sequentially (lossless codecs are pure functions).
    ///
    /// Only the `compressed_tiled` layout takes this path; for non-tiled or
    /// uncompressed layouts parallelizing encoding is unnecessary (no CPU
    /// codec work), so this returns without doing anything — callers can guard
    /// on the same flag but the method is a safe no-op regardless.
    ///
    /// # Errors
    ///
    /// On the first failing tile (by input order) the pool short-circuits; the
    /// sink state is left at the index written before the failure.
    #[cfg(feature = "parallel")]
    pub fn write_compressed_tiles_parallel(&mut self, raw_tiles: &[Vec<u8>]) -> Result<()> {
        use crate::io::backend::tiff::parallel::{encode_all, threshold};

        if !self.compressed_tiled {
            // Nothing CPU-bound to parallelize here.
            return Ok(());
        }

        let params = self.directory_layout_snapshot();

        if !threshold::should_parallelize(raw_tiles.len()) {
            // Small tile sets: reuse the existing sequential write path, which
            // is byte-identical and avoids pool startup overhead.
            return self.write_compressed_tiles_sequential(raw_tiles, &params);
        }

        // Phase 5: parallel encode, ordered collect (deterministic), then
        // sequential write.
        let encoded = encode_all(
            raw_tiles.iter().map(Vec::clone).collect::<Vec<Vec<u8>>>(),
            move |raw: &[u8]| encode_tile_bytes(raw, params),
        )?;

        for (idx, bytes) in encoded.iter().enumerate() {
            self.write_pre_encoded_compressed_tile(idx, bytes)?;
        }
        Ok(())
    }

    /// Sequential fallback for [`Self::write_compressed_tiles_parallel`] when
    /// the tile count is below the parallelization threshold — performs the
    /// same encoding via the shared `encode_tile_bytes` and writes in order.
    fn write_compressed_tiles_sequential(
        &mut self,
        raw_tiles: &[Vec<u8>],
        params: &TileEncodeParams,
    ) -> Result<()> {
        for (idx, raw) in raw_tiles.iter().enumerate() {
            let encoded = encode_tile_bytes(raw, *params)?;
            self.write_pre_encoded_compressed_tile(idx, &encoded)?;
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

    /// Builds a 16x16-tiled grayscale model.
    fn tiled_model(width: u32, height: u32, compression: &str, predictor: &str) -> StorageModel {
        let mut m = StorageModel::new();
        m.set_field("imageWidth", width.to_string());
        m.set_field("imageHeight", height.to_string());
        m.set_field("samplesPerPixel", "1");
        m.set_field("pixelType", "UInt8");
        m.set_field("tileWidth", "16");
        m.set_field("tileHeight", "16");
        m.set_field("compression", compression);
        m.set_field("predictor", predictor);
        m
    }

    /// Serializes a tiled single-IFD file, writing each 16x16 tile in row-major
    /// order through the sink. Returns the bytes.
    fn write_tiled_file(model: &StorageModel, payload: &[u8]) -> Vec<u8> {
        let plan = plan_tiff_write(model).unwrap();
        let is_big = plan.is_big_tiff;
        let header_size = if is_big { 16u64 } else { 8u64 };
        let mut w = MemoryBinaryWriter::new();
        write_tiff_header(&mut w, header_size, is_big).unwrap();
        write_tiff_ifd(&mut w, plan.entries.clone(), is_big, 0).unwrap();

        let layout = plan.directory.layout;
        let cols = layout.columns(0);
        let rows = layout.rows(0);
        let tile_bytes = (layout.tile_size.width as u64
            * layout.tile_size.height as u64
            * plan.directory.samples_per_pixel as u64
            * u64::from(bytes_per_sample(plan.directory.pixel_type)))
            as usize;
        let mut sink = TiffImageSink::new(&mut w, plan.directory.clone());
        for row in 0..rows {
            for col in 0..cols {
                let linear = (row * cols + col) as usize;
                let start = linear * tile_bytes as usize;
                let end = start + tile_bytes as usize;
                let tile = crate::tile::Tile::new(
                    TileId::new(linear as u64),
                    TileIndex::new(col, row, 0),
                    TileRegion::new(0, 0, TileExtent::new(16, 16)),
                    &payload[start..end],
                );
                sink.write_tile(&tile).unwrap();
            }
        }
        w.take_buffer()
    }

    /// Reads every tile through our own image source, concatenated row-major.
    fn read_tile_grid(file: &[u8], level: u32) -> Vec<u8> {
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
        let layout = *source.layout();
        let cols = layout.columns(level);
        let rows = layout.rows(level);
        let mut out = Vec::new();
        for row in 0..rows {
            for col in 0..cols {
                let t = source.read_tile(TileIndex::new(col, row, level)).unwrap();
                out.extend_from_slice(t.data());
            }
        }
        out
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

    /// Six-band multispectral round trip (Mercury spectrographic-camera use
    /// case, samplesPerPixel > 3 -- not RGB, so ExtraSamples must be emitted
    /// for the non-baseline bands and the reader must accept spp up to 512).
    #[test]
    fn e2e_six_band_multispectral_round_trip() {
        let width = 4u32;
        let height = 4u32;
        let samples_per_pixel = 6u32;
        let mut model = strip_model(width, height, "None", "None");
        model.set_field("samplesPerPixel", samples_per_pixel.to_string());
        let payload: Vec<u8> = (0..(width * height * samples_per_pixel) as usize)
            .map(|i| (i % 251) as u8)
            .collect();

        let file = write_file(&model, &payload);
        let decoded = read_pixel(&file);
        assert_eq!(decoded, payload);

        let mut r = MemoryBinaryReader::from_slice(&file);
        let header = read_tiff_header(&mut r).unwrap();
        let ifd = read_tiff_ifd(
            &mut r,
            header.first_ifd_offset,
            Endian::Little,
            header.is_big_tiff,
        )
        .unwrap();
        let directory = interpret_tiff_ifd(&ifd).unwrap();
        assert_eq!(directory.samples_per_pixel, samples_per_pixel);
        // BlackIsZero baseline is 1 sample; the remaining 5 are unspecified
        // extra (scientific/spectral) bands per TIFF 6.0 ExtraSamples = 0.
        let extra = ifd
            .tag(crate::io::backend::tiff::TagId::ExtraSamples.as_u16())
            .unwrap()
            .to_vec();
        assert_eq!(extra, vec![0u64; 5]);
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

    /// End-to-end tiled + LZW + horizontal differencing. Multiple tiles force
    /// the running-cursor layout and per-tile offset/byte-count patches.
    #[test]
    fn e2e_tiled_lzw_with_predictor_round_trip() {
        let width = 32u32;
        let height = 48u32; // 2 cols x 3 rows of 16x16 tiles
        let payload: Vec<u8> = (0..(width * height) as usize)
            .map(|i| (((i as u32) / width) % 17) as u8)
            .collect();
        let model = tiled_model(width, height, "LZW", "HorizontalDifferencing");
        let file = write_tiled_file(&model, &payload);
        let decoded = read_tile_grid(&file, 0);
        assert_eq!(decoded, payload);
    }

    /// End-to-end tiled + Deflate.
    #[test]
    fn e2e_tiled_deflate_round_trip() {
        let width = 48u32;
        let height = 32u32; // 3 cols x 2 rows
        let payload: Vec<u8> = (0..(width * height) as usize)
            .map(|i| (i % 251) as u8)
            .collect();
        let model = tiled_model(width, height, "Deflate", "None");
        let file = write_tiled_file(&model, &payload);
        let decoded = read_tile_grid(&file, 0);
        assert_eq!(decoded, payload);
    }

    /// End-to-end tiled + PackBits (repetitive data).
    #[test]
    fn e2e_tiled_packbits_round_trip() {
        let width = 32u32;
        let height = 32u32; // 2 x 2 tiles
        let payload: Vec<u8> = (0..(width * height) as usize)
            .map(|i| if (i % 64) < 32 { 0xAA } else { 0x55 })
            .collect();
        let model = tiled_model(width, height, "PackBits", "None");
        let file = write_tiled_file(&model, &payload);
        let decoded = read_tile_grid(&file, 0);
        assert_eq!(decoded, payload);
    }

    /// End-to-end tiled + JPEG is lossy; assert per-sample tolerance.
    #[test]
    fn e2e_tiled_jpeg_tolerance_round_trip() {
        let width = 32u32;
        let height = 48u32; // 2 x 3 tiles
        let payload: Vec<u8> = (0..(width * height) as usize)
            .map(|i| ((i as u32 % width) * 8) as u8)
            .collect();
        let model = tiled_model(width, height, "Jpeg", "None");
        let file = write_tiled_file(&model, &payload);
        let decoded = read_tile_grid(&file, 0);
        assert_eq!(decoded.len(), payload.len());
        for (a, b) in decoded.iter().zip(payload.iter()) {
            let diff = i32::from(*a) - i32::from(*b);
            assert!(diff.abs() <= 40, "tiled JPEG sample off by {diff}");
        }
    }

    /// Tiled + compressed requires tiles in ascending row-major order; writing
    /// out of order must be rejected instead of silently corrupting offsets.
    #[test]
    fn tiled_compressed_rejects_out_of_order_tiles() {
        let width = 32u32;
        let height = 32u32; // 2 x 2 tiles
        let plan = plan_tiff_write(&tiled_model(width, height, "LZW", "None")).unwrap();
        let mut w = MemoryBinaryWriter::new();
        write_tiff_header(&mut w, 8, false).unwrap();
        write_tiff_ifd(&mut w, plan.entries.clone(), false, 0).unwrap();
        let mut sink = TiffImageSink::new(&mut w, plan.directory.clone());
        let bytes = vec![0u8; 16 * 16];
        // Write tile (1,0) first -> linear index 1 while next expected is 0.
        let tile = tile(TileIndex::new(1, 0, 0), &bytes);
        assert!(sink.write_tile(&tile).is_err());
    }

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
