#include <limits>
#include <span>
#include <utility>

#include <ptiff/io/backend/memory/memory_image_source.hpp>

namespace ptiff::io::backend::memory {

MemoryImageSource::MemoryImageSource(BinaryReader& reader,
                                     MemoryImageInfo info,
                                     std::uint64_t imagePixelStart)
    : reader_(reader), info_(std::move(info)), imagePixelStart_(imagePixelStart) {}

const io::tile::TileLayout& MemoryImageSource::layout() const noexcept {
    return info_.layout;
}

Result<io::tile::Tile> MemoryImageSource::readTile(const io::tile::TileIndex& index) {
    auto region = info_.layout.regionFor(index);
    if (!region.has_value()) {
        return std::unexpected(region.error());
    }

    const std::uint32_t columns = info_.layout.columns(index.level);
    const std::uint64_t linearIndex =
        static_cast<std::uint64_t>(index.row) * columns + index.column;

    if (info_.tileBytes == 0) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "MemoryImageSource::readTile: zero tile size"});
    }
    if (linearIndex > std::numeric_limits<std::uint64_t>::max() / info_.tileBytes) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument,
                  "MemoryImageSource::readTile: tile offset computation overflows"});
    }
    const std::uint64_t tileOffset = linearIndex * info_.tileBytes;

    if (imagePixelStart_ > std::numeric_limits<std::uint64_t>::max() - tileOffset) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument,
                  "MemoryImageSource::readTile: absolute tile offset overflows"});
    }
    const std::uint64_t absoluteOffset = imagePixelStart_ + tileOffset;

    auto fileSize = reader_.size();
    if (!fileSize.has_value()) {
        return std::unexpected(fileSize.error());
    }
    if (absoluteOffset > *fileSize || info_.tileBytes > *fileSize - absoluteOffset) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument,
                  "MemoryImageSource::readTile: tile byte range exceeds reader size"});
    }

    buffer_.resize(static_cast<std::size_t>(info_.tileBytes));
    auto seekResult = reader_.seek(absoluteOffset);
    if (!seekResult.has_value()) {
        return std::unexpected(seekResult.error());
    }
    auto readResult = reader_.read(std::span<std::byte>{buffer_});
    if (!readResult.has_value()) {
        return std::unexpected(readResult.error());
    }
    if (*readResult != buffer_.size()) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "MemoryImageSource::readTile: truncated tile data"});
    }

    return io::tile::Tile{
        ptiff::TileId{linearIndex}, index, *region, std::span<const std::byte>{buffer_}};
}

} // namespace ptiff::io::backend::memory
