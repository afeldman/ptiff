#pragma once

// Tiny zstd/zlib compression helpers for Zarr chunk slots. Chunk payloads are either raw
// (Compressor::None) or compressed with zstd/zlib; both directions round-trip losslessly for the
// pure pixel bytes ptiff writes.

#include <cstddef>
#include <span>
#include <vector>

#include <ptiff/core/result.hpp>

// C APIs use extern "C".
extern "C" {
#include <zlib.h>
#include <zstd.h>
}

#include <ptiff/io/backend/zarr/zarr_document.hpp>

namespace ptiff::io::backend::zarr {

/// @brief Worst-case on-disk payload size for @p compressor over @p n raw bytes (before the 4-byte
///        length prefix is added).
inline std::size_t payloadBound(Compressor c, std::size_t n) {
    switch (c) {
    case Compressor::Zstd:
        return ZSTD_compressBound(n);
    case Compressor::Zlib:
        return compressBound(static_cast<uLong>(n));
    case Compressor::None:
    default:
        return n;
    }
}

/// @brief Compresses @p in according to @p compressor (returns raw bytes when None).
[[nodiscard]] inline Result<std::vector<std::byte>> compress(Compressor c,
                                                             std::span<const std::byte> in) {
    if (c == Compressor::None) {
        return std::vector<std::byte>{in.begin(), in.end()};
    }
    const std::size_t bound = payloadBound(c, in.size());
    std::vector<std::byte> out(bound);
    std::size_t outLen = 0;

    if (c == Compressor::Zstd) {
        const std::size_t n = ZSTD_compress(out.data(), out.size(), in.data(), in.size(), 3);
        if (ZSTD_isError(n)) {
            return std::unexpected(Error{ErrorCode::InvalidArgument, "zarr: zstd compress failed"});
        }
        outLen = n;
    } else { // Zlib
        uLongf n = static_cast<uLongf>(out.size());
        const int rc = compress2(reinterpret_cast<Bytef*>(out.data()),
                                 &n,
                                 reinterpret_cast<const Bytef*>(in.data()),
                                 static_cast<uLong>(in.size()),
                                 6);
        if (rc != Z_OK) {
            return std::unexpected(Error{ErrorCode::InvalidArgument, "zarr: zlib compress failed"});
        }
        outLen = n;
    }
    out.resize(outLen);
    return out;
}

/// @brief Decompresses @p in (raw when @p c is None) into exactly @p expectedBytes.
[[nodiscard]] inline Result<std::vector<std::byte>>
decompress(Compressor c, std::span<const std::byte> in, std::size_t expectedBytes) {
    if (c == Compressor::None) {
        if (in.size() != expectedBytes) {
            return std::unexpected(
                Error{ErrorCode::InvalidArgument, "zarr: raw chunk size mismatch"});
        }
        return std::vector<std::byte>{in.begin(), in.end()};
    }
    std::vector<std::byte> out(expectedBytes);
    if (c == Compressor::Zstd) {
        const std::size_t n = ZSTD_decompress(out.data(), out.size(), in.data(), in.size());
        if (ZSTD_isError(n) || n != expectedBytes) {
            return std::unexpected(
                Error{ErrorCode::InvalidArgument, "zarr: zstd decompress failed"});
        }
    } else { // Zlib
        uLongf n = static_cast<uLongf>(out.size());
        const int rc = uncompress(reinterpret_cast<Bytef*>(out.data()),
                                  &n,
                                  reinterpret_cast<const Bytef*>(in.data()),
                                  static_cast<uLong>(in.size()));
        if (rc != Z_OK || n != expectedBytes) {
            return std::unexpected(
                Error{ErrorCode::InvalidArgument, "zarr: zlib decompress failed"});
        }
    }
    return out;
}

} // namespace ptiff::io::backend::zarr
