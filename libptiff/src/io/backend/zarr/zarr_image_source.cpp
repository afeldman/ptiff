#include <limits>
#include <span>
#include <utility>

#include <ptiff/io/backend/zarr/zarr_codec.hpp>
#include <ptiff/io/backend/zarr/zarr_image_source.hpp>

namespace ptiff::io::backend::zarr {

namespace {
// Computes the absolute byte offset of chunk `index` within the document, given `pixelRegion`
// (start of the chunk block) and `slotSize`. Returns an error on arithmetic overflow.
Result<std::uint64_t> chunkOffset(std::uint64_t pixelRegion,
                                  std::uint32_t slotSize,
                                  std::uint32_t columns,
                                  const io::tile::TileIndex& index) {
    const std::uint64_t linear = static_cast<std::uint64_t>(index.row) * columns + index.column;
    if (linear > (std::numeric_limits<std::uint64_t>::max() - pixelRegion) / slotSize) {
        return std::unexpected(Error{ErrorCode::InvalidArgument, "zarr: chunk offset overflows"});
    }
    return pixelRegion + linear * slotSize;
}
} // namespace

ZarrImageSource::ZarrImageSource(io::BinaryReader& reader,
                                 ZarrLayout layout,
                                 std::uint64_t pixelRegion)
    : reader_(reader), layout_(std::move(layout)), pixelRegion_(pixelRegion) {}

const io::tile::TileLayout& ZarrImageSource::layout() const noexcept {
    return layout_.layout;
}

Result<io::tile::Tile> ZarrImageSource::readTile(const io::tile::TileIndex& index) {
    auto region = layout_.layout.regionFor(index);
    if (!region.has_value()) {
        return std::unexpected(region.error());
    }
    if (layout_.slotSize == 0 || layout_.chunkBytes == 0) {
        return std::unexpected(Error{ErrorCode::InvalidArgument, "zarr: zero chunk geometry"});
    }
    const std::uint32_t columns = layout_.layout.columns(index.level);
    auto offsetResult = chunkOffset(pixelRegion_, layout_.slotSize, columns, index);
    if (!offsetResult.has_value()) {
        return std::unexpected(offsetResult.error());
    }
    const std::uint64_t chunkOffset = *offsetResult;

    // Bound-check against the reader size (slot start + slotSize).
    auto fileSize = reader_.size();
    if (!fileSize.has_value()) {
        return std::unexpected(fileSize.error());
    }
    if (chunkOffset > *fileSize || layout_.slotSize > *fileSize - chunkOffset) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "zarr: chunk slot exceeds reader size"});
    }

    // Length prefix (actual payload length).
    std::array<std::byte, 4> lenBytes{};
    auto seekResult = reader_.seek(chunkOffset);
    if (!seekResult.has_value()) {
        return std::unexpected(seekResult.error());
    }
    auto readLen = reader_.read(std::span<std::byte>{lenBytes});
    if (!readLen.has_value() || *readLen != lenBytes.size()) {
        return std::unexpected(Error{ErrorCode::InvalidArgument, "zarr: truncated chunk length"});
    }
    std::uint32_t payloadLen = 0;
    for (std::size_t i = 0; i < 4; ++i) {
        payloadLen |= static_cast<std::uint32_t>(std::to_integer<unsigned char>(lenBytes[i]))
                      << (8 * i);
    }
    if (payloadLen == 0 || payloadLen > layout_.slotSize - 4) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "zarr: invalid chunk payload length"});
    }

    std::vector<std::byte> payload(payloadLen);
    auto readPayload = reader_.read(std::span<std::byte>{payload});
    if (!readPayload.has_value() || *readPayload != payloadLen) {
        return std::unexpected(Error{ErrorCode::InvalidArgument, "zarr: truncated chunk payload"});
    }

    auto pixels = decompress(layout_.compressor, payload, layout_.chunkBytes);
    if (!pixels.has_value()) {
        return std::unexpected(pixels.error());
    }
    buffer_ = std::move(*pixels);
    return io::tile::Tile{
        ptiff::TileId(static_cast<std::uint64_t>(index.row) * columns + index.column),
        index,
        *region,
        std::span<const std::byte>{buffer_}};
}

} // namespace ptiff::io::backend::zarr
