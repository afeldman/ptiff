#pragma once

#include <ptiff/io/tile/tile_extent.hpp>

namespace ptiff::io::tile {

/// @brief The pixel-space rectangle one tile covers within its level.
///
/// A TileRegion is a thin value type combining a top-left corner (\c x, \c y), expressed in image
/// pixels at the tile's level, with a pixel @ref ptiff::io::tile::TileExtent "TileExtent". It is
/// the concrete, geometry-resolved output of TileLayout::regionFor and is handed to consumers
/// (e.g. @ref ptiff::io::ImageSource "ImageSource" / @ref ptiff::io::ImageSink "ImageSink") that
/// need to know exactly which pixels a tile's data corresponds to.
///
/// @section tile_region_edge Edge tiles
///
/// The \c extent of an edge tile is always the **full** tile size, even when the tile hangs over
/// the right or bottom edge of the image (that is, \c x + \c extent.width may exceed the image
/// width). Consumers treat overhanging pixels as padding/clipped, matching how storage layers pad
/// edge tiles out to the full tile size.
///
/// @section tile_region_example Example
///
/// @code{.cpp}
/// using ptiff::io::tile::TileRegion;
/// using ptiff::io::tile::TileExtent;
///
/// // Tile at column 2, row 1 of a 16x16 grid: its top-left corner is (32, 16).
/// TileRegion r{.x = 32, .y = 16, .extent = {.width = 16, .height = 16}};
/// assert(r.x == 32 && r.y == 16);
/// assert(r.extent.width == 16 && r.extent.height == 16);
///
/// TileRegion edge{.x = 48, .y = 0, .extent = {.width = 16, .height = 16}};
/// assert(edge.x + edge.extent.width == 64); // may exceed a narrower image; caller decides
/// @endcode
struct TileRegion {
    /// The rectangle's left edge, in image pixels at its level.
    std::uint32_t x = 0;
    /// The rectangle's top edge, in image pixels at its level.
    std::uint32_t y = 0;
    /// The rectangle's pixel extent (width and height).
    TileExtent extent;

    /// Defaulted member-wise equality.
    friend constexpr bool operator==(const TileRegion&, const TileRegion&) = default;
};

} // namespace ptiff::io::tile
