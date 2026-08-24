//! Data-level OpenEXR reader/writer over the core's byte transport.
//!
//! Mirrors the C++ `openexr::openexr_document` (see
//! `libptiff/src/io/backend/openexr/openexr_document.cpp`) replacing the
//! `Imf::` C++ API with the pure-Rust `exr` crate.
//!
//! Only `Float32` and `UInt32` pixel types are supported (the two that map 1:1
//! onto a native OpenEXR `F32`/`U32` channel type), with `samplesPerPixel` 1
//! ("Y"), 3 ("R","G","B") or 4 ("R","G","B","A"). The whole image is always a
//! single scanline-block-per-row image (uncompressed); genuine multi-tile
//! OpenEXR chunking is deferred, exactly like the C++ oracle.

use std::io::{BufReader, BufWriter};

use exr::block::writer::ChunksWriter;
use exr::block::{read as exr_read, write as exr_write, UncompressedBlock};
use exr::compression::Compression;
use exr::meta::attribute::{ChannelDescription, LineOrder, SampleType};
use exr::meta::header::Header;
use exr::meta::BlockDescription;
use exr::prelude::SmallVec;

use crate::io::{BinaryReader, BinaryWriter, StorageModel};
use crate::pixel_type::PixelType;
use crate::{Error, Result};

use super::io_adapter::{ReaderAdapter, WriterAdapter};

/// Geometry + type + channel layout of one OpenEXR document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenExrImageInfo {
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// Samples per pixel: 1 ("Y"), 3 ("R","G","B") or 4 ("R","G","B","A").
    pub samples_per_pixel: u32,
    /// Pixel type: `Float32` or `UInt32`.
    pub pixel_type: PixelType,
}

/// Channel name list for a given samples-per-pixel. Mirrors the C++ oracle.
pub fn channel_names_for(samples_per_pixel: u32) -> Result<[&'static str; 4]> {
    match samples_per_pixel {
        1 => Ok(["Y", "", "", ""]),
        3 => Ok(["R", "G", "B", ""]),
        4 => Ok(["R", "G", "B", "A"]),
        _ => Err(Error::invalid_argument(
            "openexr: only samplesPerPixel 1 (Y), 3 (RGB) or 4 (RGBA) are supported",
        )),
    }
}

/// Returns the (alphabetically sorted) OpenEXR channel names and, for each
/// such channel, the 0-based pixel slot it maps to in our interleaved
/// `R,G,B,A` buffer. OpenEXR requires channel names in a channel list to be
/// sorted alphabetically (e.g. `B,G,R` for RGB), while our pixel bytes stay in
/// the conventional `R,G,B,..` order — so we keep a slot mapping to preserve
/// the byte-for-byte round trip.
fn channel_layout(samples_per_pixel: u32) -> Result<(Vec<&'static str>, Vec<usize>)> {
    match samples_per_pixel {
        1 => Ok((vec!["Y"], vec![0])),
        3 => Ok((vec!["B", "G", "R"], vec![2, 1, 0])),
        4 => Ok((vec!["A", "B", "G", "R"], vec![3, 2, 1, 0])),
        _ => Err(Error::invalid_argument(
            "openexr: only samplesPerPixel 1 (Y), 3 (RGB) or 4 (RGBA) are supported",
        )),
    }
}

/// Maps a [`PixelType`] to the native OpenEXR [`SampleType`], or errors.
pub fn to_exr_sample_type(pixel_type: PixelType) -> Result<SampleType> {
    match pixel_type {
        PixelType::Float32 => Ok(SampleType::F32),
        PixelType::UInt32 => Ok(SampleType::U32),
        _ => Err(Error::invalid_argument(
            "openexr: only Float32 and UInt32 pixel types are supported this phase",
        )),
    }
}

/// Maps an OpenEXR [`SampleType`] to a [`PixelType`], or errors.
pub fn from_exr_sample_type(sample_type: SampleType) -> Result<PixelType> {
    match sample_type {
        SampleType::F32 => Ok(PixelType::Float32),
        SampleType::U32 => Ok(PixelType::UInt32),
        _ => Err(Error::invalid_argument(
            "openexr: only Float32 and UInt32 pixel types are supported this phase",
        )),
    }
}

/// Extracts a required u32 field from `model` with an `openexr:`-prefixed error.
fn required_u32(model: &StorageModel, field: &str) -> Result<u32> {
    let value = model
        .field(field)
        .map_err(|_| Error::invalid_argument(format!("openexr: missing {field}")))?;
    value
        .parse()
        .map_err(|_| Error::invalid_argument(format!("openexr: non-numeric {field}")))
}

/// Builds [`OpenExrImageInfo`] from a flat per-image `model`, validating the
/// phase-1 constraints (pixel type + samples-per-pixel, optional
/// tileWidth/tileHeight == image size, compression == None).
pub fn image_info_from_model(model: &StorageModel) -> Result<OpenExrImageInfo> {
    let width = required_u32(model, "imageWidth")?;
    let height = required_u32(model, "imageHeight")?;
    let samples_per_pixel = required_u32(model, "samplesPerPixel")?;
    channel_names_for(samples_per_pixel)?;

    let pixel_type_field = model
        .field("pixelType")
        .map_err(|_| Error::invalid_argument("openexr: missing pixelType"))?;
    let pixel_type = pixel_type_from_field(pixel_type_field)
        .ok_or_else(|| Error::invalid_argument("openexr: unrecognized pixelType"))?;
    to_exr_sample_type(pixel_type)?;

    if let Ok(tw) = model.field("tileWidth") {
        let tw: u32 = tw
            .parse()
            .map_err(|_| Error::invalid_argument("openexr: non-numeric tileWidth"))?;
        if tw != width {
            return Err(Error::invalid_argument(
                "openexr: tileWidth must equal imageWidth (whole-image tile only)",
            ));
        }
    }
    if let Ok(th) = model.field("tileHeight") {
        let th: u32 = th
            .parse()
            .map_err(|_| Error::invalid_argument("openexr: non-numeric tileHeight"))?;
        if th != height {
            return Err(Error::invalid_argument(
                "openexr: tileHeight must equal imageHeight (whole-image tile only)",
            ));
        }
    }
    if let Ok(compression) = model.field("compression") {
        if compression != "None" && compression != "none" {
            return Err(Error::invalid_argument(
                "openexr: only uncompressed (compression==None) images are supported",
            ));
        }
    }

    Ok(OpenExrImageInfo {
        width,
        height,
        samples_per_pixel,
        pixel_type,
    })
}

/// Builds the Scene-convention model (root with one image child) from
/// [`OpenExrImageInfo`].
pub fn model_from_image_info(info: &OpenExrImageInfo) -> StorageModel {
    let mut image = StorageModel::new();
    image.set_field("imageWidth", info.width.to_string());
    image.set_field("imageHeight", info.height.to_string());
    image.set_field("samplesPerPixel", info.samples_per_pixel.to_string());
    image.set_field(
        "pixelType",
        pixel_type_field_string(info.pixel_type).to_owned(),
    );
    let mut root = StorageModel::new();
    root.add_child(image);
    root
}

/// Builds the OpenEXR [`Header`] for `info` (uncompressed scanlines, channels
/// in the alphabetically sorted order OpenEXR requires).
fn build_header(info: &OpenExrImageInfo, name: &str) -> Result<Header> {
    let sample_type = to_exr_sample_type(info.pixel_type)?;
    let (names, _slots) = channel_layout(info.samples_per_pixel)?;
    let mut channels = SmallVec::new();
    for name in &names {
        channels.push(ChannelDescription::new(*name, sample_type, true));
    }
    let header = Header::new(
        name.into(),
        (info.width as usize, info.height as usize),
        channels,
    )
    .with_encoding(
        Compression::Uncompressed,
        BlockDescription::ScanLines,
        LineOrder::Increasing,
    );
    Ok(header)
}

/// Parses the CamelCase `pixelType` model-field value into a [`PixelType`].
fn pixel_type_from_field(value: &str) -> Option<PixelType> {
    match value {
        "Float32" => Some(PixelType::Float32),
        "UInt32" => Some(PixelType::UInt32),
        _ => None,
    }
}

/// Serializes a [`PixelType`] to the CamelCase model-field value.
fn pixel_type_field_string(pixel_type: PixelType) -> &'static str {
    match pixel_type {
        PixelType::Float32 => "Float32",
        PixelType::UInt32 => "UInt32",
        _ => "Float32", // unreachable: callers only pass validated types
    }
}

/// Bytes per sample for the supported pixel types.
fn bytes_per_sample(pixel_type: PixelType) -> usize {
    match pixel_type {
        PixelType::UInt8 => 1,
        PixelType::UInt16 => 2,
        PixelType::UInt32 => 4,
        PixelType::Float32 => 4,
        PixelType::Float64 => 8,
    }
}

/// Reads only the OpenEXR header (metadata) from `reader`, rewinding to byte 0
/// first. Returns the decoded [`OpenExrImageInfo`].
pub fn read_header_info(reader: &mut dyn BinaryReader) -> Result<OpenExrImageInfo> {
    reader.seek(0)?;
    let adapter = BufReader::new(ReaderAdapter::new(reader));
    let exr_reader = block_read(adapter, true)?;
    into_info_from_meta(exr_reader)
}

/// Convenience around `exr::block::read`, keeping the borrowed adapter type
/// opaque.
fn block_read<R: std::io::Read + std::io::Seek>(
    buffered: R,
    pedantic: bool,
) -> Result<exr::block::reader::Reader<R>> {
    exr_read(buffered, pedantic).map_err(|e| Error::invalid_argument(format!("openexr: {e}")))
}

/// Extracts [`OpenExrImageInfo`] from an `exr` reader's header.
fn into_info_from_meta<R: std::io::Read + std::io::Seek>(
    reader: exr::block::reader::Reader<R>,
) -> Result<OpenExrImageInfo> {
    let headers = reader.headers();
    let header = headers
        .first()
        .ok_or_else(|| Error::invalid_argument("openexr: no header found"))?;
    let bounds = header.data_window();
    let width = bounds.size.x() as u32;
    let height = bounds.size.y() as u32;

    let names = header
        .channels
        .list
        .iter()
        .map(|c| c.name.clone())
        .collect::<Vec<_>>();
    let is_gray = names.iter().any(|n| n == "Y") && !names.iter().any(|n| n == "R");
    let samples_per_pixel = if is_gray {
        1
    } else if names.iter().any(|n| n == "A") {
        4
    } else if names.iter().any(|n| n == "R") {
        3
    } else {
        return Err(Error::invalid_argument(
            "openexr: unsupported channel layout",
        ));
    };

    let expected = channel_names_for(samples_per_pixel)?;
    let sample_type = header
        .channels
        .list
        .iter()
        .find_map(|c| {
            if c.name == *expected[0] {
                Some(c.sample_type)
            } else {
                None
            }
        })
        .ok_or_else(|| Error::invalid_argument("openexr: missing expected channel"))?;
    let pixel_type = from_exr_sample_type(sample_type)?;

    Ok(OpenExrImageInfo {
        width,
        height,
        samples_per_pixel,
        pixel_type,
    })
}

/// Writes a complete, uncompressed `.exr` document for `info` from `pixels`,
/// which must hold the raw interleaved pixel bytes in native endianness
/// (`R,G,B,A` per pixel, `samplesPerPixel * bytesPerSample` per pixel), plus a
/// used line/byte layout that exactly mirrors the C++ `ImageSink` frame buffer.
///
/// `pixels` must have length `width * height * samplesPerPixel * bytesPerSample`.
///
/// # Errors
///
/// Returns [`crate::ErrorCode::InvalidArgument`] if `pixels` has the wrong
/// length or the OpenEXR write fails.
pub fn encode_image(
    writer: &mut dyn BinaryWriter,
    info: &OpenExrImageInfo,
    pixels: &[u8],
) -> Result<()> {
    let expected = expected_pixel_bytes(info)?;
    if pixels.len() != expected {
        return Err(Error::invalid_argument(
            "openexr: tile data size does not match image geometry",
        ));
    }

    let header = build_header(info, "image")?;
    let headers = SmallVec::from_elem(header, 1);
    let (_names, slots) = channel_layout(info.samples_per_pixel)?;

    {
        let adapter = BufWriter::new(WriterAdapter::new(writer));
        exr_write(adapter, headers.clone(), true, |meta, chunk_writer| {
            let blocks = meta.collect_ordered_blocks(|block_index| {
                let channels = &meta.headers[block_index.layer].channels;
                UncompressedBlock::from_lines(channels, block_index, |line_mut| {
                    fill_line_from_interleaved(info, pixels, &slots, line_mut);
                })
            });
            // `compress_all_blocks_*` consumes an owned `ChunksWriter`;
            // `on_progress` hands us one that borrows the writer.
            chunk_writer
                .on_progress(|_| {})
                .compress_all_blocks_sequential(&meta, blocks)
                .expect("block compress");
            Ok(())
        })
        .map_err(|e| Error::invalid_argument(format!("openexr: pixel write failed: {e}")))?;
    }

    Ok(())
}

/// Fills one mutable scanline block from the interleaved, native-endian
/// `pixels` buffer.
fn fill_line_from_interleaved(
    info: &OpenExrImageInfo,
    pixels: &[u8],
    slots: &[usize],
    line_mut: exr::block::lines::LineRefMut<'_>,
) {
    // Map the row/block channel (in alphabetical OpenEXR order) to the
    // conventional R,G,B pixel slot in our interleaved buffer.
    let c = slots[line_mut.location.channel];
    let y = line_mut.location.position.y();
    let spp = info.samples_per_pixel as usize;
    let width = info.width as usize;
    let bps = bytes_per_sample(info.pixel_type);
    // Write each sample of this line from the interleaved pixel buffer. The
    // line is one whole image row (uncompressed scanlines), so `x` is the
    // horizontal pixel index and the global pixel is `(y * width + x)`.
    match info.pixel_type {
        PixelType::Float32 => {
            let _ = line_mut.write_samples::<f32>(|x| {
                let off = ((y * width + x) * spp + c) * bps;
                let mut a = [0u8; 4];
                a.copy_from_slice(&pixels[off..off + 4]);
                f32::from_ne_bytes(a)
            });
        }
        PixelType::UInt32 => {
            let _ = line_mut.write_samples::<u32>(|x| {
                let off = ((y * width + x) * spp + c) * bps;
                let mut a = [0u8; 4];
                a.copy_from_slice(&pixels[off..off + 4]);
                u32::from_ne_bytes(a)
            });
        }
        _ => unreachable!("openexr only supports Float32/UInt32 this phase"),
    }
}

/// Number of whole-image pixel bytes for `info`.
pub fn expected_pixel_bytes(info: &OpenExrImageInfo) -> Result<usize> {
    let w = usize::try_from(info.width)
        .map_err(|_| Error::invalid_argument("openexr: width too large"))?;
    let h = usize::try_from(info.height)
        .map_err(|_| Error::invalid_argument("openexr: height too large"))?;
    let spp = usize::try_from(info.samples_per_pixel)
        .map_err(|_| Error::invalid_argument("openexr: spp too large"))?;
    let bps = bytes_per_sample(info.pixel_type);
    w.checked_mul(h)
        .and_then(|n| n.checked_mul(spp))
        .and_then(|n| n.checked_mul(bps))
        .ok_or_else(|| Error::invalid_argument("openexr: image too large"))
}

/// Reads and decodes a complete `.exr` document from `reader`, rewinding to
/// byte 0 first, and returns the raw interleaved pixel bytes in native
/// endianness (exactly as the C++ `ImageSource` frame buffer would).
///
/// # Errors
///
/// Returns [`crate::ErrorCode::InvalidArgument`] if the document cannot be
/// parsed or its layout does not match `info`.
pub fn decode_image(reader: &mut dyn BinaryReader, info: &OpenExrImageInfo) -> Result<Vec<u8>> {
    use exr::block::reader::ChunksReader;

    reader.seek(0)?;
    let adapter = BufReader::new(ReaderAdapter::new(reader));
    let buffered = block_read(adapter, true)?;

    let expected = expected_pixel_bytes(info)?;
    let mut out = vec![0u8; expected];
    let info_ref = info.clone();
    let (_names, slots) = channel_layout(info.samples_per_pixel)?;

    buffered
        .all_chunks(true)
        .map_err(|e| Error::invalid_argument(format!("openexr: {e}")))?
        .decompress_sequential(true, |meta, block| {
            let channels = &meta.headers[block.index.layer].channels;
            for line in block.lines(channels) {
                fill_line_into_interleaved(&info_ref, &mut out, &slots, line);
            }
            Ok(())
        })
        .map_err(|e| Error::invalid_argument(format!("openexr: pixel read failed: {e}")))?;

    Ok(out)
}

/// Copies one decoded scanline block into the interleaved, native-endian
/// `out` buffer.
fn fill_line_into_interleaved(
    info: &OpenExrImageInfo,
    out: &mut [u8],
    slots: &[usize],
    line: exr::block::lines::LineRef<'_>,
) {
    let c = slots[line.location.channel];
    let y = line.location.position.y();
    let spp = info.samples_per_pixel as usize;
    let width = info.width as usize;
    let bps = bytes_per_sample(info.pixel_type);
    match info.pixel_type {
        PixelType::Float32 => {
            for (x, sample) in line.read_samples::<f32>().enumerate() {
                if let Ok(sample) = sample {
                    let off = ((y * width + x) * spp + c) * bps;
                    out[off..off + 4].copy_from_slice(&sample.to_ne_bytes());
                }
            }
        }
        PixelType::UInt32 => {
            for (x, sample) in line.read_samples::<u32>().enumerate() {
                if let Ok(sample) = sample {
                    let off = ((y * width + x) * spp + c) * bps;
                    out[off..off + 4].copy_from_slice(&sample.to_ne_bytes());
                }
            }
        }
        _ => unreachable!("openexr only supports Float32/UInt32 this phase"),
    }
}

/// Writes a structurally valid, uncompressed `.exr` document whose header
/// carries `model`'s geometry. `serializeModel` has no pixel bytes to write, so
/// the pixel payload is zero-filled: the `exr` crate refuses to emit an image
/// with unwritten chunks, so an all-zero scanline payload keeps the file valid
/// and lets a later `deserialize_model` read the geometry back (matching the
/// round-trip contract of the other backends).
///
/// # Errors
///
/// Returns [`crate::ErrorCode::InvalidArgument`] if the model is invalid or the
/// write fails.
pub fn write_header_only(writer: &mut dyn BinaryWriter, model: &StorageModel) -> Result<()> {
    let info = image_info_from_model(model)?;
    let size = expected_pixel_bytes(&info)?;
    let zero_pixels = vec![0u8; size];
    encode_image(writer, &info, &zero_pixels)
        .map_err(|e| Error::invalid_argument(format!("openexr: header write failed: {e}")))
}
