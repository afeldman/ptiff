#pragma once

#include <cstddef>
#include <span>
#include <vector>

#include <ptiff/core/result.hpp>

namespace ptiff::compression {

/// Decodes TIFF-variant LZW-compressed data (Compression=5): MSB-first bit packing, 9-bit codes
/// growing to 12 bits, clear code 256 and end-of-information code 257, and TIFF's "early change"
/// quirk -- the code width grows one code earlier than standard LZW/GIF (at dictionary sizes
/// 511/1023/2047 instead of 512/1024/2048), required to correctly decode real-world TIFF LZW
/// streams. Decoding stops the instant exactly `expectedSize` bytes have been produced.
/// Error::InvalidArgument if: a code references a dictionary entry that doesn't exist yet (other
/// than the one valid "not yet in the table" case), decoding would produce more than
/// `expectedSize` bytes (the decompression-bomb guard), the bitstream is exhausted before an EOI
/// code appears, or the decoded output is shorter than `expectedSize` once EOI is reached.
[[nodiscard]] Result<std::vector<std::byte>> decodeLzw(std::span<const std::byte> input,
                                                       std::size_t expectedSize);

/// Encodes `input` with the TIFF-variant of LZW (Compression=5), inverse of decodeLzw. Emits an
/// initial clear code 256, then greedy dictionary matches built one code per input phrase; the
/// dictionary grows from code 258 toward 4094 and is cleared when full. Codes are MSB-first with
/// the TIFF "early change" width schedule (grows at dictionary sizes 511/1023/2047). The final
/// partial code word is zero-padded to a byte boundary (end of stream is implied by padding,
/// following the same byte-stream convention decodeLzw consumes). Error::InvalidArgument if
/// input.size() exceeds 0xFFFFFFFFU (the strip's uint32 byte count).
[[nodiscard]] Result<std::vector<std::byte>> encodeLzw(std::span<const std::byte> input);

} // namespace ptiff::compression
