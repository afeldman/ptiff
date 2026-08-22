#pragma once

namespace ptiff {

/// @brief Storage compression scheme, if any.
///
/// Describes how an image's pixels are compressed on disk. As of `0.2.0` this is descriptive
/// metadata only -- the value is accepted when writing but the actual encode/decode of every
/// scheme is implemented at the backend level (see
/// @ref ptiff::io::backend::TiffBackend "TiffBackend"). This list is **additive** when extended.
enum class CompressionKind {
    None,    ///< No compression (raw/lossless storage).
    Lzw,     ///< TIFF-variant LZW lossless compression.
    Deflate, ///< zlib-wrapped Deflate lossless compression.
    Jpeg,    ///< Baseline JPEG lossy compression (8-bit samples only).
};

} // namespace ptiff
