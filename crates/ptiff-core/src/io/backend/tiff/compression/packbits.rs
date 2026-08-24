//! TIFF PackBits compression (Compression = 32773).
//!
//! Mirrors `ptiff::compression::packbits.{hpp,cpp}`.

use crate::{Error, Result};

/// Decodes PackBits-compressed data (TIFF Compression=32773), the byte-oriented
/// RLE scheme: a signed control byte `n` followed by either `(n+1)` literal
/// bytes (`0 <= n <= 127`) or one byte repeated `(1-n)` times
/// (`-127 <= n <= -1`); `n == -128` is a no-op consuming only itself. Decoding
/// stops the instant exactly `expected_size` bytes have been produced.
///
/// # Errors
///
/// Returns [`crate::ErrorCode::InvalidArgument`] if a literal/repeat run would
/// read past `input`'s end, decoding would produce more than `expected_size`
/// bytes (decompression-bomb guard), or the input is exhausted before exactly
/// `expected_size` bytes have been produced.
pub fn decode_pack_bits(input: &[u8], expected_size: usize) -> Result<Vec<u8>> {
    let mut output = Vec::with_capacity(expected_size);
    let mut pos = 0usize;

    while output.len() < expected_size {
        if pos >= input.len() {
            return Err(Error::invalid_argument(
                "decodePackBits: input exhausted before expectedSize",
            ));
        }
        let control = input[pos] as i8;
        pos += 1;

        if control >= 0 {
            let count = (control as usize) + 1;
            if pos + count > input.len() {
                return Err(Error::invalid_argument(
                    "decodePackBits: literal run reads past input end",
                ));
            }
            if output.len() + count > expected_size {
                return Err(Error::invalid_argument(
                    "decodePackBits: decoded output exceeds expectedSize",
                ));
            }
            output.extend_from_slice(&input[pos..pos + count]);
            pos += count;
        } else if control != -128 {
            let count = ((-control) as usize) + 1;
            if pos >= input.len() {
                return Err(Error::invalid_argument(
                    "decodePackBits: repeat run missing its byte",
                ));
            }
            if output.len() + count > expected_size {
                return Err(Error::invalid_argument(
                    "decodePackBits: decoded output exceeds expectedSize",
                ));
            }
            let value = input[pos];
            pos += 1;
            output.extend(std::iter::repeat_n(value, count));
        }
        // control == -128: no-op.
    }

    if output.len() != expected_size {
        return Err(Error::invalid_argument(
            "decodePackBits: decoded output does not match expectedSize",
        ));
    }
    Ok(output)
}

/// Encodes `input` with the TIFF PackBits variant, the exact inverse of
/// [`decode_pack_bits`]. Group runs of >= 2 identical bytes into repeat
/// controls and maximal non-run stretches into literal controls capped at 128
/// literals, so a compliant decoder reproduces `input` exactly.
///
/// # Errors
///
/// Returns [`crate::ErrorCode::InvalidArgument`] if `input.len()` exceeds
/// `u32::MAX` (a single strip's byte count).
pub fn encode_pack_bits(input: &[u8]) -> Result<Vec<u8>> {
    if input.len() as u64 > u32::MAX as u64 {
        return Err(Error::invalid_argument(
            "encodePackBits: input exceeds uint32 strip byte count",
        ));
    }
    let mut out = Vec::with_capacity(input.len());
    let n = input.len();
    let mut pos = 0usize;

    while pos < n {
        // A run of >= 2 identical bytes becomes a repeat control.
        let mut run_len = 1usize;
        while run_len < n - pos && input[pos + run_len] == input[pos] {
            run_len += 1;
        }
        if run_len >= 2 {
            let run_len_before = run_len;
            while run_len > 0 {
                let chunk = run_len.min(128);
                // control = -(chunk - 1): chunk 2 -> -1 ... chunk 128 -> -127.
                out.push((-(chunk as isize - 1)) as i8 as u8);
                out.push(input[pos]);
                run_len -= chunk;
            }
            pos += run_len_before;
            continue;
        }
        // Literal run: gather up to 128 bytes, stopping just before a run >= 2.
        let lit_start = pos;
        pos += 1;
        while pos < n && (pos - lit_start) < 128 && input[pos] != input[pos - 1] {
            pos += 1;
        }
        let literal_count = pos - lit_start;
        out.push((literal_count - 1) as u8);
        out.extend_from_slice(&input[lit_start..pos]);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_handles_literal_repeat_and_noop() {
        // Literal "abc", repeat 0x41 x3, no-op -128.
        let input = vec![
            2, b'a', b'b', b'c', // literal run of 3
            -3i8 as u8, 0x41, // repeat 0x41 x4
            0x80, // -128 no-op
            0, 0x00, // literal 1 byte
        ];
        let decoded = decode_pack_bits(&input, 3 + 4 + 1).unwrap();
        assert_eq!(
            decoded,
            vec![b'a', b'b', b'c', 0x41, 0x41, 0x41, 0x41, 0x00]
        );
    }

    #[test]
    fn decode_rejects_input_exhausted() {
        // Declares a repeat of 4 but provides no byte.
        let input = vec![1, b'a', b'b', -3i8 as u8];
        let err = decode_pack_bits(&input, 100).unwrap_err();
        assert_eq!(err.code(), crate::ErrorCode::InvalidArgument);
    }

    #[test]
    fn encode_then_decode_round_trips() {
        // Mixed literals and a run.
        let data: Vec<u8> = vec![
            1, 2, 3, // literals
            0xAA, 0xAA, 0xAA, 0xAA, // run of 4
            5, 6, 7, 8, // literals
        ];
        let encoded = encode_pack_bits(&data).unwrap();
        let decoded = decode_pack_bits(&encoded, data.len()).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn encode_handles_long_runs_in_chunks() {
        // 300 identical bytes -> repeat controls chunked at 128.
        let data = vec![0x7Fu8; 300];
        let encoded = encode_pack_bits(&data).unwrap();
        let decoded = decode_pack_bits(&encoded, data.len()).unwrap();
        assert_eq!(decoded, data);
    }
}
