//! Zarr-style single-file container header: shape/chunks/dtype/compressor.
//!
//! Mirrors `libptiff/include/ptiff/io/backend/zarr/zarr_document.hpp` /
//! `libptiff/src/io/backend/zarr/zarr_document.cpp`. Layout of a Zarr
//! document produced/consumed by [`super::ZarrBackend`]:
//!
//! ```text
//! [0 .. 8)     JSON header byte length H, little-endian u64 (the seek header)
//! [8 .. 8+H)   JSON array header text (shape/chunks/dtype/compressor)
//! [8+H ..)     chunk slots, each `slot_size` bytes:
//!                [0 .. 4)   actual chunk length L, little-endian u32
//!                [4 .. 4+L) chunk bytes (compressed via the header's
//!                           compressor, or raw)
//!                [4+L .. slot_size) unused padding
//! ```
//!
//! Chunks are addressed linearly (row-major, same order as the memory
//! backend's tiles). For this phase chunk == tile, so chunk `i` shares the
//! tile's geometry. Fixed slot sizes make offsets deterministic without a
//! per-chunk index (`slot_size` is derived from the format's worst-case
//! compressed bound, see [`super::codec`]). This is a deliberately compact,
//! self-describing Zarr subset -- real JSON metadata + real chunk
//! compression, packed into the single-stream interface the `StorageBackend`
//! API requires.

use crate::io::BinaryReader;
use crate::io::StorageModel;
use crate::pixel_type::PixelType;
use crate::tile::{TileExtent, TileLayout};
use crate::{Error, Result};

use super::codec::payload_bound;

/// Byte length of the seek header that precedes the JSON array header.
pub const HEADER_SIZE: u64 = std::mem::size_of::<u64>() as u64;

/// Upper bound on a plausible JSON header byte length: guards against a
/// corrupt seek header turning into a huge allocation (mirrors the C++
/// `jsonLen > (1ull << 24)` check).
const MAX_HEADER_BYTES: u64 = 1 << 24;

/// Compressor ids understood by this container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compressor {
    /// Chunks are stored raw (uncompressed).
    None,
    /// Chunks are compressed with zstd.
    Zstd,
    /// Chunks are compressed with zlib (RFC 1950).
    Zlib,
}

/// Header+chunk geometry of a Zarr document.
#[derive(Debug, Clone)]
pub struct ZarrLayout {
    /// Reconstructed flat per-image storage fields (populated by
    /// [`parse_header`]; left empty by [`build_layout`], mirroring the C++
    /// oracle -- `serialize_model`/`open_image_sink` only consume the
    /// header/layout/geometry fields below).
    pub image: StorageModel,
    /// Serialized JSON array header (without the seek header).
    pub header: Vec<u8>,
    /// Tile geometry (chunk == tile for this phase).
    pub layout: TileLayout,
    /// The chunk compressor.
    pub compressor: Compressor,
    /// Uncompressed bytes per chunk.
    pub chunk_bytes: u32,
    /// Fixed on-disk size of one chunk slot (>= 4 + `chunk_bytes`).
    pub slot_size: u32,
}

/// Maps a ptiff `pixelType` storage-field value to a Zarr dtype string, or
/// `None` if unsupported.
fn pixel_type_to_dtype(pt: &str) -> Option<&'static str> {
    match pt {
        "UInt8" => Some("|u1"),
        "UInt16" => Some("<u2"),
        "UInt32" => Some("<u4"),
        "Float32" => Some("<f4"),
        "Float64" => Some("<f8"),
        _ => None,
    }
}

/// Maps a Zarr dtype string to a ptiff `pixelType`, or `None` if unrecognized.
fn dtype_to_pixel_type(dtype: &str) -> Option<&'static str> {
    match dtype {
        "|u1" => Some("UInt8"),
        "<u2" => Some("UInt16"),
        "<u4" => Some("UInt32"),
        "<f4" => Some("Float32"),
        "<f8" => Some("Float64"),
        _ => None,
    }
}

/// Maps a known `pixelType` string (already validated via
/// [`pixel_type_to_dtype`]/[`dtype_to_pixel_type`]) to the [`PixelType`] enum.
fn pixel_type_from_str(pt: &str) -> PixelType {
    match pt {
        "UInt16" => PixelType::UInt16,
        "UInt32" => PixelType::UInt32,
        "Float32" => PixelType::Float32,
        "Float64" => PixelType::Float64,
        _ => PixelType::UInt8,
    }
}

/// Parses `value` (from the "None"/"zstd"/"zlib" storage field, or the JSON
/// header's "none"/"zstd"/"zlib" spelling) into a [`Compressor`].
pub fn parse_compressor(value: &str) -> Result<Compressor> {
    // Accept the storage-field spelling ("None") and the JSON-header spelling ("none").
    match value {
        "None" | "none" => Ok(Compressor::None),
        "zstd" => Ok(Compressor::Zstd),
        "zlib" => Ok(Compressor::Zlib),
        _ => Err(Error::invalid_argument(
            "zarr: unsupported compression (expected None/zstd/zlib)",
        )),
    }
}

/// Parses a numeric field value, mapping any parse failure to an
/// `InvalidArgument` naming `field`.
fn parse_numeric<T: std::str::FromStr>(value: &str, field: &str) -> Result<T> {
    value
        .parse::<T>()
        .map_err(|_| Error::invalid_argument(format!("zarr: non-numeric field \"{field}\"")))
}

/// Builds the Zarr JSON array header from a flat per-image `model`.
///
/// `model` must carry `imageWidth`/`imageHeight`/`tileWidth`/`tileHeight`/
/// `samplesPerPixel`/`pixelType` and optionally `compression`
/// (`None`/`zstd`/`zlib`).
///
/// # Errors
///
/// Returns [`crate::ErrorCode::InvalidArgument`] if a required field is
/// missing, or the pixel type/compressor is unsupported.
pub fn build_layout(model: &StorageModel) -> Result<ZarrLayout> {
    let w = model.field("imageWidth").ok();
    let h = model.field("imageHeight").ok();
    let b = model.field("samplesPerPixel").ok();
    let pt = model.field("pixelType").ok();
    let tw = model.field("tileWidth").ok();
    let th = model.field("tileHeight").ok();
    // Note: like the C++ oracle, each individual lookup's more specific
    // "missing field X" detail is discarded here in favor of this single
    // generic message once any required field is absent.
    let (w, h, b, pt, tw, th) = match (w, h, b, pt, tw, th) {
        (Some(w), Some(h), Some(b), Some(pt), Some(tw), Some(th)) => (w, h, b, pt, tw, th),
        _ => {
            return Err(Error::invalid_argument(
                "zarr: missing geometry/frame fields",
            ))
        }
    };

    let dtype = pixel_type_to_dtype(pt)
        .ok_or_else(|| Error::invalid_argument("zarr: unsupported pixelType"))?;

    let mut compressor = Compressor::None;
    if let Ok(c) = model.field("compression") {
        compressor = parse_compressor(c)?;
    }

    let w_u32: u32 = parse_numeric(w, "imageWidth")?;
    let h_u32: u32 = parse_numeric(h, "imageHeight")?;
    let tw_u32: u32 = parse_numeric(tw, "tileWidth")?;
    let th_u32: u32 = parse_numeric(th, "tileHeight")?;
    let spp_u64: u64 = parse_numeric(b, "samplesPerPixel")?;

    let pixel_type = pixel_type_from_str(pt);
    let bps = pixel_type.bytes_per_sample() as u64;
    let chunk_bytes = (u64::from(tw_u32) * u64::from(th_u32) * spp_u64 * bps) as u32;
    let slot_size = (4u64 + payload_bound(compressor, chunk_bytes as usize) as u64) as u32;

    let compressor_str = match compressor {
        Compressor::Zstd => "zstd",
        Compressor::Zlib => "zlib",
        Compressor::None => "none",
    };
    let mut j = serde_json::json!({
        "compressor": compressor_str,
        "dtype": dtype,
    });
    if b == "1" {
        j["shape"] = serde_json::json!([h_u32, w_u32]);
    } else {
        j["shape"] = serde_json::json!([spp_u64, h_u32, w_u32]);
    }
    j["chunks"] = serde_json::json!([th_u32, tw_u32]);

    let header = serde_json::to_vec(&j)
        .map_err(|e| Error::unknown(format!("zarr: encode header failed: {e}")))?;

    Ok(ZarrLayout {
        image: StorageModel::new(),
        header,
        layout: TileLayout::new(TileExtent::new(tw_u32, th_u32), w_u32, h_u32, 1),
        compressor,
        chunk_bytes,
        slot_size,
    })
}

/// Reads a required `u32` array element at `index`, or an `InvalidArgument`
/// naming `what` if `index` is out of bounds or not a number.
fn json_u32_at(array: &[serde_json::Value], index: usize, what: &str) -> Result<u32> {
    array
        .get(index)
        .and_then(serde_json::Value::as_u64)
        .and_then(|n| u32::try_from(n).ok())
        .ok_or_else(|| Error::invalid_argument(format!("zarr: malformed {what}")))
}

/// Reconstructs a flat per-image [`StorageModel`] from a JSON array header
/// (round-trips with [`build_layout`]).
///
/// # Errors
///
/// Returns [`crate::ErrorCode::InvalidArgument`] if the JSON isn't a Zarr
/// array header.
pub fn parse_header(json_header: &[u8]) -> Result<ZarrLayout> {
    let text = std::str::from_utf8(json_header)
        .map_err(|_| Error::invalid_argument("zarr: malformed JSON array header"))?;
    let j: serde_json::Value = serde_json::from_str(text)
        .map_err(|_| Error::invalid_argument("zarr: malformed JSON array header"))?;

    let shape = j.get("shape").and_then(serde_json::Value::as_array);
    let chunks = j.get("chunks").and_then(serde_json::Value::as_array);
    let dtype = j.get("dtype").and_then(serde_json::Value::as_str);
    let (shape, chunks, dtype) = match (shape, chunks, dtype) {
        (Some(s), Some(c), Some(d)) => (s, c, d),
        _ => {
            return Err(Error::invalid_argument(
                "zarr: header missing shape/chunks/dtype",
            ))
        }
    };

    let ndim = shape.len();
    if ndim != 2 && ndim != 3 {
        return Err(Error::invalid_argument("zarr: unsupported shape rank"));
    }
    let w = json_u32_at(shape, ndim - 1, "shape")?;
    let h = json_u32_at(shape, ndim - 2, "shape")?;
    // The chunks array is always rank 2 (tileHeight, tileWidth), independent
    // of the shape's rank; index from its own length rather than `ndim` to
    // stay in bounds for a 3-dim (multi-band) shape.
    let cndim = chunks.len();
    if cndim < 2 {
        return Err(Error::invalid_argument("zarr: malformed chunks"));
    }
    let tw = json_u32_at(chunks, cndim - 1, "chunks")?;
    let th = json_u32_at(chunks, cndim - 2, "chunks")?;
    let bands = if ndim == 3 {
        json_u32_at(shape, 0, "shape")?
    } else {
        1
    };

    let pt = dtype_to_pixel_type(dtype)
        .ok_or_else(|| Error::invalid_argument("zarr: unrecognized dtype"))?;

    let mut compressor = Compressor::None;
    if let Some(c) = j.get("compressor").and_then(serde_json::Value::as_str) {
        compressor = parse_compressor(c)?;
    }

    let pixel_type = pixel_type_from_str(pt);
    let bps = pixel_type.bytes_per_sample() as u64;
    let chunk_bytes = (u64::from(tw) * u64::from(th) * u64::from(bands) * bps) as u32;
    let slot_size = (4u64 + payload_bound(compressor, chunk_bytes as usize) as u64) as u32;

    let mut image = StorageModel::new();
    image.set_field("imageWidth", w.to_string());
    image.set_field("imageHeight", h.to_string());
    image.set_field("tileWidth", tw.to_string());
    image.set_field("tileHeight", th.to_string());
    image.set_field("samplesPerPixel", bands.to_string());
    image.set_field("pixelType", pt);
    image.set_field(
        "compression",
        match compressor {
            Compressor::Zstd => "zstd",
            Compressor::Zlib => "zlib",
            Compressor::None => "None",
        },
    );

    Ok(ZarrLayout {
        image,
        header: Vec::new(),
        layout: TileLayout::new(TileExtent::new(tw, th), w, h, 1),
        compressor,
        chunk_bytes,
        slot_size,
    })
}

/// Reads the seek header + JSON array header from `reader`.
///
/// # Errors
///
/// Returns [`crate::ErrorCode::InvalidArgument`] on a truncated seek header,
/// an implausible header byte length, a truncated header, or a malformed
/// header (see [`parse_header`]).
pub fn read_document(reader: &mut dyn BinaryReader) -> Result<ZarrLayout> {
    reader.seek(0)?;

    let mut header_size_bytes = [0u8; HEADER_SIZE as usize];
    let read_size = reader.read(&mut header_size_bytes)?;
    if read_size != header_size_bytes.len() {
        return Err(Error::invalid_argument("zarr: truncated seek header"));
    }
    let json_len = u64::from_le_bytes(header_size_bytes);
    if json_len == 0 || json_len > MAX_HEADER_BYTES {
        return Err(Error::invalid_argument("zarr: implausible header length"));
    }

    let mut json = vec![0u8; json_len as usize];
    let read_json = reader.read(&mut json)?;
    if read_json != json.len() {
        return Err(Error::invalid_argument("zarr: truncated header"));
    }

    let mut parsed = parse_header(&json)?;
    parsed.header = json;
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::MemoryBinaryReader;
    use crate::ErrorCode;

    fn image_model(compression: &str) -> StorageModel {
        let mut m = StorageModel::new();
        m.set_field("imageWidth", "32");
        m.set_field("imageHeight", "32");
        m.set_field("tileWidth", "16");
        m.set_field("tileHeight", "16");
        m.set_field("samplesPerPixel", "1");
        m.set_field("pixelType", "UInt8");
        m.set_field("compression", compression);
        m
    }

    #[test]
    fn parse_compressor_accepts_storage_and_json_spellings() {
        assert!(matches!(parse_compressor("None"), Ok(Compressor::None)));
        assert!(matches!(parse_compressor("none"), Ok(Compressor::None)));
        assert!(matches!(parse_compressor("zstd"), Ok(Compressor::Zstd)));
        assert!(matches!(parse_compressor("zlib"), Ok(Compressor::Zlib)));
    }

    #[test]
    fn parse_compressor_rejects_unknown() {
        let err = parse_compressor("lz4").unwrap_err();
        assert_eq!(err.code(), ErrorCode::InvalidArgument);
    }

    #[test]
    fn build_layout_derives_geometry_and_header() {
        let model = image_model("zstd");
        let layout = build_layout(&model).expect("build_layout");
        assert_eq!(layout.layout.image_width, 32);
        assert_eq!(layout.layout.image_height, 32);
        assert_eq!(layout.layout.tile_size, TileExtent::new(16, 16));
        assert_eq!(layout.compressor, Compressor::Zstd);
        assert_eq!(layout.chunk_bytes, 16 * 16);
        assert!(layout.slot_size >= 4 + layout.chunk_bytes);
        assert!(!layout.header.is_empty());
    }

    #[test]
    fn build_layout_missing_field_is_invalid_argument() {
        let mut m = StorageModel::new();
        m.set_field("imageWidth", "32");
        let err = build_layout(&m).unwrap_err();
        assert_eq!(err.code(), ErrorCode::InvalidArgument);
        assert_eq!(err.message(), "zarr: missing geometry/frame fields");
    }

    #[test]
    fn build_layout_unsupported_pixel_type_is_invalid_argument() {
        let mut m = image_model("None");
        m.set_field("pixelType", "Int8");
        let err = build_layout(&m).unwrap_err();
        assert_eq!(err.code(), ErrorCode::InvalidArgument);
        assert_eq!(err.message(), "zarr: unsupported pixelType");
    }

    #[test]
    fn build_layout_unsupported_compression_is_invalid_argument() {
        let m = image_model("brotli");
        let err = build_layout(&m).unwrap_err();
        assert_eq!(err.code(), ErrorCode::InvalidArgument);
    }

    #[test]
    fn parse_header_round_trips_build_layout() {
        let model = image_model("zlib");
        let built = build_layout(&model).expect("build_layout");
        let parsed = parse_header(&built.header).expect("parse_header");
        assert_eq!(parsed.image.field("imageWidth").unwrap(), "32");
        assert_eq!(parsed.image.field("tileWidth").unwrap(), "16");
        assert_eq!(parsed.image.field("pixelType").unwrap(), "UInt8");
        assert_eq!(parsed.image.field("compression").unwrap(), "zlib");
        assert_eq!(parsed.compressor, Compressor::Zlib);
        assert_eq!(parsed.chunk_bytes, built.chunk_bytes);
        assert_eq!(parsed.slot_size, built.slot_size);
    }

    #[test]
    fn parse_header_rejects_malformed_json() {
        let err = parse_header(b"not json").unwrap_err();
        assert_eq!(err.code(), ErrorCode::InvalidArgument);
    }

    #[test]
    fn parse_header_rejects_missing_keys() {
        let err = parse_header(br#"{"dtype":"|u1"}"#).unwrap_err();
        assert_eq!(err.code(), ErrorCode::InvalidArgument);
        assert_eq!(err.message(), "zarr: header missing shape/chunks/dtype");
    }

    #[test]
    fn parse_header_rejects_unsupported_rank() {
        let err = parse_header(br#"{"shape":[1],"chunks":[1],"dtype":"|u1"}"#).unwrap_err();
        assert_eq!(err.code(), ErrorCode::InvalidArgument);
        assert_eq!(err.message(), "zarr: unsupported shape rank");
    }

    #[test]
    fn parse_header_rejects_unrecognized_dtype() {
        let err = parse_header(br#"{"shape":[2,2],"chunks":[2,2],"dtype":"<i8"}"#).unwrap_err();
        assert_eq!(err.code(), ErrorCode::InvalidArgument);
        assert_eq!(err.message(), "zarr: unrecognized dtype");
    }

    #[test]
    fn read_document_round_trips_written_header() {
        let model = image_model("None");
        let built = build_layout(&model).expect("build_layout");
        let mut bytes = (built.header.len() as u64).to_le_bytes().to_vec();
        bytes.extend_from_slice(&built.header);

        let mut reader = MemoryBinaryReader::from_vec(bytes);
        let doc = read_document(&mut reader).expect("read_document");
        assert_eq!(doc.image.field("imageWidth").unwrap(), "32");
        assert_eq!(doc.header, built.header);
    }

    #[test]
    fn read_document_truncated_seek_header_is_invalid_argument() {
        let mut reader = MemoryBinaryReader::from_vec(vec![1, 2, 3]);
        let err = read_document(&mut reader).unwrap_err();
        assert_eq!(err.code(), ErrorCode::InvalidArgument);
        assert_eq!(err.message(), "zarr: truncated seek header");
    }

    #[test]
    fn read_document_zero_header_length_is_invalid_argument() {
        let bytes = 0u64.to_le_bytes().to_vec();
        let mut reader = MemoryBinaryReader::from_vec(bytes);
        let err = read_document(&mut reader).unwrap_err();
        assert_eq!(err.code(), ErrorCode::InvalidArgument);
        assert_eq!(err.message(), "zarr: implausible header length");
    }

    #[test]
    fn read_document_implausible_header_length_is_invalid_argument() {
        let bytes = (MAX_HEADER_BYTES + 1).to_le_bytes().to_vec();
        let mut reader = MemoryBinaryReader::from_vec(bytes);
        let err = read_document(&mut reader).unwrap_err();
        assert_eq!(err.code(), ErrorCode::InvalidArgument);
        assert_eq!(err.message(), "zarr: implausible header length");
    }

    #[test]
    fn read_document_truncated_header_is_invalid_argument() {
        let mut bytes = 100u64.to_le_bytes().to_vec();
        bytes.extend_from_slice(b"{\"shape\":");
        let mut reader = MemoryBinaryReader::from_vec(bytes);
        let err = read_document(&mut reader).unwrap_err();
        assert_eq!(err.code(), ErrorCode::InvalidArgument);
        assert_eq!(err.message(), "zarr: truncated header");
    }
}
