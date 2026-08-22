#pragma once

#include <cstddef>
#include <cstdint>
#include <limits>

#include <ptiff/core/result.hpp>

namespace ptiff::io::backend::tiff {

/// Upper bound on the number of elements a single tag-array / strip-tile table may claim from a
/// file. Guards against resource-exhaustion from crafted counts (RFC-0001 §13). Real baseline
/// TIFF structures never come close; this only bounds hostile input.
constexpr std::uint64_t kMaxTagCount = 64ULL * 1024 * 1024; // 64M

/// Overflow-checked arithmetic helpers for the TIFF parser. TIFF metadata (offsets, counts,
/// lengths) comes from untrusted file contents, so any byte-size or file-position arithmetic
/// derived from it MUST be checked against integer overflow before use (RFC-0001 §13). These
/// helpers return ErrorCode::InvalidArgument on overflow so the caller can reject the malformed
/// file instead of silently wrapping.
///
/// The functions are `inline` in the header (no TU needed) and therefore usable from any parser
/// .cpp without a linking dependency.

/// Adds `a` + `b`, rejecting overflow instead of wrapping.
[[nodiscard]] inline Result<std::uint64_t> checkedAddU64(std::uint64_t a, std::uint64_t b) {
    if (a > std::numeric_limits<std::uint64_t>::max() - b) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "checkedAddU64: unsigned addition overflows"});
    }
    return a + b;
}

/// Multiplies `a` * `b`, rejecting overflow instead of wrapping.
[[nodiscard]] inline Result<std::uint64_t> checkedMulU64(std::uint64_t a, std::uint64_t b) {
    if (a != 0 && b > std::numeric_limits<std::uint64_t>::max() / a) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "checkedMulU64: unsigned multiplication overflows"});
    }
    return a * b;
}

/// Adds `a` + `b` (std::size_t), rejecting overflow instead of wrapping.
[[nodiscard]] inline Result<std::size_t> checkedAddSize(std::size_t a, std::size_t b) {
    if (a > std::numeric_limits<std::size_t>::max() - b) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "checkedAddSize: size addition overflows"});
    }
    return a + b;
}

/// Multiplies `a` * `b` (std::size_t), rejecting overflow instead of wrapping.
[[nodiscard]] inline Result<std::size_t> checkedMulSize(std::size_t a, std::size_t b) {
    if (a != 0 && b > std::numeric_limits<std::size_t>::max() / a) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "checkedMulSize: size multiplication overflows"});
    }
    return a * b;
}

} // namespace ptiff::io::backend::tiff
