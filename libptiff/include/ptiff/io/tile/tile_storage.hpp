#pragma once

#include <ptiff/core/result.hpp>
#include <ptiff/export.hpp>
#include <ptiff/io/tile/tile.hpp>
#include <ptiff/io/tile/tile_index.hpp>

namespace ptiff::io::tile {

/// @brief The physical backing store one tile lives in behind a StorageBackend.
///
/// A TileStorage abstracts a single tile's persistence: a file region, a memory blob, a cloud
/// object. A StorageBackend uses it to load from, and store to, the concrete transport without
/// knowing its details. This keeps the byte-level read/write concerns (via
/// @ref ptiff::io::BinaryReader "BinaryReader" / @ref ptiff::io::BinaryWriter "BinaryWriter")
/// separate from the higher-level tile addressing a backend implements.
///
/// @section tile_storage_threading Thread safety
///
/// A TileStorage is **not thread-safe**: implementations typically wrap a single
/// @ref ptiff::io::BinaryReader "BinaryReader" / @ref ptiff::io::BinaryWriter "BinaryWriter"
/// with a mutable cursor, inheriting the same thread-*compatible*-but-not-shared contract as
/// those interfaces. Callers must serialize access (or use one storage per thread/stream).
///
/// @section tile_storage_example Example
///
/// @code{.cpp}
/// using ptiff::io::tile::Tile;
/// using ptiff::io::tile::TileIndex;
/// using ptiff::io::tile::TileRegion;
/// using ptiff::io::tile::TileStorage;
///
/// TileStorage& storage = /* concrete backing store */;
///
/// // Load tile (column 2, row 1) of the base level.
/// auto tile = storage.load(TileIndex{.column = 2, .row = 1, .level = 0});
/// if (tile.has_value()) {
///     assert(tile->index().column == 2 && tile->index().row == 1);
/// } else {
///     // handle read error (e.g. ptiff::ErrorCode::NotFound on a missing region)
/// }
///
/// // Store a tile produced elsewhere (a render, a decode, ...).
/// std::vector<std::byte> pixels(256, std::byte{0x5A});
/// Tile toWrite(ptiff::TileId{4},
///              TileIndex{.column = 0, .row = 0, .level = 0},
///              TileRegion{.x = 0, .y = 0, .extent = {.width = 16, .height = 16}},
///              pixels);
/// assert(storage.store(toWrite).has_value());
/// @endcode
class PTIFF_EXPORT TileStorage {
public:
    virtual ~TileStorage() = default;
    TileStorage(const TileStorage&) = delete;
    TileStorage& operator=(const TileStorage&) = delete;
    TileStorage(TileStorage&&) = delete;
    TileStorage& operator=(TileStorage&&) = delete;

    /// @brief Reads the tile at \p index into a @ref ptiff::io::tile::Tile "Tile".
    ///
    /// @param index The grid position (@ref ptiff::io::tile::TileIndex "TileIndex") of the tile
    ///              to read.
    /// @return The loaded Tile on success, or an error on failure (e.g.
    ///         @ref ptiff::ErrorCode::NotFound "NotFound" if the region does not exist,
    ///         @ref ptiff::ErrorCode::InvalidArgument "InvalidArgument" on a malformed payload).
    [[nodiscard]] virtual Result<Tile> load(const TileIndex& index) = 0;

    /// @brief Writes \p tile to the backing store at the tile's grid position.
    ///
    /// @param tile The tile to persist (its @ref ptiff::io::tile::Tile::index "index()" selects
    ///             the destination region).
    /// @return A success on success, or an error on failure (e.g.
    ///         @ref ptiff::ErrorCode::InvalidArgument "InvalidArgument" for an out-of-range
    ///         index or a store that cannot be addressed).
    [[nodiscard]] virtual Result<void> store(const Tile& tile) = 0;

protected:
    TileStorage() = default;
};

} // namespace ptiff::io::tile
