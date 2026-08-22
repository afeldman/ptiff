#pragma once

#include <ptiff/core/result.hpp>
#include <ptiff/export.hpp>
#include <ptiff/io/tile/tile.hpp>
#include <ptiff/io/tile/tile_layout.hpp>

namespace ptiff::io {

/// @brief Pixel-data-level write abstraction over one image.
///
/// ImageSink is the write-side counterpart to @ref ptiff::io::ImageSource "ImageSource": it
/// consumes an image one @ref ptiff::io::tile::Tile "Tile" at a time, so a caller can stream
/// pixels into a file without knowing the target format's on-disk byte layout. Edge tiles may
/// be padded to the full tile size, matching the contract of
/// @ref ptiff::io::tile::TileLayout "TileLayout".
///
/// @section image_sink_contract Contract
///
/// - \c layout() describes how the image is tiled; tiles handed to \c writeTile() must agree
///   with this layout.
/// - \c writeTile() persists one tile's pixel bytes, returning the outcome.
///
/// @section image_sink_thread Thread-safety
///
/// Mirrors @ref ptiff::io::ImageSource "ImageSource": not thread-safe by default, since a
/// concrete sink typically wraps a single mutable @ref ptiff::io::BinaryWriter "BinaryWriter".
///
/// @section image_sink_example Example
///
/// @code{.cpp}
/// using ptiff::io::ImageSink;
/// using ptiff::io::tile::Tile;
/// using ptiff::io::tile::TileIndex;
/// using ptiff::io::tile::TileRegion;
///
/// // `sink` is obtained from a StorageBackend (see openImageSink).
/// const auto& layout = sink->layout();
/// std::vector<std::byte> pixels(16 * 16, std::byte{0});
/// Tile t(ptiff::TileId{0},
///        TileIndex{.column = 2, .row = 1, .level = 0},
///        TileRegion{.x = 32, .y = 16, .extent = {.width = 16, .height = 16}},
///        pixels);
/// auto ok = sink->writeTile(t);
/// assert(ok.has_value());
/// @endcode
///
/// @see @ref ptiff::io::ImageSource "ImageSource" (read mirror),
///      @ref ptiff::io::tile::Tile "Tile",
///      @ref ptiff::io::StorageBackend "StorageBackend".
class PTIFF_EXPORT ImageSink {
public:
    virtual ~ImageSink() = default;
    ImageSink(const ImageSink&) = delete;
    ImageSink& operator=(const ImageSink&) = delete;
    ImageSink(ImageSink&&) = delete;
    ImageSink& operator=(ImageSink&&) = delete;

    /// @brief Returns the authoritative tiling of the target image.
    /// @return A reference to the image's @ref ptiff::io::tile::TileLayout "TileLayout",
    ///         valid for the lifetime of this sink.
    [[nodiscard]] virtual const tile::TileLayout& layout() const noexcept = 0;
    /// @brief Persists one tile's pixel bytes.
    ///
    /// @param tile The tile to write; its index and region must agree with `layout()`.
    /// @return `Result<void>` success once the tile is persisted, or
    ///         @ref ptiff::ErrorCode::OutOfRange "OutOfRange" if \p tile's index lies outside
    ///         the layout's grid, or a backend-specific error if the underlying write fails.
    [[nodiscard]] virtual Result<void> writeTile(const tile::Tile& tile) = 0;

protected:
    ImageSink() = default;
};

} // namespace ptiff::io
