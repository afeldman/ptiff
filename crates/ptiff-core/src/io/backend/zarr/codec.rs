//! Zarr chunk compression codecs.
//!
//! Mirrors `libptiff/include/ptiff/io/backend/zarr/zarr_codec.hpp`. Chunk
//! payloads are either raw (`Compressor::None`) or compressed with zstd/zlib
//! (RFC 1950 zlib format via `flate2`); both directions round-trip losslessly
//! for the pure pixel bytes ptiff writes. `payload_bound` mirrors the C++
//! reference's `ZSTD_compressBound` / `compressBound` bounds so slots are laid
//! out byte-for-byte identically to the C++ oracle.

use crate::{Error, Result};

use super::document::Compressor;

/// Worst-case on-disk payload size for `compressor` over `n` raw bytes
/// (before the 4-byte length prefix is added). Mirrors the C++ reference's
/// `payloadBound`, which reuses the zstd/zlib `compressBound` formulae so the
/// slot geometry is byte-identical to `libptiff`.
pub fn payload_bound(compressor: Compressor, n: usize) -> usize {
    match compressor {
        Compressor::Zstd => n + (n >> 7) + 128 + 3,
        Compressor::Zlib => n + (n >> 12) + (n >> 14) + (n >> 25) + 13,
        Compressor::None => n,
    }
}

/// Compresses `data` according to `compressor`, returning the raw bytes
/// unchanged when `None`.
///
/// # Errors
///
/// Returns [`crate::ErrorCode::InvalidArgument`] if the underlying zstd/zlib
/// (de)compressor rejects the input.
pub fn compress(compressor: Compressor, data: &[u8]) -> Result<Vec<u8>> {
    match compressor {
        Compressor::None => Ok(data.to_vec()),
        Compressor::Zstd => zstd::bulk::compress(data, 3)
            .map_err(|e| Error::invalid_argument(format!("zarr: zstd compress failed: {e}"))),
        Compressor::Zlib => {
            use std::io::Write;
            let mut encoder =
                flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::new(6));
            encoder
                .write_all(data)
                .and_then(|_| encoder.finish())
                .map_err(|e| Error::invalid_argument(format!("zarr: zlib compress failed: {e}")))
        }
    }
}

/// Decompresses `data` (raw when `compressor` is `None`) into exactly
/// `expected_bytes`.
///
/// # Errors
///
/// Returns [`crate::ErrorCode::InvalidArgument`] when the raw size differs from
/// `expected_bytes` or the decompressor rejects the input / produces a
/// different length.
pub fn decompress(compressor: Compressor, data: &[u8], expected_bytes: usize) -> Result<Vec<u8>> {
    match compressor {
        Compressor::None => {
            if data.len() != expected_bytes {
                return Err(Error::invalid_argument("zarr: raw chunk size mismatch"));
            }
            Ok(data.to_vec())
        }
        Compressor::Zstd => {
            let out = zstd::bulk::decompress(data, expected_bytes).map_err(|e| {
                Error::invalid_argument(format!("zarr: zstd decompress failed: {e}"))
            })?;
            if out.len() != expected_bytes {
                return Err(Error::invalid_argument("zarr: zstd decompress failed"));
            }
            Ok(out)
        }
        Compressor::Zlib => {
            use std::io::Read;
            let mut decoder = flate2::read::ZlibDecoder::new(data);
            let mut out = Vec::with_capacity(expected_bytes);
            decoder.read_to_end(&mut out).map_err(|e| {
                Error::invalid_argument(format!("zarr: zlib decompress failed: {e}"))
            })?;
            if out.len() != expected_bytes {
                return Err(Error::invalid_argument("zarr: zlib decompress failed"));
            }
            Ok(out)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &[u8] = b"the quick brown fox jumps over the lazy dog. the quick brown fox";

    #[test]
    fn none_is_a_pass_through() {
        assert_eq!(payload_bound(Compressor::None, SAMPLE.len()), SAMPLE.len());
        assert_eq!(compress(Compressor::None, SAMPLE).unwrap(), SAMPLE);
        assert_eq!(
            decompress(Compressor::None, SAMPLE, SAMPLE.len()).unwrap(),
            SAMPLE
        );
    }

    #[test]
    fn compressed_chunks_round_trip() {
        for compressor in [Compressor::Zstd, Compressor::Zlib] {
            let compressed = compress(compressor, SAMPLE).unwrap();
            // Bounds must at least fit the true payload (plus the 4-byte prefix in the slot).
            assert!(compressed.len() <= payload_bound(compressor, SAMPLE.len()));
            let decompressed = decompress(compressor, &compressed, SAMPLE.len()).unwrap();
            assert_eq!(decompressed, SAMPLE);
        }
    }

    #[test]
    fn payload_bound_is_strict() {
        // For a compressible sample the bound comfortably exceeds the length-prefixed payload.
        for compressor in [Compressor::Zstd, Compressor::Zlib] {
            let compressed = compress(compressor, SAMPLE).unwrap();
            assert!(payload_bound(compressor, SAMPLE.len()) >= 4 + compressed.len());
        }
    }

    #[test]
    fn zlib_is_real_rfc1950_stream() {
        let compressed = compress(Compressor::Zlib, SAMPLE).unwrap();
        // RFC 1950 zlib streams start with 0x78 (CMF).
        assert_eq!(compressed[0], 0x78);
    }
}
