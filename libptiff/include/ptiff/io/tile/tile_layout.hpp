#pragma once

#include <cstdint>

#include <ptiff/core/error.hpp>
#include <ptiff/core/result.hpp>
#include <ptiff/image/image_descriptor.hpp>
#include <ptiff/io/tile/tile_extent.hpp>
#include <ptiff/io/tile/tile_index.hpp>
#include <ptiff/io/tile/tile_region.hpp>

namespace ptiff::io::tile {

/// @brief Describes how a whole image is divided into tiles across one or more pyramid levels.
///
/// TileLayout is the storage-layer counterpart to @ref ptiff::TileInfo "TileInfo": while the
/// domain model only records a *hint* about the intended tile size on an
/// @ref ptiff::ImageDescriptor "ImageDescriptor", a TileLayout is the resolved, authoritative
/// tiling used by the I/O layer to place and index tile data on disk.
///
/// Every layout knows its tile dimensions, the total image size at the base resolution, and how
/// many resolution levels exist. Tile coordinates are expressed in row-major order -- column is
/// the fastest-moving index -- which matches how tile byte ranges are laid out back-to-back in
/// file formats such as TIFF (see \c tileByteRanges in the TIFF backend).
///
/// @section tile_layout_structure Member semantics
///
/// - \c tileSize: the pixel extent of a single tile. Both widths/heights may be smaller than the
///   image at the right/bottom edges; the tile is *not* logically cropped for indexing (see
///   @ref columns / @ref rows / @ref regionFor).
/// - \c imageWidth / \c imageHeight: base-resolution (level 0) image size.
/// - \c levelCount: number of pyramid levels; level 0 is the base resolution and each further
///   level halves both dimensions.
///
/// @section tile_layout_empty The "no tiling" sentinel
///
/// A layout whose \c tileSize.width or \c tileSize.height is zero represents "no tiling":
/// @ref columns / @ref rows return 0 and the convert/query operations report
/// @ref ptiff::ErrorCode::OutOfRange "OutOfRange". This is the natural result of default
/// construction and is how non-tiled images are represented internally.
///
/// @section tile_layout_example Example
///
/// The simplest way to obtain a layout is from an image descriptor that carries a tile hint:
///
/// @code{.cpp}
/// using ptiff::io::tile::TileLayout;
/// using ptiff::io::tile::TileIndex;
///
/// ptiff::ImageDescriptor desc;
/// desc.width  = 64;
/// desc.height = 32;
/// desc.tileInfo = ptiff::TileInfo{.tileWidth = 16, .tileHeight = 16};
///
/// auto layout = TileLayout::fromDescriptor(desc);
/// assert(layout.has_value());
/// assert(layout->columns()   == 4);   // 64 / 16
/// assert(layout->rows()      == 2);   // 32 / 16
/// assert(layout->levelCount  == 1);
///
/// // Map a pixel to its tile (row-major iteration yields tile (0,0), (1,0), ...).
/// auto index = layout->indexFor(33, 3);           // x=33 -> tile column 2
/// assert(index->column == 2 && index->row == 0);
///
/// // Ask which pixel rectangle a tile covers.
/// auto region = layout->regionFor({.column = 2, .row = 0, .level = 0});
/// assert(region->x == 32 && region->y == 0 && region->extent.width == 16);
/// @endcode
///
/// @note This type is a plain aggregate friendly to C++20 designated initializers
///       (see the example above) and is intended to be cheaply copied around.
///
/// @see @ref ptiff::io::ImageSource "ImageSource",
///      @ref ptiff::io::ImageSink "ImageSink" for the consumers of a layout.
struct TileLayout {
    /// Tile extent in pixels. A zero width or height indicates "no tiling".
    TileExtent tileSize;
    /// Base-resolution (level 0) image width in pixels.
    std::uint32_t imageWidth = 0;
    /// Base-resolution (level 0) image height in pixels.
    std::uint32_t imageHeight = 0;
    /// Number of resolution levels in the pyramid (level 0 = base resolution).
    std::uint32_t levelCount = 1;

    /// @brief Returns the number of tile columns that cover the image at \p level.
    ///
    /// Level 0 is the base resolution; each higher level \c L down-samples both dimensions by a
    /// factor of \c 2^L, matching conventional image-pyramid downsampling.
    ///
    /// @param level The pyramid level to query (defaults to the base level 0).
    /// @return The number of columns (\c 0 if the layout is non-tiled, i.e.
    ///         \c tileSize.width == 0).
    ///
    /// @code{.cpp}
    /// TileLayout l{.tileSize = {16, 16}, .imageWidth = 64, .imageHeight = 32};
    /// assert(l.columns() == 4); // 64 / 16, integer-rounding up
    /// assert(l.columns(1) == 2); // level 1: image width 32, 32 / 16 = 2 columns
    /// @endcode
    [[nodiscard]] constexpr std::uint32_t columns(std::uint32_t level = 0) const noexcept {
        if (tileSize.width == 0) {
            return 0;
        }
        const std::uint32_t levelWidth = imageWidth >> level;
        return (levelWidth + tileSize.width - 1) / tileSize.width;
    }

    /// @brief Returns the number of tile rows that cover the image at \p level.
    ///
    /// @param level The pyramid level to query (defaults to the base level 0).
    /// @return The number of rows (\c 0 if the layout is non-tiled, i.e.
    ///         \c tileSize.height == 0).
    ///
    /// @see @ref columns for the analogous column-count query.
    ///
    /// @code{.cpp}
    /// TileLayout l{.tileSize = {16, 16}, .imageWidth = 64, .imageHeight = 32};
    /// assert(l.rows() == 2);   // 32 / 16
    /// assert(l.rows(1) == 1);  // level 1: image height 16, 16 / 16 = 1 row
    /// @endcode
    [[nodiscard]] constexpr std::uint32_t rows(std::uint32_t level = 0) const noexcept {
        if (tileSize.height == 0) {
            return 0;
        }
        const std::uint32_t levelHeight = imageHeight >> level;
        return (levelHeight + tileSize.height - 1) / tileSize.height;
    }

    /// @brief Maps a tile \p index to the pixel-space rectangle it covers.
    ///
    /// The returned region's \c extent is always the full tile size, even for right/bottom edge
    /// tiles that extend beyond the image -- matching the contract @ref ptiff::io::ImageSource
    /// / @ref ptiff::io::ImageSink reason about (edge tiles are padded to the full tile size).
    ///
    /// @param index The tile to map; its \c level must be < \c levelCount and its
    ///        \c column/\c row must lie within the grid at that level (see @ref columns and
    ///        @ref rows).
    /// @return The pixel-space @ref ptiff::io::tile::TileRegion "TileRegion" on success, or
    ///         @ref ptiff::ErrorCode::OutOfRange "OutOfRange" if \p index's level is outside
    ///         this layout's level count, or if \p index is outside this layout's grid at its
    ///         level.
    ///
    /// @code{.cpp}
    /// TileLayout l{.tileSize = {16, 16}, .imageWidth = 64, .imageHeight = 32};
    /// auto r = l.regionFor({.column = 1, .row = 1, .level = 0});
    /// assert(r.has_value());
    /// assert(r->x == 16 && r->y == 16);           // top-left corner of that tile
    /// assert(r->extent.width == 16 && r->extent.height == 16);
    ///
    /// auto bad = l.regionFor({.column = 99, .row = 0, .level = 0}); // outside the grid
    /// assert(!bad.has_value());
    /// @endcode
    [[nodiscard]] constexpr Result<TileRegion> regionFor(const TileIndex& index) const {
        if (index.level >= levelCount) {
            return std::unexpected(
                Error{ErrorCode::OutOfRange, "TileLayout::regionFor: level out of range"});
        }
        if (index.column >= columns(index.level) || index.row >= rows(index.level)) {
            return std::unexpected(
                Error{ErrorCode::OutOfRange, "TileLayout::regionFor: index outside grid"});
        }
        return TileRegion{.x = index.column * tileSize.width,
                          .y = index.row * tileSize.height,
                          .extent = tileSize};
    }

    /// @brief Maps a pixel coordinate to the index of the tile that contains it.
    ///
    /// @param x The pixel's column coordinate at level \p level.
    /// @param y The pixel's row coordinate at level \p level.
    /// @param level The pyramid level to reason about (defaults to the base level 0).
    /// @return The tile @ref ptiff::io::tile::TileIndex "TileIndex" containing \c (x,y) on
    ///         success, or @ref ptiff::ErrorCode::OutOfRange "OutOfRange" if \p level is
    ///         outside this layout's level count, if (x, y) is outside the image at that level,
    ///         or if the layout is non-tiled (a tile dimension is zero).
    ///
    /// @code{.cpp}
    /// TileLayout l{.tileSize = {16, 16}, .imageWidth = 64, .imageHeight = 32};
    /// auto i = l.indexFor(17, 5);
    /// assert(i.has_value());
    /// assert(i->column == 1 && i->row == 0 && i->level == 0);
    /// @endcode
    [[nodiscard]] constexpr Result<TileIndex>
    indexFor(std::uint32_t x, std::uint32_t y, std::uint32_t level = 0) const {
        if (level >= levelCount) {
            return std::unexpected(
                Error{ErrorCode::OutOfRange, "TileLayout::indexFor: level out of range"});
        }
        if (tileSize.width == 0 || tileSize.height == 0 || x >= (imageWidth >> level) ||
            y >= (imageHeight >> level)) {
            return std::unexpected(
                Error{ErrorCode::OutOfRange, "TileLayout::indexFor: pixel outside image"});
        }
        return TileIndex{.column = x / tileSize.width, .row = y / tileSize.height, .level = level};
    }

    /// @brief Builds a single-level TileLayout from an image's descriptor.
    ///
    /// @param descriptor The image descriptor whose \c tileInfo (if present) seeds the layout.
    /// @return A single-level TileLayout on success, or
    ///         @ref ptiff::ErrorCode::InvalidArgument "InvalidArgument" if \p descriptor has no
    ///         \c tileInfo (i.e. the image isn't tiled).
    ///
    /// @note The layout this factory produces always has \c levelCount == 1. Multi-level
    ///       (pyramid) layouts are constructed directly or by higher-level orchestration on
    ///       top of this type; this helper intentionally keeps the common single-level case
    ///       trivial.
    ///
    /// @code{.cpp}
    /// ptiff::ImageDescriptor desc;
    /// desc.width  = 64;
    /// desc.height = 32;
    /// desc.tileInfo = ptiff::TileInfo{.tileWidth = 16, .tileHeight = 16};
    /// auto layout = TileLayout::fromDescriptor(desc);
    /// assert(layout.has_value());
    /// assert(layout->tileSize.width == 16 && layout->tileSize.height == 16);
    ///
    /// ptiff::ImageDescriptor untiled; // no tileInfo
    /// assert(!TileLayout::fromDescriptor(untiled).has_value());
    /// @endcode
    [[nodiscard]] static constexpr Result<TileLayout>
    fromDescriptor(const ImageDescriptor& descriptor) {
        if (!descriptor.tileInfo.has_value()) {
            return std::unexpected(Error{ErrorCode::InvalidArgument,
                                         "TileLayout::fromDescriptor: descriptor has no tileInfo"});
        }
        return TileLayout{.tileSize = {.width = descriptor.tileInfo->tileWidth,
                                       .height = descriptor.tileInfo->tileHeight},
                          .imageWidth = descriptor.width,
                          .imageHeight = descriptor.height,
                          .levelCount = 1};
    }

    /// Defaulted structural equality; two layouts are equal iff all of their members compare
    /// equal.
    friend constexpr bool operator==(const TileLayout&, const TileLayout&) = default;
};

} // namespace ptiff::io::tile
