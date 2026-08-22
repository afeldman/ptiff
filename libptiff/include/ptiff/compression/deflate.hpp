#pragma once

#include <cstddef>
#include <span>
#include <vector>

#include <ptiff/core/result.hpp>

namespace ptiff::compression {

/// Decodes zlib-wrapped Deflate-compressed data (TIFF Compression=8, "Adobe Deflate", and the
/// legacy Compression=32946 tag value both resolve to this codec -- both write the same zlib
/// (RFC 1950) stream format). Decoding fails the instant the decompressed size would exceed
/// `expectedSize` (the decompression-bomb guard) or falls short of it once the stream ends.
/// Error::InvalidArgument on a malformed/truncated zlib stream, or a decoded size that does not
/// exactly equal `expectedSize`.
[[nodiscard]] Result<std::vector<std::byte>> decodeDeflate(std::span<const std::byte> input,
                                                           std::size_t expectedSize);

/// Encodes `input` as a zlib-wrapped Deflate stream (TIFF Compression=8), inverse of
/// decodeDeflate. Error::InvalidArgument if `input.size()` exceeds 0xFFFFFFFFU (a single strip's
/// byte count) or the underlying zlib compressor reports failure.
[[nodiscard]] Result<std::vector<std::byte>> encodeDeflate(std::span<const std::byte> input);

} // namespace ptiff::compression
