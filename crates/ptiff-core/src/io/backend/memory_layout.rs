//! Per-image pixel layout resolution for the Memory ("PMEM") backend.
//!
//! Mirrors `libptiff/include/ptiff/io/backend/memory/memory_layout.hpp`. It
//! derives each image's [`TileLayout`] and pixel sizing from a flat image
//! [`crate::io::StorageModel`], so the memory backend can place tiles at
//! deterministic linear byte offsets.

use crate::io::StorageModel;
use crate::pixel_type::PixelType;
use crate::tile::{TileExtent, TileLayout};
use crate::{Error, Result};

/// Resolved per-image storage metadata plus derived pixel geometry.
#[derive(Debug, Clone)]
pub struct MemoryImageInfo {
    /// The image's tiling.
    pub layout: TileLayout,
    /// Channels per pixel (only 1 or 3 are supported upstream).
    pub samples_per_pixel: u32,
    /// Per-sample primitive type.
    pub pixel_type: PixelType,
    /// `tileW * tileH * samplesPerPixel * bytesPerSample` — bytes per one tile.
    pub tile_bytes: u64,
    /// `rows(0) * columns(0) * tileBytes` — bytes of this image's whole pixel region.
    pub image_pixel_bytes: u64,
}

impl MemoryImageInfo {
    fn new(
        layout: TileLayout,
        samples_per_pixel: u32,
        pixel_type: PixelType,
        tile_bytes: u64,
        image_pixel_bytes: u64,
    ) -> Self {
        Self {
            layout,
            samples_per_pixel,
            pixel_type,
            tile_bytes,
            image_pixel_bytes,
        }
    }
}

/// Parses the `pixelType` string field ("UInt8".."Float64") into a [`PixelType`].
fn parse_pixel_type(value: &str) -> Result<PixelType> {
    match value {
        "UInt8" => Ok(PixelType::UInt8),
        "UInt16" => Ok(PixelType::UInt16),
        "UInt32" => Ok(PixelType::UInt32),
        "Float32" => Ok(PixelType::Float32),
        "Float64" => Ok(PixelType::Float64),
        _ => Err(Error::invalid_argument("memory: unrecognized pixelType")),
    }
}

/// Parses a required numeric u32 scalar field from `node`.
fn required_u32_field(node: &StorageModel, key: &str) -> Result<u32> {
    let field = node.field(key).map_err(|_| {
        Error::invalid_argument(format!("memory: missing required field \"{key}\""))
    })?;
    let parsed = field
        .parse::<u64>()
        .map_err(|_| Error::invalid_argument(format!("memory: non-numeric field \"{key}\"")))?;
    u32::try_from(parsed)
        .map_err(|_| Error::invalid_argument(format!("memory: field \"{key}\" out of range")))
}

/// Multiplies two `u64` values, rejecting instead of overflowing.
fn checked_mul(a: u64, b: u64) -> Result<u64> {
    match a.checked_mul(b) {
        Some(v) => Ok(v),
        None => Err(Error::invalid_argument("memory: pixel geometry overflow")),
    }
}

/// Derives a [`TileLayout`] plus pixel sizing from a flat image model.
///
/// # Errors
///
/// Returns [`crate::ErrorCode::InvalidArgument`] if a required field is
/// missing/non-numeric/out of range, if the image is not tiled, if the
/// pixelType is unrecognized, if the compression is not `None`, or if the
/// derived geometry overflows.
pub fn image_info_from_model(model: &StorageModel) -> Result<MemoryImageInfo> {
    let width = required_u32_field(model, "imageWidth")?;
    let height = required_u32_field(model, "imageHeight")?;
    let tile_width = required_u32_field(model, "tileWidth")?;
    let tile_height = required_u32_field(model, "tileHeight")?;

    if tile_width == 0 || tile_height == 0 {
        return Err(Error::invalid_argument(
            "memory: non-tiled images are not supported",
        ));
    }

    let samples_per_pixel = required_u32_field(model, "samplesPerPixel")?;

    let pixel_type_field = model
        .field("pixelType")
        .map_err(|_| Error::invalid_argument("memory: missing pixelType"))?;
    let pixel_type = parse_pixel_type(pixel_type_field)?;

    if let Ok(compression) = model.field("compression") {
        if compression != "None" {
            return Err(Error::invalid_argument(
                "memory: only uncompressed (compression==None) images are supported",
            ));
        }
    }

    let layout = TileLayout::new(TileExtent::new(tile_width, tile_height), width, height, 1);

    let bytes_per_sample = pixel_type.bytes_per_sample() as u64;
    let per_tile = checked_mul(u64::from(tile_width), u64::from(tile_height))?;
    let per_tile_samples = checked_mul(per_tile, u64::from(samples_per_pixel))?;
    let tile_bytes = checked_mul(per_tile_samples, bytes_per_sample)?;

    let columns = layout.columns(0);
    let rows = layout.rows(0);
    let image_pixels = checked_mul(u64::from(columns), u64::from(rows))?;
    let image_pixel_bytes = checked_mul(image_pixels, tile_bytes)?;

    Ok(MemoryImageInfo::new(
        layout,
        samples_per_pixel,
        pixel_type,
        tile_bytes,
        image_pixel_bytes,
    ))
}

/// Returns the absolute byte offset of image `image_index`'s pixel region
/// within a document whose model bytes occupy `doc_prefix` at the front.
pub fn image_pixel_offset(infos: &[MemoryImageInfo], image_index: usize, doc_prefix: u64) -> u64 {
    let mut offset = doc_prefix;
    for info in infos.iter().take(image_index) {
        offset += info.image_pixel_bytes;
    }
    offset
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ErrorCode;

    fn tiled_model() -> StorageModel {
        let mut m = StorageModel::new();
        m.set_field("imageWidth", "64");
        m.set_field("imageHeight", "32");
        m.set_field("tileWidth", "16");
        m.set_field("tileHeight", "16");
        m.set_field("samplesPerPixel", "1");
        m.set_field("pixelType", "UInt8");
        m
    }

    #[test]
    fn derives_layout_and_geometry() {
        let info = image_info_from_model(&tiled_model()).expect("valid");
        assert_eq!(info.layout.image_width, 64);
        assert_eq!(info.layout.image_height, 32);
        assert_eq!(info.layout.tile_size, TileExtent::new(16, 16));
        assert_eq!(info.samples_per_pixel, 1);
        assert_eq!(info.pixel_type, PixelType::UInt8);
        // 16*16*1*1
        assert_eq!(info.tile_bytes, 256);
        // 4 cols * 2 rows * 256
        assert_eq!(info.image_pixel_bytes, 2048);
    }

    #[test]
    fn missing_field_is_invalid_argument() {
        let mut clean = StorageModel::new();
        clean.set_field("imageWidth", "64");
        clean.set_field("imageHeight", "32");
        let err = image_info_from_model(&clean).unwrap_err();
        assert_eq!(err.code(), ErrorCode::InvalidArgument);
        assert_eq!(
            err.message(),
            "memory: missing required field \"tileWidth\""
        );
    }

    #[test]
    fn non_tiled_is_rejected() {
        let mut m = tiled_model();
        m.set_field("tileWidth", "0");
        let err = image_info_from_model(&m).unwrap_err();
        assert_eq!(err.code(), ErrorCode::InvalidArgument);
        assert_eq!(err.message(), "memory: non-tiled images are not supported");
    }

    #[test]
    fn compressed_is_rejected() {
        let mut m = tiled_model();
        m.set_field("compression", "LZW");
        let err = image_info_from_model(&m).unwrap_err();
        assert_eq!(
            err.message(),
            "memory: only uncompressed (compression==None) images are supported"
        );
    }

    #[test]
    fn samples_and_bytes_per_sample_scale_tile_bytes() {
        let mut m = tiled_model();
        m.set_field("samplesPerPixel", "3");
        m.set_field("pixelType", "UInt16");
        let info = image_info_from_model(&m).expect("valid");
        // 16*16*3*2
        assert_eq!(info.tile_bytes, 1536);
        assert_eq!(info.image_pixel_bytes, 4 * 2 * 1536);
    }

    #[test]
    fn pixel_offset_skips_prior_images() {
        let a = tiled_model();
        let mut b = tiled_model();
        b.set_field("imageWidth", "128"); // wider image -> more columns
        let infos = vec![
            image_info_from_model(&a).unwrap(),
            image_info_from_model(&b).unwrap(),
        ];
        assert_eq!(image_pixel_offset(&infos, 0, 12), 12);
        assert_eq!(
            image_pixel_offset(&infos, 1, 12),
            12 + infos[0].image_pixel_bytes
        );
    }
}
