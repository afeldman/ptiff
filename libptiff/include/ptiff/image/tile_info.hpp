#pragma once

#include <cstdint>

namespace ptiff {

/// @brief Describes how an image is stored as tiles rather than one contiguous strip.
///
/// Present only if the image is stored/organized as tiles rather than one contiguous strip.
/// Tiles partition the image grid into fixed-size rectangular regions; the tile dimensions must
/// divide the image dimensions (or leave a partially-filled right/bottom edge).
///
/// @section tile_info_example Example
///
/// @code{.cpp}
/// using ptiff::TileInfo;
/// TileInfo t;
/// t.tileWidth  = 256;   // 256-pixel-wide tiles
/// t.tileHeight = 256;   // 256-pixel-tall tiles
/// @endcode
struct TileInfo {
    std::uint32_t tileWidth = 0;  ///< Tile width in pixels.
    std::uint32_t tileHeight = 0; ///< Tile height in pixels.

    friend constexpr bool operator==(const TileInfo&, const TileInfo&) = default;
};

} // namespace ptiff
