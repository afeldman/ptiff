#include <limits>
#include <utility>

#include <ptiff/io/backend/memory/memory_image_sink.hpp>

namespace ptiff::io::backend::memory {

MemoryImageSink::MemoryImageSink(BinaryWriter& writer,
                                 MemoryImageInfo info,
                                 std::uint64_t imagePixelStart)
    : writer_(writer), info_(std::move(info)), imagePixelStart_(imagePixelStart) {}

const io::tile::TileLayout& MemoryImageSink::layout() const noexcept {
    return info_.layout;
}

Result<void> MemoryImageSink::writeTile(const io::tile::Tile& tile) {
    auto region = info_.layout.regionFor(tile.index());
    if (!region.has_value()) {
        return std::unexpected(region.error());
    }

    const std::uint32_t columns = info_.layout.columns(tile.index().level);
    const std::uint64_t linearIndex =
        static_cast<std::uint64_t>(tile.index().row) * columns + tile.index().column;

    if (tile.data().size() != info_.tileBytes) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument,
                  "MemoryImageSink::writeTile: tile byte count does not match the layout"});
    }

    if (info_.tileBytes == 0) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "MemoryImageSink::writeTile: zero tile size"});
    }
    if (linearIndex > std::numeric_limits<std::uint64_t>::max() / info_.tileBytes) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument,
                  "MemoryImageSink::writeTile: tile offset computation overflows"});
    }
    const std::uint64_t tileOffset = linearIndex * info_.tileBytes;
    if (imagePixelStart_ > std::numeric_limits<std::uint64_t>::max() - tileOffset) {
        return std::unexpected(Error{ErrorCode::InvalidArgument,
                                     "MemoryImageSink::writeTile: absolute tile offset overflows"});
    }

    auto seekResult = writer_.seek(imagePixelStart_ + tileOffset);
    if (!seekResult.has_value()) {
        return std::unexpected(seekResult.error());
    }
    return writer_.write(tile.data()).and_then([&](std::size_t written) -> Result<void> {
        if (written != tile.data().size()) {
            return std::unexpected(
                Error{ErrorCode::InvalidArgument, "MemoryImageSink::writeTile: short write"});
        }
        return {};
    });
}

} // namespace ptiff::io::backend::memory
