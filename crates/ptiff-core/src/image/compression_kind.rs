//! Storage compression scheme, if any.
//!
//! Mirrors `ptiff::CompressionKind` (see
//! `libptiff/include/ptiff/image/compression_kind.hpp`).

use std::fmt;

/// Storage compression scheme, if any.
///
/// Describes how an image's pixels are compressed on disk. This list is
/// **additive** when extended. It mirrors the codecs the TIFF backend can
/// physically read/write: None, LZW, PackBits, Deflate and JPEG. The typed
/// vocabulary and the TIFF storage layer round-trip these names 1:1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum CompressionKind {
    /// No compression (raw / lossless storage).
    None,
    /// TIFF-variant LZW lossless compression.
    Lzw,
    /// TIFF-variant PackBits lossless compression (run-length encoding).
    PackBits,
    /// zlib-wrapped Deflate lossless compression.
    Deflate,
    /// Baseline JPEG lossy compression (8-bit samples only).
    Jpeg,
}

impl fmt::Display for CompressionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            CompressionKind::None => "none",
            CompressionKind::Lzw => "lzw",
            CompressionKind::PackBits => "packbits",
            CompressionKind::Deflate => "deflate",
            CompressionKind::Jpeg => "jpeg",
        };
        f.write_str(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display() {
        assert_eq!(CompressionKind::None.to_string(), "none");
        assert_eq!(CompressionKind::Lzw.to_string(), "lzw");
        assert_eq!(CompressionKind::PackBits.to_string(), "packbits");
        assert_eq!(CompressionKind::Deflate.to_string(), "deflate");
        assert_eq!(CompressionKind::Jpeg.to_string(), "jpeg");
    }
}
