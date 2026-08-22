#pragma once

// Per-image chunk reader for a Zarr-style single-file container. Each ptiff tile is one Zarr chunk;
// readTile decompresses the tile bytes from that chunk's fixed slot (length prefix + payload) and
// returns them as a Tile view. The returned Tile view is valid until the next readTile call or
// until this source is destroyed (the source owns its reusable buffer).

#include <cstdint>
#include <vector>

#include <ptiff/io/backend/zarr/zarr_document.hpp>
#include <ptiff/io/binary_reader.hpp>
#include <ptiff/io/image_source.hpp>

namespace ptiff::io::backend::zarr {

/// @brief Per-image chunk reader for a Zarr-style single-file container.
///
/// Each ptiff tile is one Zarr chunk; `readTile` decompresses the tile bytes from that chunk's
/// fixed slot (length prefix + payload) and returns them as a `Tile` view.
class ZarrImageSource final : public io::ImageSource {
public:
    /// @brief Constructs the source over a binary reader and the resolved chunk layout.
    ///
    /// @param reader The underlying binary reader for the container file.
    /// @param layout The @ref ptiff::io::backend::zarr::ZarrLayout "ZarrLayout" describing chunk
    ///               placement and the container's tile grid.
    /// @param pixelRegion The byte size of one fully-populated tile's uncompressed pixel payload.
    ZarrImageSource(io::BinaryReader& reader, ZarrLayout layout, std::uint64_t pixelRegion);

    /// @brief Returns the underlying tile layout of the container.
    [[nodiscard]] const io::tile::TileLayout& layout() const noexcept override;

    /// @brief Reads one tile from its fixed chunk slot, decompressing the payload.
    ///
    /// @param index The index of the tile to read.
    /// @return A `Tile` view over the decompressed pixels. The view remains valid until the next
    ///         `readTile` call or until this source is destroyed.
    [[nodiscard]] Result<io::tile::Tile> readTile(const io::tile::TileIndex& index) override;

private:
    io::BinaryReader& reader_;
    ZarrLayout layout_;
    std::uint64_t pixelRegion_;
    std::vector<std::byte> buffer_;
};

} // namespace ptiff::io::backend::zarr
