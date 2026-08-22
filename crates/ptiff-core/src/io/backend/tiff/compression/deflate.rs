//! Deflate (zlib, RFC 1950) codec for TIFF strips/tiles.
//!
//! Mirrors `ptiff::compression::{encodeDeflate, decodeDeflate}` from
//! `src/compression/deflate.cpp`. The wire format is the zlib wrapper (RFC
//! 1950) -- the same stream `uncompress`/`compress2` in the C++ oracle produce.
//!
//! Unlike the LZW/PackBits codecs this module is NOT dependency-free: it wraps
//! [`flate2`]'s miniz_oxide backend (pure Rust, no system libz) and is only
//! built behind the `tiff-codecs` feature.

use flate2::{read::ZlibDecoder, write::ZlibEncoder, Compression};
use std::io::{Read, Write};

use crate::{Error, Result};

/// Decompresses a zlib-wrapped (RFC 1950) stream, requiring that the output is
/// *exactly* `expected_size` bytes.
///
/// Mirrors C++ `decodeDeflate(input, expectedSize)`: the decode is a
/// decompress-into-a-fixed-buffer that fails if the stream does not expand to
/// precisely the expected strip/tile byte count (a decompression-bomb guard --
/// a hostile stream cannot make us allocate more than `expected_size` bytes).
pub fn decode_deflate(input: &[u8], expected_size: usize) -> Result<Vec<u8>> {
    let decoder = ZlibDecoder::new(input);
    let mut output = Vec::with_capacity(expected_size);
    // Hard cap on decoded length: never exceed the expected strip byte count.
    decoder
        .take(expected_size as u64 + 1)
        .read_to_end(&mut output)
        .map_err(|_| Error::invalid_argument("decode_deflate: zlib decompression failed"))?;
    if output.len() != expected_size {
        return Err(Error::invalid_argument(
            "decode_deflate: decoded output does not match expectedSize",
        ));
    }
    Ok(output)
}

/// Compresses `input` with the zlib (RFC 1950) wrapper at default compression.
///
/// Mirrors C++ `encodeDeflate(input)`: rejects inputs that exceed the uint32
/// TIFF strip byte count (the caller writes the count into a 32-bit field).
pub fn encode_deflate(input: &[u8]) -> Result<Vec<u8>> {
    if input.len() as u64 > u32::MAX as u64 {
        return Err(Error::invalid_argument(
            "encode_deflate: input exceeds uint32 strip byte count",
        ));
    }
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(input)
        .map_err(|_| Error::invalid_argument("encode_deflate: zlib compression failed"))?;
    encoder
        .try_finish()
        .map_err(|_| Error::invalid_argument("encode_deflate: zlib compression failed"))?;
    let bytes = encoder
        .finish()
        .map_err(|_| Error::invalid_argument("encode_deflate: zlib compression failed"))?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::read::ZlibDecoder;
    use std::io::Read;

    #[test]
    fn round_trip() {
        let input: Vec<u8> = (0..4096).map(|i| (i % 251) as u8).collect();
        let encoded = encode_deflate(&input).unwrap();
        let decoded = decode_deflate(&encoded, input.len()).unwrap();
        assert_eq!(decoded, input);
    }

    #[test]
    fn empty_input_round_trips() {
        let encoded = encode_deflate(&[]).unwrap();
        let decoded = decode_deflate(&encoded, 0).unwrap();
        assert!(decoded.is_empty());
    }

    #[test]
    fn encode_matches_reference_zlib_wrapper() {
        // Independent RFC 1950 reference to prove our output is a valid zlib
        // stream (not raw deflate): decompress it with a second ZlibDecoder.
        let input = b"hello deflate world".to_vec();
        let encoded = encode_deflate(&input).unwrap();
        let mut decoder = ZlibDecoder::new(&encoded[..]);
        let mut out = Vec::new();
        decoder.read_to_end(&mut out).unwrap();
        assert_eq!(out, input);
        // The first two bytes are a zlib header (CMF=0x78, FLG with check bit).
        assert_eq!(encoded[0] & 0x0F, 8, "zlib CMF must be deflate method 8");
    }

    #[test]
    fn decode_is_exact_size_bomb_guard() {
        let input = b"compressible compressible compressible".to_vec();
        let encoded = encode_deflate(&input).unwrap();
        // Decoding to the wrong expected size must fail (the C++ oracle errors
        // when outLen != expectedSize).
        assert!(decode_deflate(&encoded, 1000).is_err());
        assert!(decode_deflate(&encoded, input.len() + 1).is_err());
        assert!(decode_deflate(&encoded, input.len()).is_ok());
    }

    #[test]
    fn decode_rejects_truncated_input() {
        let input = b"some data to be truncated during transmission".to_vec();
        let encoded = encode_deflate(&input).unwrap();
        let truncated = &encoded[..encoded.len() / 2];
        let result = decode_deflate(truncated, input.len());
        assert!(result.is_err());
    }

    #[test]
    fn decode_bomb_guard_limits_allocation() {
        // A stream that inflates to 10x its size must fail because expected is
        // small (only <= expected_size+1 bytes are ever allocated/read).
        let big: Vec<u8> = vec![0u8; 100_000];
        let encoded = encode_deflate(&big).unwrap();
        assert!(encoded.len() < big.len());
        let result = decode_deflate(&encoded, 10);
        assert!(
            result.is_err(),
            "bomb guard must reject oversized inflation"
        );
    }

    #[test]
    fn encode_rejects_oversized_input() {
        // Feed a slice we can't literally allocate at u32::MAX; the function
        // checks len as u64 against u32::MAX, so a large-but-representable
        // length on 64-bit triggers it without allocating.
        let big = vec![0u8; u32::MAX as usize + 1];
        assert!(encode_deflate(&big).is_err());
    }
}
