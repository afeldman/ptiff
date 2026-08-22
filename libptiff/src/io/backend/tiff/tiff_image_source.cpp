#include <cstdint>
#include <limits>
#include <span>
#include <utility>
#include <vector>

#include <ptiff/compression/deflate.hpp>
#include <ptiff/compression/jpeg.hpp>
#include <ptiff/compression/lzw.hpp>
#include <ptiff/compression/packbits.hpp>
#include <ptiff/compression/predictor.hpp>
#include <ptiff/core/id.hpp>
#include <ptiff/io/backend/tiff/tiff_image_source.hpp>
#include <ptiff/io/backend/tiff/tiff_pixel_format.hpp>

namespace ptiff::io::backend::tiff {

namespace {

/// Multiplies `a` and `b`, rejecting the result rather than silently overflowing.
Result<std::size_t> checkedMultiply(std::size_t a, std::size_t b) {
    if (a != 0 && b > std::numeric_limits<std::size_t>::max() / a) {
        return std::unexpected(Error{ErrorCode::InvalidArgument,
                                     "TiffImageSource::readTile: tile size computation overflows"});
    }
    return a * b;
}

} // namespace

TiffImageSource::TiffImageSource(BinaryReader& reader, TiffDirectory directory)
    : reader_(reader), directory_(std::move(directory)) {}

const io::tile::TileLayout& TiffImageSource::layout() const noexcept {
    return directory_.layout;
}

Result<io::tile::Tile> TiffImageSource::readTile(const io::tile::TileIndex& index) {
    auto region = directory_.layout.regionFor(index);
    if (!region.has_value()) {
        return std::unexpected(region.error());
    }

    const std::uint32_t columns = directory_.layout.columns(index.level);
    const std::uint64_t linearIndex =
        static_cast<std::uint64_t>(index.row) * columns + index.column;
    if (linearIndex >= directory_.tileByteRanges.size()) {
        return std::unexpected(Error{ErrorCode::OutOfRange,
                                     "TiffImageSource::readTile: index outside byte range table"});
    }

    const TileByteRange& range = directory_.tileByteRanges[linearIndex];

    auto fileSize = reader_.size();
    if (!fileSize.has_value()) {
        return std::unexpected(fileSize.error());
    }
    if (range.offset > *fileSize || range.byteCount > *fileSize - range.offset) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument,
                  "TiffImageSource::readTile: tile byte range exceeds underlying reader size"});
    }

    rawBuffer_.resize(range.byteCount);
    auto seekResult = reader_.seek(range.offset);
    if (!seekResult.has_value()) {
        return std::unexpected(seekResult.error());
    }
    auto readResult = reader_.read(std::span<std::byte>{rawBuffer_});
    if (!readResult.has_value()) {
        return std::unexpected(readResult.error());
    }
    if (*readResult != rawBuffer_.size()) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "TiffImageSource::readTile: truncated tile data"});
    }

    auto expectedSizeStep1 =
        checkedMultiply(static_cast<std::size_t>(directory_.layout.tileSize.width),
                        static_cast<std::size_t>(directory_.layout.tileSize.height));
    if (!expectedSizeStep1.has_value()) {
        return std::unexpected(expectedSizeStep1.error());
    }
    auto expectedSizeStep2 =
        checkedMultiply(*expectedSizeStep1, static_cast<std::size_t>(directory_.samplesPerPixel));
    if (!expectedSizeStep2.has_value()) {
        return std::unexpected(expectedSizeStep2.error());
    }
    auto expectedSizeResult = checkedMultiply(
        *expectedSizeStep2, static_cast<std::size_t>(bytesPerSample(directory_.pixelType)));
    if (!expectedSizeResult.has_value()) {
        return std::unexpected(expectedSizeResult.error());
    }
    if (*expectedSizeResult > std::vector<std::byte>{}.max_size()) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument,
                  "TiffImageSource::readTile: expected tile size exceeds maximum representable "
                  "size"});
    }
    const std::size_t expectedSize = *expectedSizeResult;

    switch (directory_.compression) {
    case TiffCompression::None: {
        if (rawBuffer_.size() != expectedSize) {
            return std::unexpected(Error{ErrorCode::InvalidArgument,
                                         "TiffImageSource::readTile: uncompressed tile size does "
                                         "not match the expected size"});
        }
        buffer_ = rawBuffer_;
        break;
    }
    case TiffCompression::Lzw: {
        auto decoded = ptiff::compression::decodeLzw(rawBuffer_, expectedSize);
        if (!decoded.has_value()) {
            return std::unexpected(decoded.error());
        }
        buffer_ = std::move(*decoded);
        break;
    }
    case TiffCompression::PackBits: {
        auto decoded = ptiff::compression::decodePackBits(rawBuffer_, expectedSize);
        if (!decoded.has_value()) {
            return std::unexpected(decoded.error());
        }
        buffer_ = std::move(*decoded);
        break;
    }
    case TiffCompression::Deflate: {
        auto decoded = ptiff::compression::decodeDeflate(rawBuffer_, expectedSize);
        if (!decoded.has_value()) {
            return std::unexpected(decoded.error());
        }
        buffer_ = std::move(*decoded);
        break;
    }
    case TiffCompression::Jpeg: {
        auto decoded = ptiff::compression::decodeJpeg(rawBuffer_,
                                                      directory_.layout.tileSize.width,
                                                      directory_.layout.tileSize.height,
                                                      directory_.samplesPerPixel);
        if (!decoded.has_value()) {
            return std::unexpected(decoded.error());
        }
        buffer_ = std::move(*decoded);
        break;
    }
    }

    if (directory_.predictor == TiffPredictor::HorizontalDifferencing) {
        auto undone =
            ptiff::compression::undoHorizontalDifferencing(std::span<std::byte>{buffer_},
                                                           directory_.layout.tileSize.width,
                                                           directory_.samplesPerPixel,
                                                           bytesPerSample(directory_.pixelType),
                                                           directory_.endian == Endian::Big);
        if (!undone.has_value()) {
            return std::unexpected(undone.error());
        }
    }

    return io::tile::Tile{
        ptiff::TileId{linearIndex}, index, *region, std::span<const std::byte>{buffer_}};
}

} // namespace ptiff::io::backend::tiff
