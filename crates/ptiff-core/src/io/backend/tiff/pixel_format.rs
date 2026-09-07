//! TIFF BitsPerSample/SampleFormat to [`PixelType`] mapping.
//!
//! Mirrors `ptiff::io::backend::tiff::tiff_pixel_format.{hpp,cpp}`.

use crate::pixel_type::PixelType;
use crate::{Error, Result};

/// Maps TIFF's (BitsPerSample, SampleFormat) pair to [`PixelType`], for this
/// backend's supported subset: unsigned int at 8/16/32 bits, or IEEE float at
/// 32 or 64 bits. SampleFormat: 1 = unsigned int (TIFF's default when the tag
/// is absent), 3 = IEEE float. Any other SampleFormat, or a BitsPerSample
/// unsupported for the given format, is [`crate::ErrorCode::InvalidArgument`].
pub fn resolve_pixel_type(bits_per_sample: u64, sample_format: u64) -> Result<PixelType> {
    if sample_format != 1 && sample_format != 3 {
        return Err(Error::invalid_argument(
            "resolve_pixel_type: unsupported SampleFormat",
        ));
    }
    if sample_format == 3 {
        // IEEE float: 32-bit (Float32) or 64-bit (Float64). Float64 must be
        // accepted here so files written with Float64 pixels (BitsPerSample 64,
        // SampleFormat 3, emitted by `sample_format_for`) can be read back.
        return match bits_per_sample {
            32 => Ok(PixelType::Float32),
            64 => Ok(PixelType::Float64),
            _ => Err(Error::invalid_argument(
                "resolve_pixel_type: float SampleFormat requires 32-bit or 64-bit BitsPerSample",
            )),
        };
    }
    match bits_per_sample {
        8 => Ok(PixelType::UInt8),
        16 => Ok(PixelType::UInt16),
        32 => Ok(PixelType::UInt32),
        _ => Err(Error::invalid_argument(
            "resolve_pixel_type: unsupported BitsPerSample",
        )),
    }
}

/// Error::InvalidArgument if `values` is empty or contains more than one
/// distinct value -- this backend requires BitsPerSample to be uniform across
/// all samples of a pixel.
pub fn require_uniform_bits_per_sample(values: &[u64]) -> Result<()> {
    let Some(first) = values.first() else {
        return Err(Error::invalid_argument(
            "require_uniform_bits_per_sample: no values",
        ));
    };
    for value in values {
        if value != first {
            return Err(Error::invalid_argument(
                "require_uniform_bits_per_sample: non-uniform BitsPerSample",
            ));
        }
    }
    Ok(())
}

/// The StorageModel field-value string for `pixel_type` (e.g. "UInt8",
/// "Float32") -- the inverse of the BitsPerSample/SampleFormat mapping, used by
/// the TIFF directory's `to_storage_model`.
#[must_use]
pub const fn pixel_type_field_value(pixel_type: PixelType) -> &'static str {
    match pixel_type {
        PixelType::UInt8 => "UInt8",
        PixelType::UInt16 => "UInt16",
        PixelType::UInt32 => "UInt32",
        PixelType::Float32 => "Float32",
        PixelType::Float64 => "Float64",
    }
}

/// Byte size of one sample of `pixel_type` (1/2/4/8). Shared by the read path
/// (image source, expected-tile-size computation) and the write path (directory
/// writer, StripByteCounts computation).
#[must_use]
pub const fn bytes_per_sample(pixel_type: PixelType) -> u8 {
    match pixel_type {
        PixelType::UInt8 => 1,
        PixelType::UInt16 => 2,
        PixelType::UInt32 | PixelType::Float32 => 4,
        PixelType::Float64 => 8,
    }
}

/// Inverse of [`pixel_type_field_value`] -- parses a StorageModel "pixelType"
/// field string back into a [`PixelType`].
///
/// # Errors
///
/// [`crate::ErrorCode::InvalidArgument`] if `value` isn't one of the strings
/// [`pixel_type_field_value`] produces.
pub fn pixel_type_from_field_value(value: &str) -> Result<PixelType> {
    match value {
        "UInt8" => Ok(PixelType::UInt8),
        "UInt16" => Ok(PixelType::UInt16),
        "UInt32" => Ok(PixelType::UInt32),
        "Float32" => Ok(PixelType::Float32),
        "Float64" => Ok(PixelType::Float64),
        _ => Err(Error::invalid_argument(
            "pixel_type_from_field_value: unrecognized pixelType field value",
        )),
    }
}

/// The TIFF BitsPerSample value to write for `pixel_type` -- the inverse
/// (together with [`sample_format_for`]) of [`resolve_pixel_type`].
#[must_use]
pub fn bits_per_sample_for(pixel_type: PixelType) -> u16 {
    (u16::from(bytes_per_sample(pixel_type))) * 8
}

/// The TIFF SampleFormat value to write for `pixel_type` (1 = unsigned int,
/// 3 = IEEE float) -- the inverse (together with [`bits_per_sample_for`]) of
/// [`resolve_pixel_type`].
#[must_use]
pub fn sample_format_for(pixel_type: PixelType) -> u16 {
    if matches!(pixel_type, PixelType::Float32 | PixelType::Float64) {
        3
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ErrorCode;

    #[test]
    fn resolve_unsigned_integer_formats() {
        assert_eq!(resolve_pixel_type(8, 1).unwrap(), PixelType::UInt8);
        assert_eq!(resolve_pixel_type(16, 1).unwrap(), PixelType::UInt16);
        assert_eq!(resolve_pixel_type(32, 1).unwrap(), PixelType::UInt32);
    }

    #[test]
    fn resolve_32_bit_float() {
        assert_eq!(resolve_pixel_type(32, 3).unwrap(), PixelType::Float32);
    }

    #[test]
    fn resolve_64_bit_float() {
        assert_eq!(resolve_pixel_type(64, 3).unwrap(), PixelType::Float64);
    }

    #[test]
    fn reject_float_at_unsupported_bit_depth() {
        // 16-bit float is not part of the supported subset.
        let e = resolve_pixel_type(16, 3).unwrap_err();
        assert_eq!(e.code(), ErrorCode::InvalidArgument);
    }

    #[test]
    fn reject_unsupported_bits_per_sample() {
        let e = resolve_pixel_type(12, 1).unwrap_err();
        assert_eq!(e.code(), ErrorCode::InvalidArgument);
    }

    #[test]
    fn reject_unsupported_sample_format() {
        // 2 = signed int, unsupported.
        let e = resolve_pixel_type(8, 2).unwrap_err();
        assert_eq!(e.code(), ErrorCode::InvalidArgument);
    }

    #[test]
    fn uniform_bits_per_sample_accepted() {
        require_uniform_bits_per_sample(&[8, 8, 8]).unwrap();
    }

    #[test]
    fn non_uniform_bits_per_sample_rejected() {
        let e = require_uniform_bits_per_sample(&[8, 16, 8]).unwrap_err();
        assert_eq!(e.code(), ErrorCode::InvalidArgument);
    }

    #[test]
    fn empty_bits_per_sample_rejected() {
        let e = require_uniform_bits_per_sample(&[]).unwrap_err();
        assert_eq!(e.code(), ErrorCode::InvalidArgument);
    }

    #[test]
    fn field_value_names_every_supported_type() {
        assert_eq!(pixel_type_field_value(PixelType::UInt8), "UInt8");
        assert_eq!(pixel_type_field_value(PixelType::UInt16), "UInt16");
        assert_eq!(pixel_type_field_value(PixelType::UInt32), "UInt32");
        assert_eq!(pixel_type_field_value(PixelType::Float32), "Float32");
        assert_eq!(pixel_type_field_value(PixelType::Float64), "Float64");
    }

    #[test]
    fn bytes_per_sample_maps_every_type() {
        assert_eq!(bytes_per_sample(PixelType::UInt8), 1);
        assert_eq!(bytes_per_sample(PixelType::UInt16), 2);
        assert_eq!(bytes_per_sample(PixelType::UInt32), 4);
        assert_eq!(bytes_per_sample(PixelType::Float32), 4);
        assert_eq!(bytes_per_sample(PixelType::Float64), 8);
    }

    #[test]
    fn field_value_round_trip_is_inverse() {
        for t in [
            PixelType::UInt8,
            PixelType::UInt16,
            PixelType::UInt32,
            PixelType::Float32,
            PixelType::Float64,
        ] {
            assert_eq!(
                pixel_type_from_field_value(pixel_type_field_value(t)).unwrap(),
                t
            );
        }
    }

    #[test]
    fn unknown_field_value_rejected() {
        let e = pixel_type_from_field_value("NotAPixelType").unwrap_err();
        assert_eq!(e.code(), ErrorCode::InvalidArgument);
    }

    #[test]
    fn bits_and_sample_format_round_trip_through_resolve() {
        for t in [
            PixelType::UInt8,
            PixelType::UInt16,
            PixelType::UInt32,
            PixelType::Float32,
            PixelType::Float64,
        ] {
            assert_eq!(
                resolve_pixel_type(
                    u64::from(bits_per_sample_for(t)),
                    u64::from(sample_format_for(t)),
                )
                .unwrap(),
                t
            );
        }
    }
}
