#include <array>
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
#include <ptiff/io/backend/tiff/tiff_endian.hpp>
#include <ptiff/io/backend/tiff/tiff_image_sink.hpp>
#include <ptiff/io/backend/tiff/tiff_pixel_format.hpp>

namespace ptiff::io::backend::tiff {

TiffImageSink::TiffImageSink(BinaryWriter& writer, TiffDirectory directory)
    : writer_(writer), directory_(std::move(directory)) {}

const io::tile::TileLayout& TiffImageSink::layout() const noexcept {
    return directory_.layout;
}

Result<void> TiffImageSink::writeTile(const io::tile::Tile& tile) {
    const std::uint32_t columns = directory_.layout.columns(tile.index().level);
    const std::uint64_t linearIndex =
        static_cast<std::uint64_t>(tile.index().row) * columns + tile.index().column;
    if (linearIndex >= directory_.tileByteRanges.size()) {
        return std::unexpected(Error{ErrorCode::OutOfRange,
                                     "TiffImageSink::writeTile: index outside byte range table"});
    }

    const TileByteRange& range = directory_.tileByteRanges[linearIndex];
    if (tile.data().size() != range.byteCount) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument,
                  "TiffImageSink::writeTile: tile data size doesn't match the strip's expected "
                  "byte count"});
    }

    if (directory_.compression == TiffCompression::None) {
        // Existing uncompressed path: seek to the strip offset and write raw bytes.
        auto seekResult = writer_.seek(range.offset);
        if (!seekResult.has_value()) {
            return std::unexpected(seekResult.error());
        }
        auto writeResult = writer_.write(tile.data());
        if (!writeResult.has_value()) {
            return std::unexpected(writeResult.error());
        }
        if (*writeResult != tile.data().size()) {
            return std::unexpected(
                Error{ErrorCode::InvalidArgument, "TiffImageSink::writeTile: short write"});
        }
        return {};
    }

    // Compressed path. First build the byte stream to encode.
    std::vector<std::byte> payload{tile.data().begin(), tile.data().end()};

    // Apply horizontal differencing first (predictor runs before compression, per TIFF).
    if (directory_.predictor == TiffPredictor::HorizontalDifferencing) {
        const auto rowWidth = directory_.imageWidth;
        const std::uint8_t bps = bytesPerSample(directory_.pixelType);
        const bool bigEndian = directory_.endian == Endian::Big;
        auto dirResult = ptiff::compression::applyHorizontalDifferencing(
            payload, rowWidth, directory_.samplesPerPixel, bps, bigEndian);
        if (!dirResult.has_value()) {
            return std::unexpected(dirResult.error());
        }
    }

    // Compress. TiffCompression::None already returned early above.
    Result<std::vector<std::byte>> encoded;
    switch (directory_.compression) {
    case TiffCompression::None:
    case TiffCompression::Lzw: {
        encoded = ptiff::compression::encodeLzw(payload);
        break;
    }
    case TiffCompression::PackBits: {
        encoded = ptiff::compression::encodePackBits(payload);
        break;
    }
    case TiffCompression::Deflate: {
        encoded = ptiff::compression::encodeDeflate(payload);
        break;
    }
    case TiffCompression::Jpeg: {
        encoded = ptiff::compression::encodeJpeg(payload,
                                                 directory_.layout.tileSize.width,
                                                 directory_.layout.tileSize.height,
                                                 directory_.samplesPerPixel,
                                                 static_cast<int>(directory_.jpegQuality));
        break;
    }
    }
    if (!encoded.has_value()) {
        return std::unexpected(encoded.error());
    }
    if (encoded->size() > std::numeric_limits<std::uint32_t>::max()) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument,
                  "TiffImageSink::writeTile: compressed strip exceeds uint32 size"});
    }

    // Back-patch StripByteCounts with the compressed byte count.
    std::array<std::byte, 4> countBytes{};
    writeU32(countBytes, static_cast<std::uint32_t>(encoded->size()), Endian::Little);
    auto seekPatch = writer_.seek(directory_.stripByteCountsPatchOffset);
    if (!seekPatch.has_value()) {
        return std::unexpected(seekPatch.error());
    }
    auto writePatch = writer_.write(countBytes);
    if (!writePatch.has_value()) {
        return std::unexpected(writePatch.error());
    }

    // Write the compressed strip at the deterministic strip offset.
    auto seekStrip = writer_.seek(range.offset);
    if (!seekStrip.has_value()) {
        return std::unexpected(seekStrip.error());
    }
    auto writeStrip = writer_.write(*encoded);
    if (!writeStrip.has_value()) {
        return std::unexpected(writeStrip.error());
    }
    if (*writeStrip != encoded->size()) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "TiffImageSink::writeTile: short write"});
    }
    return {};
}

} // namespace ptiff::io::backend::tiff
