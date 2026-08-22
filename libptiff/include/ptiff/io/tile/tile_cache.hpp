#pragma once

#include <optional>

#include <ptiff/core/id.hpp>
#include <ptiff/export.hpp>
#include <ptiff/io/tile/tile.hpp>

namespace ptiff::io::tile {

/// @brief Pluggable tile-caching strategy (Strategy pattern).
///
/// TileCache lets a backend cache decoded/recently-used tiles without prescribing *how* eviction
/// works. The interface exposes only the three operations every cache needs: @ref find, @ref
/// insert and @ref clear. Which eviction policy is used (LRU, size-bounded, FIFO, ...) is
/// deliberately not decided here -- that is a real algorithm for a later sprint and is chosen by
/// the concrete implementation (or injected via a factory).
///
/// @section tile_cache_ownership Lifetime contract
///
/// The @ref ptiff::io::tile::Tile "Tile"s handed back by @ref find are **non-owning** views, as
/// Tile always is. A cache that retains tile byte buffers keeps them alive for the duration of
/// the cached entry; a caller must therefore not use a returned Tile after it has been @ref clear
/// ed or evicted. Caches that do *not* buffer bytes (a pure hit-counting decorator, for example)
/// simply return a tile still owned elsewhere.
///
/// @section tile_cache_threading Thread safety
///
/// Thread safety is a **per-implementation** contract; this interface carries no shared state of
/// its own. A thread-safe cache documents its guarantees explicitly; a single-threaded cache
/// relies on external synchronization.
///
/// @section tile_cache_example Example
///
/// @code{.cpp}
/// using ptiff::io::tile::TileCache;
/// using ptiff::io::tile::Tile;
/// using ptiff::io::tile::TileIndex;
/// using ptiff::io::tile::TileRegion;
///
/// TileCache& cache = /* concrete cache instance */;
///
/// // Store a freshly-loaded tile by its id.
/// std::vector<std::byte> pixels(256, std::byte{9});
/// Tile loaded(ptiff::TileId{3},
///             TileIndex{.column = 0, .row = 0, .level = 0},
///             TileRegion{.x = 0, .y = 0, .extent = {.width = 16, .height = 16}},
///             pixels);
/// cache.insert(loaded);
///
/// // A later lookup can avoid a re-read from disk.
/// auto hit = cache.find(ptiff::TileId{3});
/// if (hit.has_value()) {
///     assert(hit->id() == ptiff::TileId{3});
/// } else {
///     // cache miss: (re-)load from backing storage, then insert() again.
/// }
///
/// cache.clear(); // drop every cached tile; returned Tiles become invalid
/// @endcode
class PTIFF_EXPORT TileCache {
public:
    virtual ~TileCache() = default;
    TileCache(const TileCache&) = delete;
    TileCache& operator=(const TileCache&) = delete;
    TileCache(TileCache&&) = delete;
    TileCache& operator=(TileCache&&) = delete;

    /// @brief Looks up a tile by its unique identifier.
    ///
    /// @param id The @ref ptiff::TileId "TileId" of the requested tile.
    /// @return \c std::nullopt if the tile is not cached (a cache miss); otherwise the cached
    ///         @ref ptiff::io::tile::Tile "Tile". The returned tile is only valid until it is
    ///         evicted or @ref clear is called.
    [[nodiscard]] virtual std::optional<Tile> find(TileId id) const = 0;

    /// @brief Inserts (or replaces) a tile in the cache under its id.
    ///
    /// @param tile The tile to cache. Its @ref ptiff::io::tile::Tile::id "id()" selects the
    ///             cache key; implementations may evict other entries to honour their policy.
    ///
    /// @note Whether the cache *copies* the tile's backing bytes or holds the given view is an
    ///       implementation detail; see @ref tile_cache_ownership for the lifetime contract.
    virtual void insert(Tile tile) = 0;

    /// @brief Removes every cached tile.
    ///
    /// @note Any @ref ptiff::io::tile::Tile "Tile" the cache handed out before this call becomes
    ///       invalid (its backing store may be released).
    virtual void clear() = 0;

protected:
    TileCache() = default;
};

} // namespace ptiff::io::tile
