#pragma once

#include <cstdint>

namespace ptiff::io::tile {

/// @brief Position of one tile within its image's tiling grid.
///
/// A TileIndex identifies exactly one tile of a @ref ptiff::io::tile::TileLayout "TileLayout"
/// through its \c column and \c row within a given pyramid \c level. Tiling is iterated in
/// row-major order: the column advances fastest, which matches how tile byte ranges are laid out
/// back-to-back in file formats such as TIFF.
///
/// \c level is 0 for the base resolution and increases for coarser pyramid/mip levels a backend
/// may expose later (see TileLayout::columns / TileLayout::rows for how each level down-samples
/// the image). A tile's level is always strictly smaller than the layout's `levelCount`.
///
/// @section tile_index_example Example
///
/// @code{.cpp}
/// using ptiff::io::tile::TileIndex;
/// using ptiff::io::tile::TileLayout;
///
/// TileLayout layout{.tileSize = {16, 16}, .imageWidth = 64, .imageHeight = 32};
///
/// // The tile covering pixels x in [32,48) and y in [16,32) is (column 2, row 1) at level 0.
/// auto idx = layout.indexFor(33, 20);
/// assert(idx.has_value());
/// assert(idx->column == 2 && idx->row == 1 && idx->level == 0);
///
/// // Build an index directly (e.g. when streaming sequentially over a grid).
/// TileIndex base{.column = 0, .row = 0, .level = 0};
/// assert(base.column == 0 && base.row == 0 && base.level == 0);
/// @endcode
///
/// @note TileIndex is validated against a layout by TileLayout's query methods (regionFor,
///       indexFor); the index itself performs no validation.
struct TileIndex {
    /// Tile column index (horizontal position) within the level's grid.
    std::uint32_t column = 0;
    /// Tile row index (vertical position) within the level's grid.
    std::uint32_t row = 0;
    /// Pyramid level; 0 = base resolution.
    std::uint32_t level = 0;

    /// Defaulted member-wise equality (useful for lookups and caches).
    friend constexpr bool operator==(const TileIndex&, const TileIndex&) = default;
};

} // namespace ptiff::io::tile
