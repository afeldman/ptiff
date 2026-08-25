//! Builds the write plan for one or more TIFF/BigTIFF baseline images from a
//! format-neutral [`StorageModel`].
//!
//! Mirrors `ptiff::io::backend::tiff::tiff_directory_writer.{hpp,cpp}`.

use crate::io::backend::tiff::directory::{TiffCompression, TiffDirectory, TiffPredictor};
use crate::io::backend::tiff::header::{K_BIG_TIFF_HEADER_SIZE, K_CLASSIC_TIFF_HEADER_SIZE};
use crate::io::backend::tiff::ifd_writer::{
    tiff_ifd_byte_size, value_slot_offsets_relative, TiffIfdEntryToWrite,
};
use crate::io::backend::tiff::pixel_format::{
    bits_per_sample_for, bytes_per_sample, pixel_type_from_field_value, sample_format_for,
};
use crate::io::backend::tiff::ptiff_metadata::{
    encode_metadata_payload, records_from_storage_model,
};
use crate::io::backend::tiff::tag::{FieldType, TagId};
use crate::io::backend::tiff::Endian;
use crate::io::StorageModel;
use crate::tile::{TileExtent, TileLayout};
use crate::{Error, PixelType, Result};

/// Everything needed to write one baseline TIFF file: the resolved
/// [`TiffDirectory`] (shared with the read path) and the exact IFD entries to
/// hand to `write_tiff_ifd`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TiffWritePlan {
    /// The resolved directory shared with the read path.
    pub directory: TiffDirectory,
    /// The IFD entries to serialize.
    pub entries: Vec<TiffIfdEntryToWrite>,
    /// Whether the file uses the BigTIFF container.
    pub is_big_tiff: bool,
}

/// Top-level layout of a whole TIFF file that may hold several images.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TiffFileWritePlan {
    /// One plan per image, in file (chain) order.
    pub images: Vec<TiffWritePlan>,
    /// Whether the file uses the BigTIFF container (shared by every image).
    pub is_big_tiff: bool,
}

fn tag_id(id: TagId) -> u16 {
    id.as_u16()
}

fn require_uint32_field(model: &StorageModel, key: &str) -> Result<u32> {
    let value = model.field(key).map_err(|_| {
        Error::invalid_argument(format!("planTiffWrite: missing required field \"{key}\""))
    })?;
    value.parse::<u32>().map_err(|_| {
        Error::invalid_argument(format!(
            "planTiffWrite: field \"{key}\" is not a valid unsigned integer"
        ))
    })
}

fn parse_compression(model: &StorageModel) -> Result<Option<TiffCompression>> {
    let Ok(value) = model.field("compression") else {
        return Ok(None);
    };
    match value {
        "None" => Ok(Some(TiffCompression::None)),
        "PackBits" => Ok(Some(TiffCompression::PackBits)),
        "LZW" => Ok(Some(TiffCompression::Lzw)),
        "Deflate" => Ok(Some(TiffCompression::Deflate)),
        "Jpeg" => Ok(Some(TiffCompression::Jpeg)),
        other => Err(Error::invalid_argument(format!(
            "planTiffWrite: unsupported compression \"{other}\""
        ))),
    }
}

fn parse_predictor(model: &StorageModel) -> Result<Option<TiffPredictor>> {
    let Ok(value) = model.field("predictor") else {
        return Ok(None);
    };
    match value {
        "None" => Ok(Some(TiffPredictor::None)),
        "HorizontalDifferencing" => Ok(Some(TiffPredictor::HorizontalDifferencing)),
        other => Err(Error::invalid_argument(format!(
            "planTiffWrite: unsupported predictor \"{other}\""
        ))),
    }
}

/// "jpegQuality" field, defaulting to 90 when absent.
fn parse_jpeg_quality(model: &StorageModel) -> Result<u32> {
    let Ok(value) = model.field("jpegQuality") else {
        return Ok(90);
    };
    let parsed = value.parse::<u32>().map_err(|_| {
        Error::invalid_argument(
            "planTiffWrite: field \"jpegQuality\" is not a valid unsigned integer",
        )
    })?;
    if parsed > 100 {
        return Err(Error::invalid_argument(
            "planTiffWrite: jpegQuality must be in [0, 100]",
        ));
    }
    Ok(parsed)
}

/// Appends PTIFF extension tag entries (65001-65005) to `entries` for every
/// domain whose fields are present in `model` under the `ptiff.<domain>.*`
/// convention. A model with no `ptiff.*` fields emits nothing.
fn append_ptiff_tags(model: &StorageModel, entries: &mut Vec<TiffIfdEntryToWrite>) -> Result<()> {
    const DOMAINS: [(TagId, &str); 5] = [
        (TagId::PtiffSpice, "spice"),
        (TagId::PtiffCameraGeometry, "camera"),
        (TagId::PtiffCrs, "crs"),
        (TagId::PtiffScientificLayers, "layers"),
        (TagId::PtiffProvenance, "provenance"),
    ];
    for (tag, prefix) in DOMAINS {
        let records = records_from_storage_model(model, prefix);
        if records.is_empty() {
            continue;
        }
        let payload = encode_metadata_payload(&records)?;
        let byte_values: Vec<u32> = payload.iter().map(|&b| u32::from(b)).collect();
        entries.push(TiffIfdEntryToWrite::new(
            tag_id(tag),
            FieldType::Byte,
            byte_values,
        ));
    }
    Ok(())
}

fn parse_container(model: &StorageModel) -> Result<bool> {
    let Ok(value) = model.field("container") else {
        return Ok(false);
    };
    match value {
        "Classic" => Ok(false),
        "BigTiff" => Ok(true),
        other => Err(Error::invalid_argument(format!(
            "planTiffWrite: unsupported container \"{other}\""
        ))),
    }
}

/// Multiplies `a` and `b`, rejecting the result rather than silently
/// overflowing.
fn checked_multiply(a: u64, b: u64) -> Result<u64> {
    a.checked_mul(b)
        .ok_or_else(|| Error::invalid_argument("planTiffWrite: strip size computation overflows"))
}

/// Parses the optional "tileWidth"/"tileHeight" field pair. `None` when both are
/// absent (strip layout).
fn parse_tile_size(model: &StorageModel) -> Result<Option<(u32, u32)>> {
    let has_width = model.field("tileWidth").is_ok();
    let has_height = model.field("tileHeight").is_ok();
    if !has_width && !has_height {
        return Ok(None);
    }
    if has_width != has_height {
        return Err(Error::invalid_argument(
            "planTiffWrite: \"tileWidth\" and \"tileHeight\" must both be set or both be absent",
        ));
    }
    let width = require_uint32_field(model, "tileWidth")?;
    let height = require_uint32_field(model, "tileHeight")?;
    if width == 0 || height == 0 || width % 16 != 0 || height % 16 != 0 {
        return Err(Error::invalid_argument(
            "planTiffWrite: tileWidth/tileHeight must be nonzero multiples of 16",
        ));
    }
    Ok(Some((width, height)))
}

/// Builds a [`TiffWritePlan`] from a format-neutral [`StorageModel`].
///
/// Required fields: "imageWidth", "imageHeight", "samplesPerPixel" (1 or 3),
/// "pixelType" (one of "UInt8"/"UInt16"/"UInt32"/"Float32"). Optional
/// "compression" maps "None" (1), "PackBits" (32773), "LZW" (5), "Deflate" (8),
/// "Jpeg" (7); optional "predictor" maps "None" or "HorizontalDifferencing"
/// (2). Optional "container" selects "Classic" (default) or "BigTiff".
///
/// # Errors
///
/// Returns [`crate::ErrorCode::InvalidArgument`] for a missing/unparsable
/// required field, unsupported value, a predictor combined with
/// uncompressed-tiled output, float samples combined with horizontal
/// differencing, or quality out of range.
pub fn plan_tiff_write(model: &StorageModel) -> Result<TiffWritePlan> {
    let image_width = require_uint32_field(model, "imageWidth")?;
    let image_height = require_uint32_field(model, "imageHeight")?;
    let samples_per_pixel = require_uint32_field(model, "samplesPerPixel")?;
    if !(1..=512).contains(&samples_per_pixel) {
        return Err(Error::invalid_argument(
            "planTiffWrite: unsupported samplesPerPixel",
        ));
    }

    let pixel_type_field = model.field("pixelType").map_err(|_| {
        Error::invalid_argument("planTiffWrite: missing required field \"pixelType\"")
    })?;
    let pixel_type = pixel_type_from_field_value(pixel_type_field)?;

    let tile_size_opt = parse_tile_size(model)?;
    let tiled = tile_size_opt.is_some();
    let (tile_width_value, tile_height_value) = tile_size_opt.unwrap_or((0, 0));

    let compression = parse_compression(model)?.unwrap_or(TiffCompression::None);
    let predictor = parse_predictor(model)?.unwrap_or(TiffPredictor::None);

    if compression == TiffCompression::Jpeg && pixel_type != PixelType::UInt8 {
        return Err(Error::invalid_argument(
            "planTiffWrite: Jpeg compression requires UInt8 pixelType",
        ));
    }

    let jpeg_quality = parse_jpeg_quality(model)?;

    let is_big_tiff = parse_container(model)?;

    // Horizontal differencing is defined only for integer samples.
    if predictor == TiffPredictor::HorizontalDifferencing && !pixel_type.is_integer() {
        return Err(Error::invalid_argument(
            "planTiffWrite: horizontal differencing requires integer samples (UInt8/UInt16/UInt32)",
        ));
    }
    if predictor == TiffPredictor::HorizontalDifferencing && compression == TiffCompression::Jpeg {
        return Err(Error::invalid_argument(
            "planTiffWrite: Predictor is not defined for Jpeg compression",
        ));
    }
    // A predictor only feeds an encoder; on an uncompressed tiled image the
    // reader would try to undo differencing on raw pixels and corrupt them.
    if tiled && predictor != TiffPredictor::None && compression == TiffCompression::None {
        return Err(Error::invalid_argument(
            "planTiffWrite: a predictor requires a compression scheme; uncompressed tiled write cannot carry one",
        ));
    }

    let photometric: u32 = if compression == TiffCompression::Jpeg && samples_per_pixel == 3 {
        6 // YCbCr
    } else if samples_per_pixel == 3 {
        2 // RGB
    } else {
        1 // BlackIsZero
    };
    let compression_tag: u32 = match compression {
        TiffCompression::Lzw => 5,
        TiffCompression::PackBits => 32773,
        TiffCompression::Deflate => 8,
        TiffCompression::Jpeg => 7,
        TiffCompression::None => 1,
    };

    let mut entries = vec![
        TiffIfdEntryToWrite::new(
            tag_id(TagId::ImageWidth),
            FieldType::Long,
            vec![image_width],
        ),
        TiffIfdEntryToWrite::new(
            tag_id(TagId::ImageLength),
            FieldType::Long,
            vec![image_height],
        ),
        // BitsPerSample: one entry per sample, all equal.
        TiffIfdEntryToWrite::new(
            tag_id(TagId::BitsPerSample),
            FieldType::Short,
            vec![bits_per_sample_for(pixel_type) as u32; samples_per_pixel as usize],
        ),
        TiffIfdEntryToWrite::new(
            tag_id(TagId::Compression),
            FieldType::Long,
            vec![compression_tag],
        ),
        TiffIfdEntryToWrite::new(
            tag_id(TagId::PhotometricInterpretation),
            FieldType::Long,
            vec![photometric],
        ),
        TiffIfdEntryToWrite::new(
            tag_id(TagId::SamplesPerPixel),
            FieldType::Long,
            vec![samples_per_pixel],
        ),
        TiffIfdEntryToWrite::new(
            tag_id(TagId::SampleFormat),
            FieldType::Short,
            vec![sample_format_for(pixel_type) as u32],
        ),
    ];

    {
        let baseline_samples = if photometric == 2 { 3 } else { 1 };
        if samples_per_pixel > baseline_samples {
            entries.push(TiffIfdEntryToWrite::new(
                tag_id(TagId::ExtraSamples),
                FieldType::Short,
                vec![0; (samples_per_pixel - baseline_samples) as usize],
            ));
        }
    }

    let mut tile_columns = 0u32;
    let mut tile_rows = 0u32;
    let mut tile_byte_size = 0u64;
    let mut strip_byte_count_value = 0u64;

    if tiled {
        tile_columns = image_width.div_ceil(tile_width_value);
        tile_rows = image_height.div_ceil(tile_height_value);
        let tile_count = u64::from(tile_columns) * u64::from(tile_rows);

        let tile_row_bytes = checked_multiply(
            checked_multiply(u64::from(tile_width_value), u64::from(samples_per_pixel))?,
            u64::from(bytes_per_sample(pixel_type)),
        )?;
        tile_byte_size = checked_multiply(tile_row_bytes, u64::from(tile_height_value))?;

        entries.push(TiffIfdEntryToWrite::new(
            tag_id(TagId::TileWidth),
            FieldType::Long,
            vec![tile_width_value],
        ));
        entries.push(TiffIfdEntryToWrite::new(
            tag_id(TagId::TileLength),
            FieldType::Long,
            vec![tile_height_value],
        ));
        entries.push(TiffIfdEntryToWrite::new(
            tag_id(TagId::TileOffsets),
            FieldType::Long,
            vec![0; tile_count as usize], // patched below
        ));
        entries.push(TiffIfdEntryToWrite::new(
            tag_id(TagId::TileByteCounts),
            FieldType::Long,
            vec![tile_byte_size as u32; tile_count as usize],
        ));
    } else {
        let row_bytes = checked_multiply(
            checked_multiply(u64::from(image_width), u64::from(samples_per_pixel))?,
            u64::from(bytes_per_sample(pixel_type)),
        )?;
        strip_byte_count_value = checked_multiply(row_bytes, u64::from(image_height))?;

        entries.push(TiffIfdEntryToWrite::new(
            tag_id(TagId::StripOffsets),
            FieldType::Long,
            vec![0], // patched below
        ));
        entries.push(TiffIfdEntryToWrite::new(
            tag_id(TagId::RowsPerStrip),
            FieldType::Long,
            vec![image_height],
        ));
        entries.push(TiffIfdEntryToWrite::new(
            tag_id(TagId::StripByteCounts),
            FieldType::Long,
            vec![if compression == TiffCompression::None {
                strip_byte_count_value as u32
            } else {
                0 // placeholder, back-patched by the Sink after encode
            }],
        ));
    }

    if predictor != TiffPredictor::None {
        entries.push(TiffIfdEntryToWrite::new(
            tag_id(TagId::Predictor),
            FieldType::Long,
            vec![2], // HorizontalDifferencing
        ));
    }
    if photometric == 6 {
        entries.push(TiffIfdEntryToWrite::new(
            tag_id(TagId::YCbCrSubSampling),
            FieldType::Short,
            vec![1, 1], // 4:4:4
        ));
    }

    // Emit any PTIFF extension metadata before the data offset is computed so
    // the (possibly out-of-line) private-tag value bytes are included.
    append_ptiff_tags(model, &mut entries)?;

    let header_size = if is_big_tiff {
        K_BIG_TIFF_HEADER_SIZE
    } else {
        K_CLASSIC_TIFF_HEADER_SIZE
    };
    let data_offset = header_size + tiff_ifd_byte_size(&entries, is_big_tiff);

    let mut tile_byte_ranges = Vec::new();
    if tiled {
        let tile_count = u64::from(tile_columns) * u64::from(tile_rows);
        let mut tile_offsets = Vec::with_capacity(tile_count as usize);
        for i in 0..tile_count {
            let offset = data_offset + i * tile_byte_size;
            tile_offsets.push(offset as u32);
            tile_byte_ranges.push(crate::io::backend::tiff::directory::TileByteRange::new(
                offset,
                tile_byte_size,
            ));
        }
        for entry in &mut entries {
            if entry.tag_id == tag_id(TagId::TileOffsets) {
                entry.values = std::mem::take(&mut tile_offsets);
                break;
            }
        }
    } else {
        for entry in &mut entries {
            if entry.tag_id == tag_id(TagId::StripOffsets) {
                entry.values = vec![data_offset as u32];
            }
        }
        tile_byte_ranges = vec![crate::io::backend::tiff::directory::TileByteRange::new(
            data_offset,
            strip_byte_count_value,
        )];
    }

    let mut byte_count_patch_offset = 0u64;
    let mut tile_offsets_value_addresses: Vec<u64> = Vec::new();
    let mut tile_byte_counts_value_addresses: Vec<u64> = Vec::new();
    if tiled {
        // Tiled layouts carry per-tile offsets/counts. When compressed, the
        // Sink back-patches each tile's actual (variable) offset and size after
        // encode, so pre-compute each value's absolute slot address. Offsets
        // are computed relative to the IFD's first byte and rebased to the
        // single-image layout (IFD right after the header); a multi-image plan
        // rebases them further in `plan_tiff_write_multi`.
        let slots = value_slot_offsets_relative(&entries, is_big_tiff);
        let remap = |tag: TagId| -> Vec<u64> {
            let empty = Vec::new();
            let v = slots.get(&tag.as_u16()).unwrap_or(&empty);
            v.iter().map(|&rel| header_size + rel).collect()
        };
        tile_offsets_value_addresses = remap(TagId::TileOffsets);
        tile_byte_counts_value_addresses = remap(TagId::TileByteCounts);
        debug_assert_eq!(
            tile_offsets_value_addresses.len(),
            tile_byte_counts_value_addresses.len(),
            "TileOffsets/TileByteCounts slot tables must align"
        );
    } else {
        let mut sorted_tags: Vec<u16> = entries.iter().map(|e| e.tag_id).collect();
        sorted_tags.sort_unstable();
        let byte_count_index = sorted_tags
            .iter()
            .position(|&t| t == tag_id(TagId::StripByteCounts))
            .unwrap_or(0);
        let entry_record_size = if is_big_tiff { 20u64 } else { 12u64 };
        let entry_count_field_size = if is_big_tiff { 8u64 } else { 2u64 };
        let value_area_offset_within_record = if is_big_tiff { 12u64 } else { 8u64 };
        byte_count_patch_offset = header_size
            + entry_count_field_size
            + byte_count_index as u64 * entry_record_size
            + value_area_offset_within_record;
    }

    let layout = if tiled {
        TileLayout::new(
            TileExtent::new(tile_width_value, tile_height_value),
            image_width,
            image_height,
            1,
        )
    } else {
        TileLayout::new(
            TileExtent::new(image_width, image_height),
            image_width,
            image_height,
            1,
        )
    };

    let directory = TiffDirectory {
        image_width,
        image_height,
        pixel_type,
        samples_per_pixel,
        layout,
        tile_byte_ranges,
        compression,
        predictor,
        endian: Endian::Little,
        strip_byte_counts_patch_offset: byte_count_patch_offset,
        tile_offsets_value_addresses,
        tile_byte_counts_value_addresses,
        jpeg_quality,
        ptiff_fields: std::collections::BTreeMap::new(),
    };

    Ok(TiffWritePlan {
        directory,
        entries,
        is_big_tiff,
    })
}

/// Builds a [`TiffFileWritePlan`] -- the top-level layout of a whole TIFF file
/// that may hold several images (a chain of IFDs).
///
/// Each image's offsets are stitched contiguously: header, then IFD 0 .. N-1
/// (each linked to the next through its next-IFD offset; the last's next-IFD is
/// 0), then every image's pixel data. All models must agree on `is_big_tiff`;
/// at least one model is required.
///
/// # Errors
///
/// Returns [`crate::ErrorCode::InvalidArgument`] if `models` is empty, any
/// per-image plan fails, an image is tiled and compressed (a limitation of the
/// static multi-image layout; write it as a single image), or the container
/// kinds disagree.
pub fn plan_tiff_write_multi(models: &[StorageModel]) -> Result<TiffFileWritePlan> {
    if models.is_empty() {
        return Err(Error::invalid_argument(
            "planTiffWriteMulti: at least one model is required",
        ));
    }

    let mut images = Vec::with_capacity(models.len());
    let mut is_big_tiff = false;
    for (i, model) in models.iter().enumerate() {
        let plan = plan_tiff_write(model)?;
        if i == 0 {
            is_big_tiff = plan.is_big_tiff;
        } else if plan.is_big_tiff != is_big_tiff {
            return Err(Error::invalid_argument(
                "planTiffWriteMulti: all images must agree on a container kind (classic or BigTIFF)",
            ));
        }
        images.push(plan);
    }

    // The multi-image plan lays each image's data region out at a statically
    // pre-computed, uncompressed size so the next image's offset is known up
    // front. A tiled-and-compressed image's tiles vary in size, so its region
    // cannot be reserved this way; reject it here. (Single-image tiled write
    // with any compression is fully supported through `plan_tiff_write` +
    // `open_image_sink`, which streams tiles back-to-back and patches offsets.)
    for (i, image) in images.iter().enumerate() {
        if image.directory.layout.tile_size.width < image.directory.image_width
            && image.directory.compression != TiffCompression::None
        {
            return Err(Error::invalid_argument(format!(
                "planTiffWriteMulti: image {i} is tiled and compressed; multi-image tiled+compressed is not yet supported (write it as a single image)"
            )));
        }
    }

    let header_size = if is_big_tiff {
        K_BIG_TIFF_HEADER_SIZE
    } else {
        K_CLASSIC_TIFF_HEADER_SIZE
    };

    // Every IFD's absolute file offset, chained via NextIFD.
    let mut ifd_offsets = Vec::with_capacity(images.len());
    let mut cursor = header_size;
    for image in &images {
        ifd_offsets.push(cursor);
        let size = tiff_ifd_byte_size(&image.entries, is_big_tiff);
        cursor = cursor
            .checked_add(size)
            .ok_or_else(|| Error::invalid_argument("planTiffWriteMulti: IFD layout overflows"))?;
    }
    let first_data_byte = cursor;

    // Each image's data region start and reserved span.
    let mut data_offsets = Vec::with_capacity(images.len());
    let mut data_cursor = first_data_byte;
    for image in &images {
        data_offsets.push(data_cursor);
        let mut span = 0u64;
        for range in &image.directory.tile_byte_ranges {
            span = span.checked_add(range.byte_count).ok_or_else(|| {
                Error::invalid_argument("planTiffWriteMulti: image data region overflows")
            })?;
        }
        data_cursor = data_cursor.checked_add(span).ok_or_else(|| {
            Error::invalid_argument("planTiffWriteMulti: file data layout overflows")
        })?;
    }

    // Rebase each image's directory and IFD tag values to absolute offsets.
    for (i, image) in images.iter_mut().enumerate() {
        let original_data_base = header_size + tiff_ifd_byte_size(&image.entries, is_big_tiff);
        let data_delta = data_offsets[i] - original_data_base;
        for range in &mut image.directory.tile_byte_ranges {
            range.offset += data_delta;
        }
        for entry in &mut image.entries {
            if entry.tag_id == tag_id(TagId::StripOffsets)
                || entry.tag_id == tag_id(TagId::TileOffsets)
            {
                for value in &mut entry.values {
                    *value = (*value as u64 + data_delta) as u32;
                }
            }
        }
        if image.directory.strip_byte_counts_patch_offset != 0 {
            image.directory.strip_byte_counts_patch_offset += ifd_offsets[i] - header_size;
        }
    }

    Ok(TiffFileWritePlan {
        images,
        is_big_tiff,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strip_model(width: u32, height: u32) -> StorageModel {
        let mut m = StorageModel::new();
        m.set_field("imageWidth", width.to_string());
        m.set_field("imageHeight", height.to_string());
        m.set_field("samplesPerPixel", "1");
        m.set_field("pixelType", "UInt8");
        m
    }

    /// Grayscale model (samplesPerPixel = 1), defaulting to UInt8.
    fn gray_model() -> StorageModel {
        strip_model(2, 2)
    }

    /// N-band (multispectral) image model with the given sample count.
    fn nband_model(samples: u32) -> StorageModel {
        let mut m = StorageModel::new();
        m.set_field("imageWidth", "2");
        m.set_field("imageHeight", "2");
        m.set_field("samplesPerPixel", samples.to_string());
        m.set_field("pixelType", "UInt8");
        m
    }

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

    #[test]
    fn plans_tiled_compressed_with_patch_addresses() {
        // Single-image tiled write with any compression is now supported: the
        // plan keeps per-tile TileOffsets/TileByteCounts slot addresses that
        // the Sink back-patches after encode.
        let plan = plan_tiff_write(&tiled_model(32, 48, "LZW", "HorizontalDifferencing")).unwrap();
        assert_eq!(plan.directory.compression, TiffCompression::Lzw);
        assert_eq!(
            plan.directory.predictor,
            TiffPredictor::HorizontalDifferencing
        );
        // 32x48 at 16x16 => 2 cols x 3 rows = 6 tiles.
        assert_eq!(plan.directory.tile_byte_ranges.len(), 6);
        assert_eq!(plan.directory.tile_offsets_value_addresses.len(), 6);
        assert_eq!(plan.directory.tile_byte_counts_value_addresses.len(), 6);
    }

    #[test]
    fn rejects_tiled_predictor_without_compression() {
        let err =
            plan_tiff_write(&tiled_model(32, 32, "None", "HorizontalDifferencing")).unwrap_err();
        assert_eq!(err.code(), crate::ErrorCode::InvalidArgument);
    }

    #[test]
    fn multi_plan_rejects_tiled_compressed() {
        let models = vec![strip_model(16, 16), tiled_model(32, 32, "PackBits", "None")];
        let err = plan_tiff_write_multi(&models).unwrap_err();
        assert_eq!(err.code(), crate::ErrorCode::InvalidArgument);
    }

    #[test]
    fn plans_simple_grayscale_strip() {
        let plan = plan_tiff_write(&strip_model(64, 32)).unwrap();
        assert_eq!(plan.directory.image_width, 64);
        assert_eq!(plan.directory.image_height, 32);
        assert_eq!(plan.directory.pixel_type, PixelType::UInt8);
        assert_eq!(plan.directory.compression, TiffCompression::None);
        assert_eq!(plan.directory.tile_byte_ranges.len(), 1);
        assert!(!plan.is_big_tiff);
    }

    #[test]
    fn multi_plan_with_two_images_chains_ifds() {
        let models = vec![strip_model(16, 16), strip_model(8, 8)];
        let plan = plan_tiff_write_multi(&models).unwrap();
        assert_eq!(plan.images.len(), 2);
        assert!(!plan.is_big_tiff);
        // IFD 0 at header (8); IFD 1 immediately after IFD 0.
        let ifd0_size = tiff_ifd_byte_size(&plan.images[0].entries, false);
        assert!(
            plan.images[1].directory.tile_byte_ranges[0].offset
                > plan.images[0].directory.tile_byte_ranges[0].offset
        );
        let _ = ifd0_size;
    }

    // --- N-band / multispectral (samplesPerPixel > 3) parity with the C++
    // --- reference `tiff_directory_writer_test.cpp` (libptiff >= 0.4.0).

    #[test]
    fn accepts_five_band_multispectral_image() {
        // planTiffWrite accepts a 5-band multispectral image.
        let plan = plan_tiff_write(&nband_model(5)).unwrap();
        assert_eq!(plan.directory.samples_per_pixel, 5);
        // 2 * 2 * 5 * 1 = 20 bytes.
        assert_eq!(plan.directory.tile_byte_ranges[0].byte_count, 20);
    }

    #[test]
    fn emits_extra_samples_for_multispectral_but_not_rgb_or_gray() {
        // 5-band -> ExtraSamples (tag 338) present.
        let plan = plan_tiff_write(&nband_model(5)).unwrap();
        let has_extra_samples = plan
            .entries
            .iter()
            .any(|e| e.tag_id == tag_id(TagId::ExtraSamples));
        assert!(has_extra_samples, "5-band must emit an ExtraSamples tag");

        // RGB (3 bands) and gray (1 band) must NOT get an ExtraSamples tag.
        let rgb_plan = plan_tiff_write(&nband_model(3)).unwrap();
        let rgb_has = rgb_plan
            .entries
            .iter()
            .any(|e| e.tag_id == tag_id(TagId::ExtraSamples));
        assert!(!rgb_has, "RGB must not emit an ExtraSamples tag");

        let gray_plan = plan_tiff_write(&gray_model()).unwrap();
        let gray_has = gray_plan
            .entries
            .iter()
            .any(|e| e.tag_id == tag_id(TagId::ExtraSamples));
        assert!(!gray_has, "grayscale must not emit an ExtraSamples tag");
    }

    #[test]
    fn rejects_zero_samples_per_pixel() {
        let err = plan_tiff_write(&nband_model(0)).unwrap_err();
        assert_eq!(err.code(), crate::ErrorCode::InvalidArgument);
    }

    #[test]
    fn rejects_samples_per_pixel_above_defensive_cap() {
        // 513 is above the defensive 512 cap.
        let err = plan_tiff_write(&nband_model(513)).unwrap_err();
        assert_eq!(err.code(), crate::ErrorCode::InvalidArgument);
    }
}
