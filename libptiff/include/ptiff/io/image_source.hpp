#pragma once

#include <ptiff/core/result.hpp>
#include <ptiff/export.hpp>
#include <ptiff/io/tile/tile.hpp>
#include <ptiff/io/tile/tile_index.hpp>
#include <ptiff/io/tile/tile_layout.hpp>

namespace ptiff::io {

/// @brief Pixel-data-level read abstraction over one image.
///
/// ImageSource exposes one image's data as an *indexable set of tiles* rather than as raw
/// bytes. This is what makes lazy, tile-based, streaming access possible without the caller
/// knowing any format's on-disk byte layout: callers ask only for the image's
/// @ref ptiff::io::tile::TileLayout "TileLayout" and then read individual
/// @ref ptiff::io::tile::Tile "Tile" objects by index.
///
/// @section image_source_contract Contract
///
/// - \c layout() describes how the image is tiled (tile size, grid, pyramid levels).
/// - \c readTile() returns the raw pixel bytes for one tile; the returned tile's `data()` is a
///   non-owning view the backend owns (see @ref ptiff::io::tile::Tile "Tile"'s zero-copy note).
///
/// @section image_source_thread Thread-safety
///
/// Not thread-safe. A concrete source typically wraps a single
/// @ref ptiff::io::BinaryReader "BinaryReader" and/or holds a cache; share one only if the
/// implementation documents that it is thread-safe.
///
/// @section image_source_example Example
///
/// @code{.cpp}
/// using ptiff::io::ImageSource;
/// using ptiff::io::tile::TileIndex;
///
/// // `src` is obtained from a StorageBackend (see openImageSource).
/// const auto& layout = src->layout();
/// assert(layout.columns() == 4 && layout.rows() == 2);
///
/// TileIndex idx{.column = 0, .row = 0, .level = 0};
/// auto tile = src->readTile(idx);
/// assert(tile.has_value());
/// assert(tile->index().column == 0 && tile->index().row == 0);
/// @endcode
///
/// @see @ref ptiff::io::ImageSink "ImageSink" (write mirror),
///      @ref ptiff::io::tile::TileLayout "TileLayout",
///      @ref ptiff::io::StorageBackend "StorageBackend".
class PTIFF_EXPORT ImageSource {
public:
    virtual ~ImageSource() = default;
    ImageSource(const ImageSource&) = delete;
    ImageSource& operator=(const ImageSource&) = delete;
    ImageSource(ImageSource&&) = delete;
    ImageSource& operator=(ImageSource&&) = delete;

    /// @brief Returns the authoritative tiling of the underlying image.
    /// @return A reference to the image's @ref ptiff::io::tile::TileLayout "TileLayout",
    ///         valid for the lifetime of this source.
    [[nodiscard]] virtual const tile::TileLayout& layout() const noexcept = 0;
    /// @brief Reads the raw pixel bytes for one tile.
    ///
    /// @param index The tile to read (column/row/level; validated against `layout()`).
    /// @return The @ref ptiff::io::tile::Tile "Tile" on success, or
    ///         @ref ptiff::ErrorCode::OutOfRange "OutOfRange" if \p index lies outside the
    ///         layout's grid, or a backend-specific error if the underlying read fails.
    ///
    /// @code{.cpp}
    /// auto tile = src->readTile({.column = 1, .row = 1, .level = 0});
    /// assert(tile.has_value());
    /// @endcode
    [[nodiscard]] virtual Result<tile::Tile> readTile(const tile::TileIndex& index) = 0;

protected:
    ImageSource() = default;
};

} // namespace ptiff::io
