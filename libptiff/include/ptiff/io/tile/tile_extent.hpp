#pragma once

#include <cstdint>

namespace ptiff::io::tile {

/// @brief Pixel dimensions of a tile (or, reused inside TileRegion, of the rectangle a tile
/// covers).
///
/// TileExtent is a plain, cheap value type carrying a pixel width and height. It is used in two
/// contexts:
///
/// - as the \c tileSize of a @ref ptiff::io::tile::TileLayout "TileLayout", describing how big
///   every tile in a grid is;
/// - as the \c extent member of a @ref ptiff::io::tile::TileRegion "TileRegion", describing the
///   size of the pixel rectangle a particular tile covers.
///
/// The struct is a C++20 aggregate, so it can be created with designated initializers, and it
/// supports structural (member-wise) equality.
///
/// @section tile_extent_example Example
///
/// @code{.cpp}
/// using ptiff::io::tile::TileExtent;
///
/// TileExtent t{.width = 16, .height = 16}; // a square 16x16 tile
/// assert(t.width == 16 && t.height == 16);
///
/// TileExtent other{.width = 16, .height = 16};
/// assert(t == other); // structural equality compares both members
/// assert(TileExtent{.width = 16, .height = 32} != t);
/// @endcode
///
/// @note A tile dimension of zero meaningfully flags "untiled" in some consumers (e.g.
///       TileLayout treats a zero \c width or \c height as "no tiling").
struct TileExtent {
    /// The extent's width in pixels.
    std::uint32_t width = 0;
    /// The extent's height in pixels.
    std::uint32_t height = 0;

    /// Defaulted member-wise equality.
    friend constexpr bool operator==(const TileExtent&, const TileExtent&) = default;
};

} // namespace ptiff::io::tile
