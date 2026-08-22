#pragma once

// Zarr-style single-file container: an 8-byte seek header, a JSON array header (nlohmann_json),
// then fixed-size chunk slots holding zstd/zlib-compressed (or raw) chunks.
//
//   [0 .. 8)     JSON header byte length H, little-endian uint64
//   [8 .. 8+H)   JSON array header text (shape/chunks/dtype/compressor)
//   [8+H ..)     chunk slots, each kSlotSize bytes:
//                  [0 .. 4)  actual chunk length L, little-endian uint32
//                  [4 .. 4+L) chunk bytes (compressed via the header's compressor, or raw)
//                  [4+L .. kSlotSize) unused padding
//
// Chunks are addressed linearly (row-major, same order as the memory backend's tiles). For this
// phase chunk == tile, so chunk i shares the tile's geometry. Fixed slot sizes make offsets
// deterministic without a per-chunk index (kSlotSize is derived from the format's worst-case
// compressed bound). See zarr_codec.hpp for the compressor dispatch. This is a deliberately
// compact, self-describing Zarr subset (ROADMAP M3) -- real JSON metadata + real chunk
// compression, packed into the single-stream interface the StorageBackend API requires.

#include <cstdint>
#include <span>
#include <vector>

#include <ptiff/core/result.hpp>
#include <ptiff/io/binary_reader.hpp>
#include <ptiff/io/storage_model.hpp>
#include <ptiff/io/tile/tile_layout.hpp>

namespace ptiff::io::backend::zarr {

/// @brief Byte length of the seek header that precedes the JSON array header.
inline constexpr std::uint64_t kHeaderSize = sizeof(std::uint64_t);

/// @brief Compressor ids understood by this container.
enum class Compressor { None, Zstd, Zlib };

/// @brief Header+chunk geometry of a Zarr document.
struct ZarrLayout {
    StorageModel image;            // reconstructed flat per-image storage fields
    std::vector<std::byte> header; // serialized JSON array header (without seek header)
    io::tile::TileLayout layout;   // tile geometry (chunk == tile for this phase)
    Compressor compressor = Compressor::None;
    std::uint32_t chunkBytes = 0; // uncompressed bytes per chunk
    std::uint32_t slotSize = 0;   // fixed on-disk size of one chunk slot (>= 4 + chunkBytes)
};

/// @brief Parses @p compressor (from the "None"/"zstd"/"zlib" storage field) into a Compressor.
[[nodiscard]] Result<Compressor> parseCompressor(std::string_view value);

/// @brief Builds the Zarr JSON array header from a flat per-image @p model.
/// @param model Must carry imageWidth/imageHeight/tileWidth/tileHeight/samplesPerPixel/pixelType
///        and optionally compression (None/zstd/zlib).
/// @return @ref ZarrLayout "ZarrLayout" with the header bytes and derived geometry, or
///         `InvalidArgument` on a missing field or unsupported pixel type/compressor.
[[nodiscard]] Result<ZarrLayout> buildLayout(const StorageModel& model);

/// @brief Reconstructs a flat per-image @ref StorageModel "StorageModel" from a JSON array header
///        (round-trips with @ref buildLayout).
/// @return The layout on success, or `InvalidArgument` if the JSON isn't a Zarr array header.
[[nodiscard]] Result<ZarrLayout> parseHeader(std::span<const std::byte> jsonHeader);

/// @brief Reads the seek header + JSON array header from @p reader.
/// @return @ref ZarrLayout "ZarrLayout" on success, or a backend-specific error on malformed
///         input.
[[nodiscard]] Result<ZarrLayout> readDocument(io::BinaryReader& reader);

} // namespace ptiff::io::backend::zarr
