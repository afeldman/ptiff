#include <algorithm>
#include <array>
#include <cstddef>
#include <cstdint>
#include <span>
#include <vector>

#include <ptiff/compression/packbits.hpp>
#include <ptiff/core/id.hpp>
#include <ptiff/io/backend/tiff/tiff_directory.hpp>
#include <ptiff/io/backend/tiff/tiff_image_sink.hpp>

#include <catch2/catch_test_macros.hpp>

using ptiff::io::backend::tiff::Endian;
using ptiff::io::backend::tiff::TiffCompression;
using ptiff::io::backend::tiff::TiffDirectory;
using ptiff::io::backend::tiff::TiffImageSink;
using ptiff::io::backend::tiff::TiffPredictor;
using ptiff::io::backend::tiff::TileByteRange;

namespace {

class BufferBinaryWriter final : public ptiff::io::BinaryWriter {
public:
    explicit BufferBinaryWriter(std::size_t initialSize) : buffer_(initialSize) {}

    ptiff::Result<std::size_t> write(std::span<const std::byte> source) override {
        if (cursor_ + source.size() > buffer_.size()) {
            buffer_.resize(cursor_ + source.size());
        }
        std::copy(source.begin(), source.end(), buffer_.begin() + static_cast<long>(cursor_));
        cursor_ += source.size();
        return source.size();
    }
    ptiff::Result<void> seek(std::uint64_t offset) override {
        cursor_ = offset;
        return {};
    }
    ptiff::Result<std::uint64_t> position() const override { return cursor_; }
    ptiff::Result<void> flush() override { return {}; }

    [[nodiscard]] const std::vector<std::byte>& buffer() const noexcept { return buffer_; }

private:
    std::vector<std::byte> buffer_;
    std::uint64_t cursor_ = 0;
};

TiffDirectory oneStripDirectory() {
    TiffDirectory directory;
    directory.imageWidth = 2;
    directory.imageHeight = 2;
    directory.pixelType = ptiff::PixelType::UInt8;
    directory.samplesPerPixel = 1;
    directory.layout = ptiff::io::tile::TileLayout{
        .tileSize = {.width = 2, .height = 2}, .imageWidth = 2, .imageHeight = 2, .levelCount = 1};
    directory.tileByteRanges = {TileByteRange{.offset = 100, .byteCount = 4}};
    return directory;
}

} // namespace

TEST_CASE("TiffImageSink::layout returns the directory's layout", "[tiff-image-sink]") {
    BufferBinaryWriter writer(0);
    TiffImageSink sink(writer, oneStripDirectory());
    REQUIRE(sink.layout().imageWidth == 2);
    REQUIRE(sink.layout().imageHeight == 2);
}

TEST_CASE("TiffImageSink::writeTile writes bytes at the strip's recorded offset",
          "[tiff-image-sink]") {
    BufferBinaryWriter writer(0);
    TiffImageSink sink(writer, oneStripDirectory());

    std::array<std::byte, 4> pixels{std::byte{1}, std::byte{2}, std::byte{3}, std::byte{4}};
    ptiff::io::tile::Tile tile{ptiff::TileId{0},
                               ptiff::io::tile::TileIndex{},
                               ptiff::io::tile::TileRegion{.x = 0, .y = 0, .extent = {2, 2}},
                               std::span<const std::byte>{pixels}};

    auto result = sink.writeTile(tile);
    REQUIRE(result.has_value());

    const auto& bytes = writer.buffer();
    REQUIRE(bytes.size() == 104);
    REQUIRE(bytes[100] == std::byte{1});
    REQUIRE(bytes[101] == std::byte{2});
    REQUIRE(bytes[102] == std::byte{3});
    REQUIRE(bytes[103] == std::byte{4});
}

TEST_CASE("TiffImageSink::writeTile rejects a tile whose size doesn't match the strip's byte count",
          "[tiff-image-sink]") {
    BufferBinaryWriter writer(0);
    TiffImageSink sink(writer, oneStripDirectory());

    std::array<std::byte, 2> tooFew{std::byte{1}, std::byte{2}};
    ptiff::io::tile::Tile tile{ptiff::TileId{0},
                               ptiff::io::tile::TileIndex{},
                               ptiff::io::tile::TileRegion{.x = 0, .y = 0, .extent = {2, 2}},
                               std::span<const std::byte>{tooFew}};

    auto result = sink.writeTile(tile);
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("TiffImageSink::writeTile rejects an index outside the byte-range table",
          "[tiff-image-sink]") {
    BufferBinaryWriter writer(0);
    TiffImageSink sink(writer, oneStripDirectory());

    std::array<std::byte, 4> pixels{};
    ptiff::io::tile::Tile tile{
        ptiff::TileId{0},
        ptiff::io::tile::TileIndex{.column = 1, .row = 0, .level = 0}, // out of range: only 1 strip
        ptiff::io::tile::TileRegion{.x = 0, .y = 0, .extent = {2, 2}},
        std::span<const std::byte>{pixels}};

    auto result = sink.writeTile(tile);
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::OutOfRange);
}

TEST_CASE("TiffImageSink writes a PackBits-compressed strip and back-patches byte counts",
          "[tiff-image-sink]") {
    TiffDirectory dir;
    dir.imageWidth = 3;
    dir.imageHeight = 2;
    dir.pixelType = ptiff::PixelType::UInt8;
    dir.samplesPerPixel = 1;
    dir.layout = ptiff::io::tile::TileLayout{
        .tileSize = {.width = 3, .height = 2}, .imageWidth = 3, .imageHeight = 2, .levelCount = 1};
    dir.compression = TiffCompression::PackBits;
    dir.predictor = TiffPredictor::None;
    dir.endian = Endian::Little;
    // Direct-Sink test: pick any consistent, non-overlapping offsets in the writer. The real
    // planTiffWrite derives these deterministically (planTask 4); here we just exercise writeTile.
    constexpr std::uint64_t kPatch = 114;
    constexpr std::uint64_t kData = 122;
    dir.stripByteCountsPatchOffset = kPatch;
    dir.tileByteRanges = {TileByteRange{.offset = kData, .byteCount = 6}};

    BufferBinaryWriter writer(0);
    TiffImageSink sink(writer, dir);

    std::array<std::byte, 6> pixels{
        std::byte{10}, std::byte{20}, std::byte{30}, std::byte{40}, std::byte{50}, std::byte{60}};
    ptiff::io::tile::Tile tile{ptiff::TileId{0},
                               ptiff::io::tile::TileIndex{},
                               ptiff::io::tile::TileRegion{.x = 0, .y = 0, .extent = {3, 2}},
                               std::span<const std::byte>{pixels}};

    auto result = sink.writeTile(tile);
    REQUIRE(result.has_value());

    // Strip data equals encodePackBits(raw), and the u32 at kPatch equals its compressed size.
    auto expected = ptiff::compression::encodePackBits(std::span<const std::byte>{pixels});
    REQUIRE(expected.has_value());
    const auto& bytes = writer.buffer();
    REQUIRE(bytes.size() >= kData + expected->size());
    for (std::size_t i = 0; i < expected->size(); ++i) {
        REQUIRE(bytes[kData + i] == (*expected)[i]);
    }
    const std::uint32_t compressedSize = static_cast<std::uint32_t>(expected->size());
    REQUIRE(std::to_integer<std::uint32_t>(bytes[kPatch]) == (compressedSize & 0xFFU));
    REQUIRE(std::to_integer<std::uint32_t>(bytes[kPatch + 1]) == ((compressedSize >> 8) & 0xFFU));
    REQUIRE(std::to_integer<std::uint32_t>(bytes[kPatch + 2]) == ((compressedSize >> 16) & 0xFFU));
    REQUIRE(std::to_integer<std::uint32_t>(bytes[kPatch + 3]) == ((compressedSize >> 24) & 0xFFU));
}
