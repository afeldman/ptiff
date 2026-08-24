//! TIFF Predictor=2 (horizontal differencing) encode/decode.
//!
//! Mirrors `ptiff::compression::predictor.{hpp,cpp}`.

use crate::{Error, Result};

/// Reads one integer sample at `bytes[0..bytes_per_sample]` honoring the byte
/// order (big-endian vs little-endian).
#[must_use]
fn read_sample(bytes: &[u8], bytes_per_sample: u8, big_endian: bool) -> u32 {
    let mut value: u32 = 0;
    for i in 0..bytes_per_sample {
        let byte_value = u32::from(bytes[i as usize]);
        let shift = if big_endian {
            bytes_per_sample - 1 - i
        } else {
            i
        } as u32
            * 8;
        value |= byte_value << shift;
    }
    value
}

/// Writes one integer `value` into `bytes[0..bytes_per_sample]`.
fn write_sample(bytes: &mut [u8], value: u32, bytes_per_sample: u8, big_endian: bool) {
    for i in 0..bytes_per_sample {
        let shift = if big_endian {
            bytes_per_sample - 1 - i
        } else {
            i
        } as u32
            * 8;
        bytes[i as usize] = ((value >> shift) & 0xFF) as u8;
    }
}

/// Validates the sample geometry, returning one row's byte stride.
fn validate(
    data_len: usize,
    row_width: u32,
    samples_per_pixel: u32,
    bytes_per_sample: u8,
) -> Result<u64> {
    if bytes_per_sample != 1 && bytes_per_sample != 2 && bytes_per_sample != 4 {
        return Err(Error::invalid_argument(
            "undoHorizontalDifferencing: unsupported bytesPerSample",
        ));
    }
    let row_stride = u64::from(row_width)
        .checked_mul(u64::from(samples_per_pixel))
        .and_then(|v| v.checked_mul(u64::from(bytes_per_sample)))
        .ok_or_else(|| {
            Error::invalid_argument("undoHorizontalDifferencing: row stride computation overflows")
        })?;
    if row_stride == 0 || !(data_len as u64).is_multiple_of(row_stride) {
        return Err(Error::invalid_argument(
            "undoHorizontalDifferencing: data size is not a multiple of rowWidth * samplesPerPixel * bytesPerSample",
        ));
    }
    Ok(row_stride)
}

fn sample_mask(bytes_per_sample: u8) -> u32 {
    if bytes_per_sample == 4 {
        u32::MAX
    } else {
        (1u32 << (u32::from(bytes_per_sample) * 8)) - 1
    }
}

/// Undoes TIFF Predictor=2 (horizontal differencing) in place.
///
/// Each row was differenced independently per interleaved sample component as a
/// running sum modulo `2^(8*bytesPerSample)`; this reverses it with a running
/// sum reset at the start of every row. `bytes_per_sample` must be 1, 2, or 4;
/// `data.len()` must be an exact multiple of
/// `rowWidth * samplesPerPixel * bytesPerSample` -- any other size, or an
/// unsupported `bytes_per_sample`, is [`crate::ErrorCode::InvalidArgument`].
///
/// # Errors
///
/// Returns [`crate::ErrorCode::InvalidArgument`] on an unsupported sample size
/// or a data length that isn't an exact multiple of one row's stride.
pub fn undo_horizontal_differencing(
    data: &mut [u8],
    row_width: u32,
    samples_per_pixel: u32,
    bytes_per_sample: u8,
    big_endian: bool,
) -> Result<()> {
    let row_stride = validate(data.len(), row_width, samples_per_pixel, bytes_per_sample)?;
    let mask = sample_mask(bytes_per_sample);
    let bps = bytes_per_sample as usize;
    let spp = samples_per_pixel as usize;
    let row_stride_usize = row_stride as usize;

    for row in data.chunks_exact_mut(row_stride_usize) {
        let mut running = vec![0u32; spp];
        for col in 0..row_width {
            for (comp, running_val) in running.iter_mut().enumerate() {
                let offset = (col as usize * spp + comp) * bps;
                let sample_bytes = &mut row[offset..offset + bps];
                let delta = read_sample(sample_bytes, bytes_per_sample, big_endian);
                *running_val = running_val.wrapping_add(delta) & mask;
                write_sample(sample_bytes, *running_val, bytes_per_sample, big_endian);
            }
        }
    }
    Ok(())
}

/// Applies TIFF Predictor=2 (horizontal differencing) in place, the exact
/// inverse of [`undo_horizontal_differencing`].
///
/// # Errors
///
/// Returns [`crate::ErrorCode::InvalidArgument`] on an unsupported sample size
/// or a data length that isn't an exact multiple of one row's stride.
pub fn apply_horizontal_differencing(
    data: &mut [u8],
    row_width: u32,
    samples_per_pixel: u32,
    bytes_per_sample: u8,
    big_endian: bool,
) -> Result<()> {
    let row_stride = validate(data.len(), row_width, samples_per_pixel, bytes_per_sample)?;
    let mask = sample_mask(bytes_per_sample);
    let bps = bytes_per_sample as usize;
    let spp = samples_per_pixel as usize;
    let row_stride_usize = row_stride as usize;

    for row_bytes in data.chunks_exact_mut(row_stride_usize) {
        let mut previous = vec![0u32; spp];
        for col in 0..row_width {
            for (comp, previous_val) in previous.iter_mut().enumerate() {
                let offset = (col as usize * spp + comp) * bps;
                let sample_bytes = &mut row_bytes[offset..offset + bps];
                let sample = read_sample(sample_bytes, bytes_per_sample, big_endian);
                let delta = sample.wrapping_sub(*previous_val) & mask;
                write_sample(sample_bytes, delta, bytes_per_sample, big_endian);
                *previous_val = sample;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_reverses_apply_uint16_little() {
        // A 2x1 image, 2 samples/pixel, 2 bytes/sample, little-endian.
        let original: Vec<u8> = vec![
            10, 0, 20, 0, // row 0
            30, 0, 5, 0, //  row 1
        ];
        let mut data = original.clone();
        apply_horizontal_differencing(&mut data, 2, 2, 2, false).unwrap();
        undo_horizontal_differencing(&mut data, 2, 2, 2, false).unwrap();
        assert_eq!(data, original);
    }

    #[test]
    fn apply_wraps_modulo() {
        // Value 255 then 0: differencing stores (0 - 255) & 0xFF = 1.
        let mut data = vec![255u8, 0u8];
        apply_horizontal_differencing(&mut data, 2, 1, 1, false).unwrap();
        undo_horizontal_differencing(&mut data, 2, 1, 1, false).unwrap();
        assert_eq!(data, vec![255u8, 0u8]);
    }

    #[test]
    fn rejects_bad_sample_size() {
        let mut data = vec![0u8; 8];
        let err = undo_horizontal_differencing(&mut data, 2, 1, 3, false).unwrap_err();
        assert_eq!(err.code(), crate::ErrorCode::InvalidArgument);
    }

    #[test]
    fn rejects_mismatched_data_length() {
        let mut data = vec![0u8; 7]; // not a multiple of 2*1*2 = 4
        let err = undo_horizontal_differencing(&mut data, 2, 1, 2, false).unwrap_err();
        assert_eq!(err.code(), crate::ErrorCode::InvalidArgument);
    }
}
