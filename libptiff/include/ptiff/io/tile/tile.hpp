#pragma once

#include <cstddef>
#include <span>

#include <ptiff/core/id.hpp>
#include <ptiff/io/tile/tile_index.hpp>
#include <ptiff/io/tile/tile_region.hpp>

namespace ptiff::io::tile {

/// @brief One tile's identity, position, and pixel data.
///
/// A Tile bundles everything a storage layer needs to reason about a single unit of transfer:
/// a unique @ref ptiff::TileId "TileId", its position in the grid
/// (@ref ptiff::io::tile::TileIndex "TileIndex"), the pixel rectangle it covers
/// (@ref ptiff::io::tile::TileRegion "TileRegion"), and a view over the raw pixel bytes.
///
/// @section tile_buffering Zero-copy by design
///
/// `data()` is a **non-owning** view (`std::span<const std::byte>`): the tile itself never
/// allocates or copies pixel bytes. The view is only valid while whatever produced it is still
/// alive -- an @ref ptiff::io::ImageSource "ImageSource", a @ref ptiff::io::tile::TileCache
/// "TileCache" entry, or a caller-owned buffer. Consequence: do not retain a Tile (or its `data`)
/// beyond the lifetime of the object that handed it to you.
///
/// @section tile_example Example
///
/// @code{.cpp}
/// using ptiff::io::tile::Tile;
/// using ptiff::io::tile::TileIndex;
/// using ptiff::io::tile::TileRegion;
///
/// // 16x16 UInt8 tile fully inlined into 256 bytes, produced by a caller-owned buffer.
/// std::vector<std::byte> pixels(256, std::byte{0x41});
/// Tile t(ptiff::TileId{7},
///        TileIndex{.column = 2, .row = 1, .level = 0},
///        TileRegion{.x = 32, .y = 16, .extent = {.width = 16, .height = 16}},
///        pixels);
///
/// assert(t.id().value() == 7);
/// assert(t.index().column == 2 && t.index().row == 1);
/// assert(t.region().extent.width == 16);
/// assert(t.data().size() == 256);
/// assert(t.data()[0] == std::byte{0x41}); // zero-copy: views the same buffer, not a copy
/// @endcode
///
/// @see @ref ptiff::io::tile::TileIndex "TileIndex",
///      @ref ptiff::io::tile::TileRegion "TileRegion", @ref ptiff::io::tile::TileCache "TileCache".
class Tile {
public:
    /// @brief Builds a tile from its identity, grid position, covered region and a data view.
    ///
    /// @param id    The tile's unique identifier within its backend.
    /// @param index The tile's position (column/row/level) within its grid.
    /// @param region The pixel-space rectangle the tile covers.
    /// @param data  A non-owning view over the tile's raw pixel bytes. Must outlive this Tile.
    ///
    /// @note No copies are made of \p data; the caller retains ownership.
    constexpr Tile(TileId id,
                   TileIndex index,
                   TileRegion region,
                   std::span<const std::byte> data) noexcept
        : id_(id), index_(index), region_(region), data_(data) {}

    /// @brief Returns the tile's unique identifier.
    ///
    /// @return The @ref ptiff::TileId "TileId" this tile was constructed with.
    [[nodiscard]] constexpr TileId id() const noexcept { return id_; }

    /// @brief Returns the tile's position within its grid.
    ///
    /// @return A const reference to the tile's @ref ptiff::io::tile::TileIndex "TileIndex".
    [[nodiscard]] constexpr const TileIndex& index() const noexcept { return index_; }

    /// @brief Returns the pixel-space rectangle the tile covers.
    ///
    /// @return A const reference to the tile's @ref ptiff::io::tile::TileRegion "TileRegion".
    [[nodiscard]] constexpr const TileRegion& region() const noexcept { return region_; }

    /// @brief Returns a non-owning view over the tile's raw pixel bytes.
    ///
    /// @return A `std::span<const std::byte>` over the pixel data passed at construction. The
    ///         view is valid only while the underlying buffer is alive; see @ref tile_buffering
    ///         for the lifetime contract.
    ///
    /// @code{.cpp}
    /// std::vector<std::byte> pixels(256, std::byte{0});
    /// Tile t(ptiff::TileId{1}, {}, {}, pixels);
    /// assert(t.data().data() == pixels.data()); // zero-copy view, not a copy
    /// @endcode
    [[nodiscard]] constexpr std::span<const std::byte> data() const noexcept { return data_; }

private:
    TileId id_;
    TileIndex index_;
    TileRegion region_;
    std::span<const std::byte> data_;
};

} // namespace ptiff::io::tile
