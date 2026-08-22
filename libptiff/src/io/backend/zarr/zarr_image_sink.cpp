#include <array>
#include <limits>
#include <span>
#include <utility>
#include <vector>

#include <ptiff/io/backend/zarr/zarr_codec.hpp>
#include <ptiff/io/backend/zarr/zarr_image_sink.hpp>

namespace ptiff::io::backend::zarr {

ZarrImageSink::ZarrImageSink(io::BinaryWriter& writer, ZarrLayout layout, std::uint64_t pixelRegion)
    : writer_(writer), layout_(std::move(layout)), pixelRegion_(pixelRegion) {}

const io::tile::TileLayout& ZarrImageSink::layout() const noexcept {
    return layout_.layout;
}

Result<void> ZarrImageSink::writeTile(const io::tile::Tile& tile) {
    auto region = layout_.layout.regionFor(tile.index());
    if (!region.has_value()) {
        return std::unexpected(region.error());
    }
    if (tile.data().size() != layout_.chunkBytes) {
        return std::unexpected(Error{ErrorCode::InvalidArgument,
                                     "zarr: tile byte count does not match the chunk size"});
    }
    if (layout_.slotSize == 0 || layout_.slotSize < 4) {
        return std::unexpected(Error{ErrorCode::InvalidArgument, "zarr: invalid slot size"});
    }
    const std::uint32_t columns = layout_.layout.columns(tile.index().level);
    const std::uint64_t linear =
        static_cast<std::uint64_t>(tile.index().row) * columns + tile.index().column;
    if (linear > (std::numeric_limits<std::uint64_t>::max() - pixelRegion_) / layout_.slotSize) {
        return std::unexpected(Error{ErrorCode::InvalidArgument, "zarr: chunk offset overflows"});
    }
    const std::uint64_t chunkOffset = pixelRegion_ + linear * layout_.slotSize;

    auto compressed = compress(layout_.compressor, tile.data());
    if (!compressed.has_value()) {
        return std::unexpected(compressed.error());
    }
    if (4 + compressed->size() > layout_.slotSize) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "zarr: compressed chunk exceeds its slot"});
    }

    std::array<std::byte, 4> lenBytes{};
    std::uint32_t len = static_cast<std::uint32_t>(compressed->size());
    for (std::size_t i = 0; i < 4; ++i) {
        lenBytes[i] = static_cast<std::byte>((len >> (8 * i)) & 0xFF);
    }

    auto seekResult = writer_.seek(chunkOffset);
    if (!seekResult.has_value()) {
        return std::unexpected(seekResult.error());
    }
    auto writeLen = writer_.write(std::span<const std::byte>{lenBytes});
    if (!writeLen.has_value() || *writeLen != lenBytes.size()) {
        return std::unexpected(Error{ErrorCode::InvalidArgument, "zarr: short length write"});
    }
    auto writePayload = writer_.write(std::span<const std::byte>{*compressed});
    if (!writePayload.has_value() || *writePayload != compressed->size()) {
        return std::unexpected(Error{ErrorCode::InvalidArgument, "zarr: short payload write"});
    }

    // Pad the slot to its fixed size so every slot occupies contiguous on-disk bytes and a
    // later slot's offset is always a valid seek target (MemoryBinaryWriter refuses to seek
    // beyond the current buffer extent).
    const std::size_t padding = layout_.slotSize - 4u - compressed->size();
    if (padding > 0) {
        std::vector<std::byte> zeros(padding, std::byte{0});
        auto writePad = writer_.write(std::span<const std::byte>{zeros});
        if (!writePad.has_value() || *writePad != padding) {
            return std::unexpected(Error{ErrorCode::InvalidArgument, "zarr: short padding write"});
        }
    }
    return {};
}

} // namespace ptiff::io::backend::zarr
