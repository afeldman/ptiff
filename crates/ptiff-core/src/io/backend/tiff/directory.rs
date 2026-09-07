//! The resolved directory TiffBackend needs from one IFD, plus interpretation
//! of a parsed IFD against the supported baseline subset.
//!
//! Mirrors `ptiff::io::backend::tiff::tiff_directory.{hpp,cpp}`.

use crate::io::backend::tiff::checked_arithmetic::K_MAX_TAG_COUNT;
use crate::io::backend::tiff::ifd::TiffIfd;
use crate::io::backend::tiff::pixel_format::{
    pixel_type_field_value, require_uniform_bits_per_sample, resolve_pixel_type,
};
use crate::io::backend::tiff::ptiff_metadata::decode_metadata_payload;
use crate::io::backend::tiff::tag::TagId;
use crate::io::backend::tiff::Endian;
use crate::io::StorageModel;
use crate::tile::{TileExtent, TileLayout};
use crate::{Error, PixelType, Result};

/// One strip or tile's pixel-data byte range, in row-major order (matches
/// [`TileLayout`]'s column/row iteration: row-major, column fastest).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileByteRange {
    /// Absolute file offset of the strip/tile's pixel data.
    pub offset: u64,
    /// Number of bytes in the strip/tile.
    pub byte_count: u64,
}

impl TileByteRange {
    /// Builds a new byte range.
    #[must_use]
    pub const fn new(offset: u64, byte_count: u64) -> Self {
        Self { offset, byte_count }
    }
}

/// Compression scheme this backend can decode (TIFF tag 259). Widen additively
/// as new schemes are supported -- never reuse or renumber existing values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TiffCompression {
    /// Uncompressed (tag 1).
    None,
    /// TIFF LZW (tag 5).
    Lzw,
    /// TIFF PackBits (tag 32773).
    PackBits,
    /// zlib-wrapped Deflate (tags 8 / 32946).
    Deflate,
    /// Baseline/new-style JPEG (tag 7).
    Jpeg,
}

/// Predictor applied before compression (TIFF tag 317). Only horizontal
/// differencing (2) is supported; floating-point predictor (3) is rejected by
/// [`interpret_tiff_ifd`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TiffPredictor {
    /// No predictor (tag 1).
    None,
    /// Horizontal differencing (tag 2).
    HorizontalDifferencing,
}

/// Everything TiffBackend needs from one IFD, resolved against this backend's
/// supported baseline subset: image dimensions/pixel format, the derived
/// [`TileLayout`] (strips modeled as `RowsPerStrip`-tall tiles spanning the
/// image width), and each tile/strip's byte range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TiffDirectory {
    /// Image width in pixels.
    pub image_width: u32,
    /// Image height in pixels.
    pub image_height: u32,
    /// Resolved pixel sample type.
    pub pixel_type: PixelType,
    /// Samples per pixel (1 gray, 2..512 multispectral, 3 RGB).
    pub samples_per_pixel: u32,
    /// The derived tiling grid (strips modeled as tiles).
    pub layout: TileLayout,
    /// One byte range per strip/tile, row-major.
    pub tile_byte_ranges: Vec<TileByteRange>,
    /// The image's compression scheme.
    pub compression: TiffCompression,
    /// The image's predictor.
    pub predictor: TiffPredictor,
    /// Byte order the file declares.
    pub endian: Endian,
    /// Width, in bytes, of offset/byte-count value slots this directory's IFD
    /// uses (4 = classic LONG, 8 = BigTIFF LONG8). Meaningful only on the
    /// write side: the Sink back-patches TileOffsets/TileByteCounts/
    /// StripByteCounts at these widths after encode. Set by
    /// [`crate::io::backend::tiff::directory_writer::plan_tiff_write`].
    pub offset_value_bytes: u8,
    /// Absolute file offset of the StripByteCounts value area inside the IFD.
    /// Meaningful only when `compression != None`: the Sink patches the
    /// compressed byte count here after encode.
    pub strip_byte_counts_patch_offset: u64,
    /// Absolute file offset, one per tile in row-major order, of the tile's
    /// value slot inside the TileOffsets value array. Populated only for tiled
    /// layouts; the Sink back-patches each tile's actual (compressed) offset
    /// here after writing it.
    pub tile_offsets_value_addresses: Vec<u64>,
    /// Absolute file offset, one per tile in row-major order, of the tile's
    /// value slot inside the TileByteCounts value array. Populated only for
    /// tiled layouts; the Sink back-patches each tile's actual (compressed)
    /// byte count here after writing it.
    pub tile_byte_counts_value_addresses: Vec<u64>,
    /// libjpeg-turbo quality parameter, [0, 100]. Meaningful only when
    /// `compression == Jpeg`.
    pub jpeg_quality: u32,
    /// PTIFF extension metadata (RFC-7002) read from private tags 65001-65005,
    /// flattened into StorageModel-style fields with the `ptiff.<domain>.<key>`
    /// convention. Empty when the IFD carries no PTIFF extension tags.
    pub ptiff_fields: std::collections::BTreeMap<String, String>,
}

impl Default for TiffDirectory {
    fn default() -> Self {
        Self {
            image_width: 0,
            image_height: 0,
            pixel_type: PixelType::UInt8,
            samples_per_pixel: 1,
            layout: TileLayout::default(),
            tile_byte_ranges: Vec::new(),
            compression: TiffCompression::None,
            predictor: TiffPredictor::None,
            endian: Endian::Little,
            offset_value_bytes: 4,
            strip_byte_counts_patch_offset: 0,
            tile_offsets_value_addresses: Vec::new(),
            tile_byte_counts_value_addresses: Vec::new(),
            jpeg_quality: 90,
            ptiff_fields: std::collections::BTreeMap::new(),
        }
    }
}

// Maps each PTIFF extension tag to the StorageModel field prefix under which
// its decoded records are stored (`ptiff.<prefix>.<key>`), matching what the
// write path emits.
fn ptiff_prefix_for_tag(tag_id: u16) -> &'static str {
    match tag_id {
        65001 => "spice",
        65002 => "camera",
        65003 => "crs",
        65004 => "layers",
        65005 => "provenance",
        _ => "",
    }
}

// Decodes the value archive of one PTIFF extension tag into
// `directory.ptiff_fields`. A payload that is absent or does not decode as a
// PTIFF extension payload is silently skipped.
fn decode_ptiff_tag(ifd: &TiffIfd, tag_id: u16, directory: &mut TiffDirectory) {
    let Ok(values) = ifd.tag(tag_id) else {
        return;
    };
    let Ok(records) = decode_metadata_payload(values) else {
        return;
    };
    let prefix = ptiff_prefix_for_tag(tag_id);
    if prefix.is_empty() {
        return;
    }
    let pfx = format!("ptiff.{prefix}.");
    for rec in records {
        directory
            .ptiff_fields
            .insert(format!("{pfx}{}", rec.key), rec.value);
    }
}

fn compression_field_value(compression: TiffCompression) -> &'static str {
    match compression {
        TiffCompression::None => "None",
        TiffCompression::Lzw => "Lzw",
        TiffCompression::PackBits => "PackBits",
        TiffCompression::Deflate => "Deflate",
        TiffCompression::Jpeg => "Jpeg",
    }
}

fn predictor_field_value(predictor: TiffPredictor) -> &'static str {
    match predictor {
        TiffPredictor::None => "None",
        TiffPredictor::HorizontalDifferencing => "HorizontalDifferencing",
    }
}

/// A required tag that is absent is reported as `InvalidArgument` (NotFound is
/// reserved for genuinely optional lookups), matching the documented contract.
fn require_single_value(ifd: &TiffIfd, id: TagId) -> Result<u64> {
    match ifd.single_value(id.as_u16()) {
        Ok(v) => Ok(v),
        Err(e) => {
            if e.code() == crate::ErrorCode::NotFound {
                Err(Error::invalid_argument(
                    "interpretTiffIfd: required tag is missing",
                ))
            } else {
                Err(e)
            }
        }
    }
}

fn require_tag(ifd: &TiffIfd, id: TagId) -> Result<Vec<u64>> {
    match ifd.tag(id.as_u16()) {
        Ok(v) => Ok(v.to_vec()),
        Err(e) => {
            if e.code() == crate::ErrorCode::NotFound {
                Err(Error::invalid_argument(
                    "interpretTiffIfd: required tag is missing",
                ))
            } else {
                Err(e)
            }
        }
    }
}

/// Builds one byte range per strip/tile. Offsets and byte counts must have
/// matched lengths -- a malformed TIFF with mismatched arrays is rejected
/// rather than silently truncated. A claim of more entries than a single tag
/// array can legally hold (RFC-0001 §13 resource guard) is rejected up front.
fn build_byte_ranges(offsets: &[u64], byte_counts: &[u64]) -> Result<Vec<TileByteRange>> {
    if offsets.len() != byte_counts.len() {
        return Err(Error::invalid_argument(
            "interpretTiffIfd: offsets and byteCounts arrays have mismatched lengths",
        ));
    }
    if offsets.len() as u64 > K_MAX_TAG_COUNT {
        return Err(Error::invalid_argument(
            "interpretTiffIfd: strip/tile table exceeds a safe element bound",
        ));
    }
    Ok(offsets
        .iter()
        .zip(byte_counts.iter())
        .map(|(&offset, &byte_count)| TileByteRange::new(offset, byte_count))
        .collect())
}

/// SamplesPerPixel, defaulting to 1. Grayscale (1), RGB (3), and arbitrary
/// multispectral band counts (2..512) are supported.
fn resolve_samples_per_pixel(ifd: &TiffIfd) -> Result<u64> {
    let spp = ifd.single_value_or(TagId::SamplesPerPixel.as_u16(), 1)?;
    if !(1..=512).contains(&spp) {
        return Err(Error::invalid_argument(
            "interpretTiffIfd: unsupported SamplesPerPixel",
        ));
    }
    Ok(spp)
}

/// Compression, defaulting to 1 (uncompressed). Supported: 1 (None), 5 (LZW),
/// 32773 (PackBits), 8 (Deflate) and 32946 (legacy old-style Deflate -- both
/// resolve to the same zlib-wrapped stream format), and 7 (new-style JPEG).
fn resolve_compression(ifd: &TiffIfd) -> Result<TiffCompression> {
    let compression = ifd.single_value_or(TagId::Compression.as_u16(), 1)?;
    match compression {
        1 => Ok(TiffCompression::None),
        5 => Ok(TiffCompression::Lzw),
        32773 => Ok(TiffCompression::PackBits),
        8 | 32946 => Ok(TiffCompression::Deflate),
        7 => Ok(TiffCompression::Jpeg),
        other => Err(Error::invalid_argument(format!(
            "interpretTiffIfd: unsupported Compression value {other}"
        ))),
    }
}

/// Predictor, defaulting to 1 (none). Supported: 1 (None), 2 (horizontal
/// differencing). 2 is rejected when paired with a float32 pixelType or Jpeg
/// compression -- both undefined by the TIFF spec.
fn resolve_predictor(
    ifd: &TiffIfd,
    pixel_type: PixelType,
    compression: TiffCompression,
) -> Result<TiffPredictor> {
    let predictor = ifd.single_value_or(TagId::Predictor.as_u16(), 1)?;
    match predictor {
        1 => Ok(TiffPredictor::None),
        2 => {
            if pixel_type == PixelType::Float32 {
                return Err(Error::invalid_argument(
                    "interpretTiffIfd: Predictor=2 is not defined for float32 samples",
                ));
            }
            if compression == TiffCompression::Jpeg {
                return Err(Error::invalid_argument(
                    "interpretTiffIfd: Predictor=2 is not defined for Jpeg compression",
                ));
            }
            Ok(TiffPredictor::HorizontalDifferencing)
        }
        other => Err(Error::invalid_argument(format!(
            "interpretTiffIfd: unsupported Predictor value {other}"
        ))),
    }
}

/// PhotometricInterpretation, required. WhiteIsZero (0), BlackIsZero (1), and
/// RGB (2) are supported unconditionally; YCbCr (6) is accepted here and
/// cross-validated afterwards in `interpret_tiff_ifd`.
fn resolve_photometric_interpretation(ifd: &TiffIfd) -> Result<u64> {
    let photometric = require_single_value(ifd, TagId::PhotometricInterpretation)?;
    if photometric != 0 && photometric != 1 && photometric != 2 && photometric != 6 {
        return Err(Error::invalid_argument(
            "interpretTiffIfd: unsupported PhotometricInterpretation",
        ));
    }
    Ok(photometric)
}

/// PlanarConfiguration, defaulting to 1 (chunky). Planar (2) storage is not
/// supported.
fn resolve_planar_configuration(ifd: &TiffIfd) -> Result<u64> {
    let planar_config = ifd.single_value_or(TagId::PlanarConfiguration.as_u16(), 1)?;
    if planar_config != 1 {
        return Err(Error::invalid_argument(
            "interpretTiffIfd: unsupported PlanarConfiguration",
        ));
    }
    Ok(planar_config)
}

/// The strip/tile grid dimensions and which tags hold the byte offsets/counts,
/// resolved according to whichever of the two mutually-exclusive layouts
/// (`has_strips`) this IFD uses.
struct StripOrTileLayout {
    tile_width: u32,
    tile_height: u32,
    offsets_tag: TagId,
    byte_counts_tag: TagId,
}

fn resolve_strip_or_tile_layout(
    ifd: &TiffIfd,
    has_strips: bool,
    image_width: u32,
) -> Result<StripOrTileLayout> {
    if has_strips {
        let rows_per_strip = require_single_value(ifd, TagId::RowsPerStrip)?;
        return Ok(StripOrTileLayout {
            tile_width: image_width,
            tile_height: rows_per_strip as u32,
            offsets_tag: TagId::StripOffsets,
            byte_counts_tag: TagId::StripByteCounts,
        });
    }
    let tile_width = require_single_value(ifd, TagId::TileWidth)?;
    let tile_length = require_single_value(ifd, TagId::TileLength)?;
    Ok(StripOrTileLayout {
        tile_width: tile_width as u32,
        tile_height: tile_length as u32,
        offsets_tag: TagId::TileOffsets,
        byte_counts_tag: TagId::TileByteCounts,
    })
}

/// Interprets a parsed IFD against this backend's supported baseline tag
/// subset.
///
/// # Errors
///
/// Returns [`crate::ErrorCode::InvalidArgument`] if a required tag is missing,
/// or any tag holds an unsupported value.
pub fn interpret_tiff_ifd(ifd: &TiffIfd) -> Result<TiffDirectory> {
    let width = require_single_value(ifd, TagId::ImageWidth)?;
    let height = require_single_value(ifd, TagId::ImageLength)?;

    let samples_per_pixel = resolve_samples_per_pixel(ifd)?;

    let bits_per_sample_values = require_tag(ifd, TagId::BitsPerSample)?;
    require_uniform_bits_per_sample(&bits_per_sample_values)?;

    let compression = resolve_compression(ifd)?;
    let photometric = resolve_photometric_interpretation(ifd)?;
    resolve_planar_configuration(ifd)?;

    let sample_format = ifd.single_value_or(TagId::SampleFormat.as_u16(), 1)?;
    let pixel_type = resolve_pixel_type(bits_per_sample_values[0], sample_format)?;

    let predictor = resolve_predictor(ifd, pixel_type, compression)?;

    if compression == TiffCompression::Jpeg && pixel_type != PixelType::UInt8 {
        return Err(Error::invalid_argument(
            "interpretTiffIfd: Jpeg compression requires UInt8 samples",
        ));
    }
    if photometric == 6 && compression != TiffCompression::Jpeg {
        return Err(Error::invalid_argument(
            "interpretTiffIfd: PhotometricInterpretation=YCbCr(6) requires Jpeg compression",
        ));
    }
    if compression == TiffCompression::Jpeg
        && samples_per_pixel == 1
        && photometric != 0
        && photometric != 1
    {
        return Err(Error::invalid_argument(
            "interpretTiffIfd: Jpeg grayscale requires PhotometricInterpretation WhiteIsZero(0) or BlackIsZero(1)",
        ));
    }
    if compression == TiffCompression::Jpeg && samples_per_pixel == 3 && photometric != 6 {
        return Err(Error::invalid_argument(
            "interpretTiffIfd: Jpeg RGB requires PhotometricInterpretation YCbCr(6)",
        ));
    }

    let has_strips = ifd.tag(TagId::StripOffsets.as_u16()).is_ok();
    let has_tiles = ifd.tag(TagId::TileOffsets.as_u16()).is_ok();
    if has_strips == has_tiles {
        return Err(Error::invalid_argument(
            "interpretTiffIfd: image must be exactly one of stripped or tiled",
        ));
    }

    let mut directory = TiffDirectory {
        image_width: width as u32,
        image_height: height as u32,
        pixel_type,
        samples_per_pixel: samples_per_pixel as u32,
        ..TiffDirectory::default()
    };

    let strip_or_tile = resolve_strip_or_tile_layout(ifd, has_strips, directory.image_width)?;

    let offsets = require_tag(ifd, strip_or_tile.offsets_tag)?;
    let byte_counts = require_tag(ifd, strip_or_tile.byte_counts_tag)?;
    let byte_ranges = build_byte_ranges(&offsets, &byte_counts)?;

    directory.layout = TileLayout::new(
        TileExtent::new(strip_or_tile.tile_width, strip_or_tile.tile_height),
        directory.image_width,
        directory.image_height,
        1,
    );
    directory.tile_byte_ranges = byte_ranges;
    directory.compression = compression;
    directory.predictor = predictor;

    decode_ptiff_tag(ifd, TagId::PtiffSpice.as_u16(), &mut directory);
    decode_ptiff_tag(ifd, TagId::PtiffCameraGeometry.as_u16(), &mut directory);
    decode_ptiff_tag(ifd, TagId::PtiffCrs.as_u16(), &mut directory);
    decode_ptiff_tag(ifd, TagId::PtiffScientificLayers.as_u16(), &mut directory);
    decode_ptiff_tag(ifd, TagId::PtiffProvenance.as_u16(), &mut directory);

    Ok(directory)
}

/// Converts a [`TiffDirectory`] into the format-neutral [`StorageModel`] that
/// a storage backend's deserialization returns. Fields: "imageWidth",
/// "imageHeight", "samplesPerPixel" (decimal strings), "pixelType" (one of
/// "UInt8"/"UInt16"/"UInt32"/"Float32"), "compression", "predictor",
/// "jpegQuality", plus any PTIFF extension fields.
#[must_use]
pub fn to_storage_model(directory: &TiffDirectory) -> StorageModel {
    let mut model = StorageModel::new();
    model.set_field("imageWidth", directory.image_width.to_string());
    model.set_field("imageHeight", directory.image_height.to_string());
    model.set_field("samplesPerPixel", directory.samples_per_pixel.to_string());
    model.set_field(
        "pixelType",
        pixel_type_field_value(directory.pixel_type).to_string(),
    );
    model.set_field(
        "compression",
        compression_field_value(directory.compression).to_string(),
    );
    model.set_field(
        "predictor",
        predictor_field_value(directory.predictor).to_string(),
    );
    model.set_field("jpegQuality", directory.jpeg_quality.to_string());
    for (key, value) in &directory.ptiff_fields {
        model.set_field(key.clone(), value.clone());
    }
    model
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::backend::tiff::ifd_writer::{write_tiff_ifd, TiffIfdEntryToWrite};
    use crate::io::backend::tiff::FieldType;
    use crate::io::memory_binary_writer::MemoryBinaryWriter;

    fn long(tag: u16, values: Vec<u32>) -> TiffIfdEntryToWrite {
        TiffIfdEntryToWrite::new(tag, FieldType::Long, values)
    }
    fn short(tag: u16, values: Vec<u32>) -> TiffIfdEntryToWrite {
        TiffIfdEntryToWrite::new(tag, FieldType::Short, values)
    }

    // Builds a stripped grayscale IFD with the given pixel type and offset/
    // bytecount, parses it, and interprets it.
    fn build_and_interpret_strip_ifd(
        pixel_type: PixelType,
        width: u32,
        height: u32,
        offsets: Vec<u32>,
        byte_counts: Vec<u32>,
    ) -> Result<TiffDirectory> {
        let (bpp, sfmt) = match pixel_type {
            PixelType::UInt8 => (8u32, 1u32),
            PixelType::UInt16 => (16, 1),
            PixelType::UInt32 => (32, 1),
            PixelType::Float32 => (32, 3),
            PixelType::Float64 => (64, 3),
        };
        let mut entries = vec![
            long(256, vec![width]),         // ImageWidth
            long(257, vec![height]),        // ImageLength
            short(258, vec![bpp]),          // BitsPerSample
            long(259, vec![1]),             // Compression = None
            long(262, vec![1]),             // Photometric = BlackIsZero
            long(277, vec![1]),             // SamplesPerPixel
            long(278, vec![height]),        // RowsPerStrip
            long(279, byte_counts.clone()), // StripByteCounts
            short(339, vec![sfmt]),         // SampleFormat
            long(273, offsets.clone()),     // StripOffsets
        ];
        // Explicitly sort ascending by tag id (the parser and writer both expect
        // sorted input for some paths, though write_tiff_ifd sorts anyway).
        entries.sort_by_key(|e| e.tag_id);

        let mut w = MemoryBinaryWriter::new();
        write_tiff_ifd(&mut w, entries, false, 0).unwrap();
        let buf = w.take_buffer();
        let mut r = crate::io::memory_binary_reader::MemoryBinaryReader::from_vec(buf);
        let ifd = crate::io::backend::tiff::read_tiff_ifd(&mut r, 0, Endian::Little, false)?;
        interpret_tiff_ifd(&ifd)
    }

    #[test]
    fn interprets_stripped_grayscale_uint8() {
        let d = build_and_interpret_strip_ifd(PixelType::UInt8, 64, 32, vec![1024], vec![64 * 32])
            .unwrap();
        assert_eq!(d.image_width, 64);
        assert_eq!(d.image_height, 32);
        assert_eq!(d.pixel_type, PixelType::UInt8);
        assert_eq!(d.samples_per_pixel, 1);
        assert_eq!(d.compression, TiffCompression::None);
        assert_eq!(d.predictor, TiffPredictor::None);
        assert_eq!(d.tile_byte_ranges.len(), 1);
        assert_eq!(d.tile_byte_ranges[0].offset, 1024);
        assert_eq!(d.tile_byte_ranges[0].byte_count, 64 * 32);
        // Stripped: tile size = full image.
        assert_eq!(d.layout.tile_size.width, 64);
        assert_eq!(d.layout.tile_size.height, 32);
    }

    #[test]
    fn to_storage_model_round_trip_fields() {
        let d =
            build_and_interpret_strip_ifd(PixelType::Float32, 16, 16, vec![64], vec![16 * 16 * 4])
                .unwrap();
        let m = to_storage_model(&d);
        assert_eq!(m.field("imageWidth").unwrap(), "16");
        assert_eq!(m.field("pixelType").unwrap(), "Float32");
        assert_eq!(m.field("compression").unwrap(), "None");
        assert_eq!(m.field("predictor").unwrap(), "None");
    }
}
