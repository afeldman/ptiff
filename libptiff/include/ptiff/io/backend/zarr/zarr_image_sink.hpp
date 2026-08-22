#pragma once

// Per-image chunk writer for a Zarr-style single-file container. Each ptiff tile is one Zarr chunk;
// writeTile compresses the tile bytes and stores them in that chunk's fixed slot (length prefix +
// payload). Tiles may be written in any order.

#include <cstdint>

#include <ptiff/io/backend/zarr/zarr_document.hpp>
#include <ptiff/io/binary_writer.hpp>
#include <ptiff/io/image_sink.hpp>

namespace ptiff::io::backend::zarr {

/// @brief Per-image chunk writer for a Zarr-style single-file container.
///
/// Each ptiff tile is one Zarr chunk; `writeTile` computes the chunk's fixed slot (length prefix
/// + payload) in the container and writes it there. Tiles may be written in any order.
class ZarrImageSink final : public io::ImageSink {
public:
    /// @brief Constructs the sink over a binary writer and the resolved chunk layout.
    ///
    /// @param writer The underlying binary writer for the container file.
    /// @param layout The @ref ptiff::io::backend::zarr::ZarrLayout "ZarrLayout" describing chunk
    ///               placement and the container's tile grid.
    /// @param pixelRegion The byte size of one fully-populated tile's uncompressed pixel payload.
    ZarrImageSink(io::BinaryWriter& writer, ZarrLayout layout, std::uint64_t pixelRegion);

    /// @brief Returns the underlying tile layout of the container.
    [[nodiscard]] const io::tile::TileLayout& layout() const noexcept override;

    /// @brief Writes one tile into its fixed chunk slot.
    ///
    /// MAY be called in any order and any number of times, including repeated writes to the same
    /// chunk (overwriting the previous contents).
    ///
    /// @param tile The tile whose pixels are stored (length-prefixed) in its chunk slot.
    /// @return `Result<void>` on success, or an error if the write fails.
    [[nodiscard]] Result<void> writeTile(const io::tile::Tile& tile) override;

private:
    io::BinaryWriter& writer_;
    ZarrLayout layout_;
    std::uint64_t pixelRegion_;
};

} // namespace ptiff::io::backend::zarr
