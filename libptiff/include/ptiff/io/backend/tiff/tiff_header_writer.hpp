#pragma once

#include <cstdint>

#include <ptiff/core/result.hpp>
#include <ptiff/io/binary_writer.hpp>

namespace ptiff::io::backend::tiff {

/// Byte size of a classic TIFF header.
inline constexpr std::uint64_t kClassicTiffHeaderSize = 8;

/// Byte size of a BigTIFF header.
inline constexpr std::uint64_t kBigTiffHeaderSize = 16;

/// Writes the classic 8-byte or BigTIFF 16-byte little-endian TIFF header at `writer`'s current
/// position. Classic: "II", magic 42, firstIfdOffset truncated to 4 bytes. BigTIFF: "II", magic
/// 43, offsetByteSize=8, constant 0, firstIfdOffset as 8 bytes. `isBigTiff` selects the variant;
/// defaults to classic.
[[nodiscard]] Result<void>
writeTiffHeader(BinaryWriter& writer, std::uint64_t firstIfdOffset, bool isBigTiff = false);

} // namespace ptiff::io::backend::tiff
