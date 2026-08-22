#pragma once

#include <ptiff/core/result.hpp>
#include <ptiff/export.hpp>
#include <ptiff/io/tile/tile.hpp>
#include <ptiff/io/tile/tile_index.hpp>
#include <ptiff/io/tile/tile_layout.hpp>

namespace ptiff::io {

/// @brief Pixel-data-level write source over one image.
///
/// The write-side mirror of @ref ptiff::io::ImageSource "ImageSource". A `TileProvider` knows
/// how to produce the raw pixel bytes for one strip/tile of the image being written, but is
/// metadata-free and stateless from the @ref ptiff::io::StorageBackend "StorageBackend"'s
/// perspective: the @ref ptiff::Writer "Writer" derives the authoritative tiling from the
/// backend's @ref ptiff::io::ImageSink "ImageSink" `layout()`, then asks the provider for each
/// tile in turn and hands it to `ImageSink::writeTile`.
///
/// @section provider_metadata Metadata lives in the Scene, not the provider
///
/// The image's geometry and storage metadata (width, height, pixel type, channel count,
/// compression, tiling) come from the @ref ptiff::Scene "Scene" being written. The provider
/// supplies only pixels: raw, uncompressed, row-major tile bytes. The caller is responsible for
/// producing tile data whose byte count matches the target layout (see
/// @ref tile_byte_count "tile byte count").
///
/// @section tile_byte_count Tile byte count
///
/// @ref ptiff::io::ImageSink "ImageSink" validates that each tile handed to `writeTile` carries
/// the exact byte count the backend derives from the layout:
///
/// - **Stripped** (one strip / whole image): `width × height × samplesPerPixel × bytesPerSample`.
/// - **Tiled**: `tileWidth × tileHeight × samplesPerPixel × bytesPerSample`; every tile --
///   including edge tiles -- must carry the **full** tile size (the backend does not crop). The
///   provider pads edge-cropped *content* to full tile size.
///
/// @section provider_lifetime Zero-copy by design
///
/// The tiles a provider returns are **non-owning** views (`Tile::data()` is a
/// `std::span<const std::byte>`). The `Writer` consumes each tile synchronously -- `provideTile`
/// is immediately followed by `ImageSink::writeTile` -- so a provider's pixel buffer only needs
/// to live until that single `writeTile` completes. The provider may reuse or overwrite its
/// buffer for the next tile.
///
/// @code{.cpp}
/// using ptiff::io::tile::Tile;
/// using ptiff::io::tile::TileIndex;
///
/// class SimpleProvider final : public ptiff::io::TileProvider {
/// public:
///     const ptiff::io::tile::TileLayout& layout() const noexcept override { return layout_; }
///     ptiff::Result<Tile> provideTile(const TileIndex& index) override {
///         auto region = layout_.regionFor(index);
///         if (!region.has_value()) {
///             return std::unexpected(region.error());
///         }
///         // One tile's worth of raw bytes, produced from a caller-owned buffer.
///         return Tile(ptiff::TileId{0}, index, *region, bytes_);
///     }
///
/// private:
///     ptiff::io::tile::TileLayout layout_;
///     std::span<const std::byte> bytes_;
/// };
/// @endcode
///
/// @see @ref ptiff::io::ImageSource "ImageSource" (read mirror),
///      @ref ptiff::io::ImageSink "ImageSink",
///      @ref ptiff::io::tile::TileLayout "TileLayout",
///      @ref ptiff::Reader "Reader", @ref ptiff::Writer "Writer".
class PTIFF_EXPORT TileProvider {
public:
    virtual ~TileProvider() = default;
    TileProvider(const TileProvider&) = delete;
    TileProvider& operator=(const TileProvider&) = delete;
    TileProvider(TileProvider&&) = delete;
    TileProvider& operator=(TileProvider&&) = delete;

    /// @brief Returns the tiling this provider can supply tiles for.
    ///
    /// Must agree with the layout the @ref ptiff::io::StorageBackend "StorageBackend" derives
    /// from the @ref ptiff::Scene "Scene" being written; the @ref ptiff::Writer "Writer"
    /// validates agreement before writing any pixels.
    ///
    /// @return A reference to the image's @ref ptiff::io::tile::TileLayout "TileLayout", valid
    ///         for the lifetime of this provider.
    [[nodiscard]] virtual const tile::TileLayout& layout() const noexcept = 0;

    /// @brief Returns the raw pixel bytes for one strip/tile.
    ///
    /// @param index The strip/tile to produce (column/row/level). If it lies outside this
    ///              provider's layout, return @ref ptiff::ErrorCode::OutOfRange "OutOfRange".
    /// @return The @ref ptiff::io::tile::Tile "Tile" on success, or a backend-agnostic
    ///         @ref ptiff::ErrorCode "ErrorCode" otherwise. The returned tile's `data()` is a
    ///         non-owning view valid only until the writer's next `writeTile` (see the
    ///         zero-copy section above).
    [[nodiscard]] virtual Result<tile::Tile> provideTile(const tile::TileIndex& index) = 0;

protected:
    TileProvider() = default;
};

} // namespace ptiff::io
