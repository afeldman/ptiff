//! Baseline JPEG codec for TIFF (Compression=7).
//!
//! Mirrors `ptiff::compression::{encodeJpeg, decodeJpeg}` from
//! `src/compression/jpeg.cpp`. The C++ oracle encodes/decode via libjpeg-turbo;
//! here we use the pure-Rust `jpeg-encoder` and `jpeg-decoder` crates so the
//! core stays free of a system-toolchain dependency (same 4:4:4 semantics:
//! no chroma subsampling, 8-bit baseline, spp 1 (grayscale) or 3 (RGB)).
//!
//! This module is only built behind the `tiff-codecs` feature.

use jpeg_decoder::{Decoder, ImageInfo, PixelFormat};
use std::io::Cursor;

use crate::{Error, Result};

/// Encodes `pixels` (8-bit, row-major `width * height * samples_per_pixel`
/// bytes) as a baseline JPEG with the given `quality` in `[0, 100]`.
///
/// Mirrors C++ `encodeJpeg`: only `samples_per_pixel` 1 (JCS_GRAYSCALE) and 3
/// (JCS_RGB) are accepted, the exact pixel byte count is validated up front,
/// and the chroma sampling factor is forced to 1x1 (4:4:4, no chroma
/// subsampling) to match the C++ oracle.
pub fn encode_jpeg(
    pixels: &[u8],
    width: u32,
    height: u32,
    samples_per_pixel: u32,
    quality: u32,
) -> Result<Vec<u8>> {
    if quality > 100 {
        return Err(Error::invalid_argument(
            "encode_jpeg: quality must be in [0, 100]",
        ));
    }
    if samples_per_pixel != 1 && samples_per_pixel != 3 {
        return Err(Error::invalid_argument(
            "encode_jpeg: samplesPerPixel must be 1 or 3",
        ));
    }
    let expected_size = (width as u64)
        .checked_mul(height as u64)
        .and_then(|v| v.checked_mul(samples_per_pixel as u64))
        .ok_or_else(|| Error::invalid_argument("encode_jpeg: dimensions overflow"))?;
    if pixels.len() as u64 != expected_size {
        return Err(Error::invalid_argument(
            "encode_jpeg: pixels size does not match width * height * samplesPerPixel",
        ));
    }
    if width > u16::MAX as u32 || height > u16::MAX as u32 {
        return Err(Error::invalid_argument(
            "encode_jpeg: width/height exceed u16 (JPEG SOF limit)",
        ));
    }

    let color_type = if samples_per_pixel == 1 {
        jpeg_encoder::ColorType::Luma
    } else {
        jpeg_encoder::ColorType::Rgb
    };

    let mut out = Vec::new();
    let mut encoder = jpeg_encoder::Encoder::new(&mut out, quality.clamp(1, 100) as u8);
    // 4:4:4: no chroma subsampling, every component sampled 1x1 (matches the
    // C++ oracle's `comp_info[i].h_samp_factor = v_samp_factor = 1`).
    encoder.set_sampling_factor(jpeg_encoder::SamplingFactor::F_1_1);
    encoder
        .encode(pixels, width as u16, height as u16, color_type)
        .map_err(|e| Error::invalid_argument(format!("encode_jpeg: JPEG encoding failed: {e}")))?;
    Ok(out)
}

/// Decompresses `input` and returns `width * height * samples_per_pixel` raw
/// 8-bit bytes (grayscale for spp 1, RGB for spp 3).
///
/// Mirrors C++ `decodeJpeg`: validates `samples_per_pixel` up front, rejects a
/// decoded stream whose dimensions don't match the expected `width`/`height`
/// (before allocating the full output, guarding against hostile streams that
/// declare far larger dimensions than their small byte count could fill), and
/// validates the decoded component count equals `samples_per_pixel`.
pub fn decode_jpeg(
    input: &[u8],
    width: u32,
    height: u32,
    samples_per_pixel: u32,
) -> Result<Vec<u8>> {
    if samples_per_pixel != 1 && samples_per_pixel != 3 {
        return Err(Error::invalid_argument(
            "decode_jpeg: samplesPerPixel must be 1 or 3",
        ));
    }

    let mut decoder = Decoder::new(Cursor::new(input));
    // Size cap on the decoded buffer (strip/tile is bounded by the TIFF byte
    // range, so at most width*height*spp raw bytes are legitimate).
    let expected_size = (width as u64)
        .checked_mul(height as u64)
        .and_then(|v| v.checked_mul(samples_per_pixel as u64))
        .ok_or_else(|| Error::invalid_argument("decode_jpeg: dimensions overflow"))?;
    let expected_size = usize::try_from(expected_size).map_err(|_| {
        Error::invalid_argument("decode_jpeg: decoded size exceeds host address space")
    })?;
    decoder.set_max_decoding_buffer_size(expected_size);

    // Read the header so we can reject a dimension mismatch before allocating
    // the full decoded image (the C++ oracle does the same).
    decoder.read_info().map_err(|e| {
        Error::invalid_argument(format!("decode_jpeg: JPEG header read failed: {e}"))
    })?;
    let ImageInfo {
        width: stream_width,
        height: stream_height,
        pixel_format,
        ..
    } = decoder
        .info()
        .ok_or_else(|| Error::invalid_argument("decode_jpeg: no JPEG stream metadata available"))?;
    if u32::from(stream_width) != width || u32::from(stream_height) != height {
        return Err(Error::invalid_argument(
            "decode_jpeg: JPEG stream dimensions do not match expected width/height",
        ));
    }
    let decoded_spp = match pixel_format {
        PixelFormat::L8 => 1,
        PixelFormat::RGB24 => 3,
        _ => {
            return Err(Error::invalid_argument(
                "decode_jpeg: unsupported JPEG pixel format (expect 8-bit gray/RGB)",
            ))
        }
    };
    if decoded_spp != samples_per_pixel as usize {
        return Err(Error::invalid_argument(
            "decode_jpeg: decoded component count does not match expected samplesPerPixel",
        ));
    }

    // Re-arm the size cap (read_info may have reset it) and finish decoding,
    // letting the decoder auto-detect the stream's colour transform (baseline
    // 3-component JPEGs are YCbCr-interchange, which converts back to RGB;
    // grayscale passes through). Forcing a specific transform here would
    // mis-reconstruct the channels, so we leave it to the decoder.
    decoder.set_max_decoding_buffer_size(expected_size);
    let decoded = decoder
        .decode()
        .map_err(|e| Error::invalid_argument(format!("decode_jpeg: JPEG decoding failed: {e}")))?;
    if decoded.len() != expected_size {
        return Err(Error::invalid_argument(
            "decode_jpeg: decoded output does not match width * height * samplesPerPixel",
        ));
    }
    Ok(decoded)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grayscale(w: u32, h: u32) -> Vec<u8> {
        // Low-frequency vertical ramp: JPEG lossy errors stay small.
        (0..(w * h) as usize)
            .map(|i| ((i as u32 / w) * 8) as u8)
            .collect()
    }

    #[test]
    fn grayscale_round_trip_is_approximately_lossless() {
        let (w, h) = (32u32, 32u32);
        let px = grayscale(w, h);
        let enc = encode_jpeg(&px, w, h, 1, 90).unwrap();
        let dec = decode_jpeg(&enc, w, h, 1).unwrap();
        assert_eq!(dec.len(), px.len());
        for (a, b) in dec.iter().zip(px.iter()) {
            let d = i32::from(*a) - i32::from(*b);
            assert!(d.abs() <= 30, "lossy offset {d}");
        }
    }

    #[test]
    fn rgb_round_trip_is_approximately_lossless() {
        let (w, h) = (16u32, 16u32);
        // Smooth per-channel ramps: JPEG block artifacts stay small and the
        // test validates the RGB pipeline, not worst-case ringing.
        let mut px = Vec::new();
        for i in 0..(w * h) {
            let x = i % w;
            let y = i / w;
            px.push(((x * 16) % 256) as u8); // R: horizontal ramp
            px.push(((y * 16) % 256) as u8); // G: vertical ramp
            px.push(((((x + y) / 2) * 16) % 256) as u8); // B: diagonal ramp
        }
        let enc = encode_jpeg(&px, w, h, 3, 90).unwrap();
        let dec = decode_jpeg(&enc, w, h, 3).unwrap();
        assert_eq!(dec.len(), px.len());
        for (a, b) in dec.iter().zip(px.iter()) {
            let d = i32::from(*a) - i32::from(*b);
            assert!(d.abs() <= 40, "lossy offset {d}");
        }
    }

    #[test]
    fn quality_bounds_are_validated() {
        let (w, h) = (8u32, 8u32);
        let px = grayscale(w, h);
        assert!(encode_jpeg(&px, w, h, 1, 101).is_err());
        // quality 100 and 0 are accepted (C++ oracle allows [0, 100]).
        assert!(encode_jpeg(&px, w, h, 1, 100).is_ok());
        assert!(encode_jpeg(&px, w, h, 1, 0).is_ok());
    }

    #[test]
    fn samples_per_pixel_must_be_1_or_3() {
        let (w, h) = (8u32, 8u32);
        let px = grayscale(w, h);
        assert!(encode_jpeg(&px, w, h, 2, 80).is_err());
        assert!(decode_jpeg(&[], w, h, 2).is_err());
        assert!(decode_jpeg(&[], w, h, 4).is_err());
    }

    #[test]
    fn encode_validates_pixel_byte_count() {
        let (w, h) = (8u32, 8u32);
        let too_small = grayscale(w, h - 1);
        assert!(encode_jpeg(&too_small, w, h, 1, 80).is_err());
    }

    #[test]
    fn decode_rejects_dimension_mismatch() {
        let (w, h) = (16u32, 16u32);
        let px = grayscale(w, h);
        let enc = encode_jpeg(&px, w, h, 1, 80).unwrap();
        // Same bytes, but ask for different dimensions than the stream header.
        assert!(decode_jpeg(&enc, w + 1, h, 1).is_err());
        assert!(decode_jpeg(&enc, w, h + 1, 1).is_err());
    }

    #[test]
    fn decode_rejects_malformed_input() {
        assert!(decode_jpeg(b"not a jpeg", 4, 4, 1).is_err());
        assert!(decode_jpeg(&[], 4, 4, 1).is_err());
    }
}
