#pragma once

#include <ptiff/core/result.hpp>
#include <ptiff/export.hpp>
#include <ptiff/io/tile/tile_index.hpp>

namespace ptiff::io::tile {

/// @brief Sequential or random traversal over a TileLayout's grid.
///
/// TileIterator abstracts how tile positions are enumerated by the streaming modes of a backend.
/// The same interface serves both the "Sequential Access" and "Random Access" traversal modes
/// from the design brief; which one a concrete iterator implements is an implementation detail
/// invisible to callers. A caller only ever sees a TileIterator pointer and drives it uniformly
/// with @ref hasNext / @ref next.
///
/// @section tile_iterator_state Mutable traversal state
///
/// A TileIterator is **not thread-safe**: it holds mutable traversal state (a current position),
/// so it must not be shared across threads. Concurrent consumers obtain their own iterators or
/// serialize access externally.
///
/// @section tile_iterator_loop Idiomatic consumption
///
/// @code{.cpp}
/// using ptiff::io::tile::TileIterator;
///
/// // 'it' is obtained from a backend's iteration entry point (not part of this interface).
/// TileIterator& it = /* ... */;
///
/// std::vector<ptiff::io::tile::TileIndex> visited;
/// for (it.reset(); it.hasNext(); ) {
///     auto index = it.next();
///     assert(index.has_value());
///     visited.push_back(*index);
/// }
///
/// // Fresh iteration resumes from the beginning after reset().
/// it.reset();
/// assert(it.hasNext() == visited.size() > 0);
/// @endcode
class PTIFF_EXPORT TileIterator {
public:
    virtual ~TileIterator() = default;
    TileIterator(const TileIterator&) = delete;
    TileIterator& operator=(const TileIterator&) = delete;
    TileIterator(TileIterator&&) = delete;
    TileIterator& operator=(TileIterator&&) = delete;

    /// @brief Reports whether another tile position remains in this iteration.
    ///
    /// @return \c true if a subsequent @ref next is expected to succeed, \c false once the
    ///         iterator has been fully drained.
    [[nodiscard]] virtual bool hasNext() const noexcept = 0;

    /// @brief Advances to and returns the next tile position.
    ///
    /// @return The next @ref ptiff::io::tile::TileIndex "TileIndex" in traversal order, or an
    ///         error (e.g. @ref ptiff::ErrorCode::OutOfRange "OutOfRange") if the iterator is
    ///         exhausted -- call @ref hasNext before this to avoid that case in normal flows.
    [[nodiscard]] virtual Result<TileIndex> next() = 0;

    /// @brief Rewinds the iterator to its initial state so traversal can begin again.
    ///
    /// @note No-op semantics across implementations must make a subsequent @ref hasNext reflect
    ///       a fresh traversal (i.e. return \c true if the grid is non-empty).
    virtual void reset() noexcept = 0;

protected:
    TileIterator() = default;
};

} // namespace ptiff::io::tile
