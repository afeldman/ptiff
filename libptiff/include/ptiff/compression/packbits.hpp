#pragma once

#include <cstddef>
#include <span>
#include <vector>

#include <ptiff/core/result.hpp>

namespace ptiff::compression {

/// Decodes PackBits-compressed data (TIFF Compression=32773), the byte-oriented RLE scheme: a
/// signed control byte n followed by either (n+1) literal bytes (0 <= n <= 127) or one byte
/// repeated (1-n) times (-127 <= n <= -1); n == -128 is a no-op consuming only itself. Decoding
/// stops the instant exactly `expectedSize` bytes have been produced. Error::InvalidArgument if:
/// a literal/repeat run would read past `input`'s end, decoding would produce more than
/// `expectedSize` bytes (the decompression-bomb guard -- this bounds output growth against the
/// caller's already-known-good uncompressed size), or the input is exhausted before exactly
/// `expectedSize` bytes have been produced.
[[nodiscard]] Result<std::vector<std::byte>> decodePackBits(std::span<const std::byte> input,
                                                            std::size_t expectedSize);

/// Encodes `input` with the TIFF PackBits variant (Compression=32773): a signed control byte n
/// followed by either (n+1) literal bytes (0 <= n <= 127) or one byte repeated (1-n) times
/// (-127 <= n <= -1); n == -128 is a no-op. Group runs of >= 2 identical bytes into repeat
/// controls and maximal non-run stretches into literal controls capped at 128 literals, so a
/// compliant decoder (including this project's decodePackBits) reproduces `input` exactly.
/// Error::InvalidArgument if `input.size()` exceeds 0xFFFFFFFFU (a single strip's byte count).
[[nodiscard]] Result<std::vector<std::byte>> encodePackBits(std::span<const std::byte> input);

} // namespace ptiff::compression
