#include <algorithm>
#include <cstddef>
#include <span>
#include <vector>

#include <ptiff/io/backend/tiff/tiff_directory.hpp>
#include <ptiff/io/backend/tiff/tiff_image_source.hpp>
#include <ptiff/io/binary_reader.hpp>

#include <catch2/catch_test_macros.hpp>

namespace {

using ptiff::io::backend::tiff::TiffDirectory;
using ptiff::io::backend::tiff::TiffImageSource;
using ptiff::io::tile::TileIndex;

class BufferBinaryReader final : public ptiff::io::BinaryReader {
public:
    explicit BufferBinaryReader(std::vector<std::byte> data) : data_(std::move(data)) {}

    ptiff::Result<std::size_t> read(std::span<std::byte> destination) override {
        const std::size_t available = data_.size() - position_;
        const std::size_t toCopy = std::min(destination.size(), available);
        std::copy_n(
            data_.begin() + static_cast<std::ptrdiff_t>(position_), toCopy, destination.begin());
        position_ += toCopy;
        return toCopy;
    }
    ptiff::Result<void> seek(std::uint64_t offset) override {
        if (offset > data_.size()) {
            return std::unexpected(
                ptiff::Error{ptiff::ErrorCode::InvalidArgument, "seek out of range"});
        }
        position_ = offset;
        return {};
    }
    ptiff::Result<std::uint64_t> position() const override { return position_; }
    ptiff::Result<std::uint64_t> size() const override { return data_.size(); }

private:
    std::vector<std::byte> data_;
    std::uint64_t position_ = 0;
};

TiffDirectory strippedDirectory() {
    TiffDirectory directory;
    directory.imageWidth = 4;
    directory.imageHeight = 4;
    directory.pixelType = ptiff::PixelType::UInt8;
    directory.samplesPerPixel = 1;
    directory.layout = ptiff::io::tile::TileLayout{
        .tileSize = {.width = 4, .height = 2}, .imageWidth = 4, .imageHeight = 4, .levelCount = 1};
    directory.tileByteRanges = {{.offset = 100, .byteCount = 8}, {.offset = 200, .byteCount = 8}};
    return directory;
}

// 4x4 image cut into four 2x2 tiles (row-major: tile(0,0), tile(1,0), tile(0,1), tile(1,1)).
TiffDirectory tiledDirectory() {
    TiffDirectory directory;
    directory.imageWidth = 4;
    directory.imageHeight = 4;
    directory.pixelType = ptiff::PixelType::UInt8;
    directory.samplesPerPixel = 1;
    directory.layout = ptiff::io::tile::TileLayout{
        .tileSize = {.width = 2, .height = 2}, .imageWidth = 4, .imageHeight = 4, .levelCount = 1};
    directory.tileByteRanges = {{.offset = 100, .byteCount = 4},
                                {.offset = 150, .byteCount = 4},
                                {.offset = 200, .byteCount = 4},
                                {.offset = 250, .byteCount = 4}};
    return directory;
}

std::vector<std::byte> makeBackingFile() {
    std::vector<std::byte> data(300, std::byte{0});
    for (std::size_t i = 0; i < 8; ++i) {
        data[100 + i] = std::byte{static_cast<unsigned char>(1 + i)};
    }
    for (std::size_t i = 0; i < 8; ++i) {
        data[200 + i] = std::byte{static_cast<unsigned char>(9 + i)};
    }
    // Tiled fixture's 4th tile (offset 250) reuses distinct marker bytes.
    for (std::size_t i = 0; i < 4; ++i) {
        data[150 + i] = std::byte{static_cast<unsigned char>(50 + i)};
        data[250 + i] = std::byte{static_cast<unsigned char>(60 + i)};
    }
    return data;
}

} // namespace

TEST_CASE("TiffImageSource exposes the directory's layout", "[tiff-image-source]") {
    BufferBinaryReader reader(makeBackingFile());
    TiffImageSource source(reader, strippedDirectory());

    REQUIRE(source.layout().imageWidth == 4);
    REQUIRE(source.layout().imageHeight == 4);
}

TEST_CASE("TiffImageSource reads the first strip's bytes", "[tiff-image-source]") {
    BufferBinaryReader reader(makeBackingFile());
    TiffImageSource source(reader, strippedDirectory());

    auto tile = source.readTile(TileIndex{.column = 0, .row = 0, .level = 0});
    REQUIRE(tile.has_value());
    REQUIRE(tile->data().size() == 8);
    for (std::size_t i = 0; i < 8; ++i) {
        REQUIRE(tile->data()[i] == std::byte{static_cast<unsigned char>(1 + i)});
    }
}

TEST_CASE("TiffImageSource reads the second strip's bytes", "[tiff-image-source]") {
    BufferBinaryReader reader(makeBackingFile());
    TiffImageSource source(reader, strippedDirectory());

    auto tile = source.readTile(TileIndex{.column = 0, .row = 1, .level = 0});
    REQUIRE(tile.has_value());
    REQUIRE(tile->data().size() == 8);
    for (std::size_t i = 0; i < 8; ++i) {
        REQUIRE(tile->data()[i] == std::byte{static_cast<unsigned char>(9 + i)});
    }
}

TEST_CASE("TiffImageSource rejects an out-of-range tile index", "[tiff-image-source]") {
    BufferBinaryReader reader(makeBackingFile());
    TiffImageSource source(reader, strippedDirectory());

    auto tile = source.readTile(TileIndex{.column = 0, .row = 5, .level = 0});
    REQUIRE_FALSE(tile.has_value());
    REQUIRE(tile.error().code() == ptiff::ErrorCode::OutOfRange);
}

TEST_CASE("TiffImageSource rejects a tile byte range exceeding the reader's size",
          "[tiff-image-source]") {
    BufferBinaryReader reader(makeBackingFile());
    TiffDirectory directory = strippedDirectory();
    // The backing file is 300 bytes; this byteCount overflows past the end of the buffer.
    directory.tileByteRanges[0] = {.offset = 100, .byteCount = 1'000'000};
    TiffImageSource source(reader, directory);

    auto tile = source.readTile(TileIndex{.column = 0, .row = 0, .level = 0});
    REQUIRE_FALSE(tile.has_value());
    REQUIRE(tile.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("TiffImageSource reads a real-tiled-layout tile, byte-for-byte", "[tiff-image-source]") {
    BufferBinaryReader reader(makeBackingFile());
    TiffImageSource source(reader, tiledDirectory());

    // tile(1,0) -> linearIndex 1 -> byte range {offset=150, byteCount=4} -> markers 50..53.
    auto tile = source.readTile(TileIndex{.column = 1, .row = 0, .level = 0});
    REQUIRE(tile.has_value());
    REQUIRE(tile->data().size() == 4);
    for (std::size_t i = 0; i < 4; ++i) {
        REQUIRE(tile->data()[i] == std::byte{static_cast<unsigned char>(50 + i)});
    }
    REQUIRE(tile->region().x == 2);
    REQUIRE(tile->region().y == 0);

    // tile(1,1) -> linearIndex 3 -> byte range {offset=250, byteCount=4} -> markers 60..63.
    auto secondTile = source.readTile(TileIndex{.column = 1, .row = 1, .level = 0});
    REQUIRE(secondTile.has_value());
    for (std::size_t i = 0; i < 4; ++i) {
        REQUIRE(secondTile->data()[i] == std::byte{static_cast<unsigned char>(60 + i)});
    }
    REQUIRE(secondTile->region().x == 2);
    REQUIRE(secondTile->region().y == 2);
}

TEST_CASE("TiffImageSource decodes a PackBits-compressed tile", "[tiff-image-source]") {
    // Reuses the exact PackBits fixture verified in packbits_test.cpp: control=2 -> literal
    // {0x01,0x02,0x03}; control=-4 (0xFC) -> repeat 0x09 five times. Decodes to 8 bytes, an 8x1
    // grayscale 8-bit tile.
    std::vector<std::byte> compressed = {std::byte{0x02},
                                         std::byte{0x01},
                                         std::byte{0x02},
                                         std::byte{0x03},
                                         std::byte{0xFC},
                                         std::byte{0x09}};
    std::vector<std::byte> backing(300, std::byte{0});
    std::copy(compressed.begin(), compressed.end(), backing.begin() + 100);
    BufferBinaryReader reader(backing);

    TiffDirectory directory;
    directory.imageWidth = 8;
    directory.imageHeight = 1;
    directory.pixelType = ptiff::PixelType::UInt8;
    directory.samplesPerPixel = 1;
    directory.compression = ptiff::io::backend::tiff::TiffCompression::PackBits;
    directory.layout = ptiff::io::tile::TileLayout{
        .tileSize = {.width = 8, .height = 1}, .imageWidth = 8, .imageHeight = 1, .levelCount = 1};
    directory.tileByteRanges = {{.offset = 100, .byteCount = compressed.size()}};

    TiffImageSource source(reader, directory);
    auto tile = source.readTile(TileIndex{.column = 0, .row = 0, .level = 0});
    REQUIRE(tile.has_value());
    REQUIRE(tile->data().size() == 8);
    std::vector<std::byte> expected = {std::byte{0x01},
                                       std::byte{0x02},
                                       std::byte{0x03},
                                       std::byte{0x09},
                                       std::byte{0x09},
                                       std::byte{0x09},
                                       std::byte{0x09},
                                       std::byte{0x09}};
    for (std::size_t i = 0; i < 8; ++i) {
        REQUIRE(tile->data()[i] == expected[i]);
    }
}

TEST_CASE("TiffImageSource decodes an LZW-compressed tile", "[tiff-image-source]") {
    // Reuses the "ABAB" fixture verified in lzw_test.cpp. Decodes to 4 bytes, a 4x1 grayscale
    // 8-bit tile.
    std::vector<std::byte> compressed = {std::byte{0x80},
                                         std::byte{0x10},
                                         std::byte{0x48},
                                         std::byte{0x50},
                                         std::byte{0x28},
                                         std::byte{0x08}};
    std::vector<std::byte> backing(300, std::byte{0});
    std::copy(compressed.begin(), compressed.end(), backing.begin() + 100);
    BufferBinaryReader reader(backing);

    TiffDirectory directory;
    directory.imageWidth = 4;
    directory.imageHeight = 1;
    directory.pixelType = ptiff::PixelType::UInt8;
    directory.samplesPerPixel = 1;
    directory.compression = ptiff::io::backend::tiff::TiffCompression::Lzw;
    directory.layout = ptiff::io::tile::TileLayout{
        .tileSize = {.width = 4, .height = 1}, .imageWidth = 4, .imageHeight = 1, .levelCount = 1};
    directory.tileByteRanges = {{.offset = 100, .byteCount = compressed.size()}};

    TiffImageSource source(reader, directory);
    auto tile = source.readTile(TileIndex{.column = 0, .row = 0, .level = 0});
    REQUIRE(tile.has_value());
    REQUIRE(tile->data().size() == 4);
    REQUIRE(tile->data()[0] == std::byte{0x41});
    REQUIRE(tile->data()[1] == std::byte{0x42});
    REQUIRE(tile->data()[2] == std::byte{0x41});
    REQUIRE(tile->data()[3] == std::byte{0x42});
}

TEST_CASE("TiffImageSource undoes Predictor after LZW decode", "[tiff-image-source]") {
    // Reuses the "AB" fixture verified in lzw_test.cpp: decodes to {0x41,0x42}. Predictor=2 over
    // a 2x1 tile then undoes the differencing: running=0x41=65, then running=(65+0x42)&0xFF=131.
    std::vector<std::byte> compressed = {
        std::byte{0x80}, std::byte{0x10}, std::byte{0x48}, std::byte{0x50}, std::byte{0x10}};
    std::vector<std::byte> backing(300, std::byte{0});
    std::copy(compressed.begin(), compressed.end(), backing.begin() + 100);
    BufferBinaryReader reader(backing);

    TiffDirectory directory;
    directory.imageWidth = 2;
    directory.imageHeight = 1;
    directory.pixelType = ptiff::PixelType::UInt8;
    directory.samplesPerPixel = 1;
    directory.compression = ptiff::io::backend::tiff::TiffCompression::Lzw;
    directory.predictor = ptiff::io::backend::tiff::TiffPredictor::HorizontalDifferencing;
    directory.layout = ptiff::io::tile::TileLayout{
        .tileSize = {.width = 2, .height = 1}, .imageWidth = 2, .imageHeight = 1, .levelCount = 1};
    directory.tileByteRanges = {{.offset = 100, .byteCount = compressed.size()}};

    TiffImageSource source(reader, directory);
    auto tile = source.readTile(TileIndex{.column = 0, .row = 0, .level = 0});
    REQUIRE(tile.has_value());
    REQUIRE(tile->data().size() == 2);
    REQUIRE(tile->data()[0] == std::byte{0x41});
    REQUIRE(tile->data()[1] == std::byte{0x83}); // 131 decimal
}

TEST_CASE("TiffImageSource rejects an LZW tile that would decode past the expected tile size",
          "[tiff-image-source]") {
    // Same "AB"-decoding stream, but the tile is declared 1x1 (expectedSize=1) -- decoding 'A'
    // hits the bound exactly, decoding 'B' would exceed it. Decompression-bomb guard.
    std::vector<std::byte> compressed = {
        std::byte{0x80}, std::byte{0x10}, std::byte{0x48}, std::byte{0x50}, std::byte{0x10}};
    std::vector<std::byte> backing(300, std::byte{0});
    std::copy(compressed.begin(), compressed.end(), backing.begin() + 100);
    BufferBinaryReader reader(backing);

    TiffDirectory directory;
    directory.imageWidth = 1;
    directory.imageHeight = 1;
    directory.pixelType = ptiff::PixelType::UInt8;
    directory.samplesPerPixel = 1;
    directory.compression = ptiff::io::backend::tiff::TiffCompression::Lzw;
    directory.layout = ptiff::io::tile::TileLayout{
        .tileSize = {.width = 1, .height = 1}, .imageWidth = 1, .imageHeight = 1, .levelCount = 1};
    directory.tileByteRanges = {{.offset = 100, .byteCount = compressed.size()}};

    TiffImageSource source(reader, directory);
    auto tile = source.readTile(TileIndex{.column = 0, .row = 0, .level = 0});
    REQUIRE_FALSE(tile.has_value());
    REQUIRE(tile.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("TiffImageSource rejects tile dimensions whose expected size overflows or exceeds vector "
          "limits",
          "[tiff-image-source]") {
    // A maliciously crafted TIFF can declare implausible tile dimensions. The expected-size
    // computation must be rejected before any allocation/decode is attempted, rather than
    // throwing std::length_error/std::bad_alloc out of a Result<T>-returning function.
    //
    // width/height are kept just under 2^31 so TileLayout::columns()/rows() (uint32_t arithmetic,
    // a pre-existing and out-of-scope area of the code) still resolve this as a single in-range
    // tile; samplesPerPixel * bytesPerSample(Float64) then pushes the size computation itself
    // past what a std::size_t multiplication can represent.
    constexpr std::uint32_t hugeDimension = 2'000'000'000U;
    std::vector<std::byte> backing(300, std::byte{0});
    BufferBinaryReader reader(backing);

    TiffDirectory directory;
    directory.imageWidth = hugeDimension;
    directory.imageHeight = hugeDimension;
    directory.pixelType = ptiff::PixelType::Float64;
    directory.samplesPerPixel = 1;
    directory.compression = ptiff::io::backend::tiff::TiffCompression::PackBits;
    directory.layout =
        ptiff::io::tile::TileLayout{.tileSize = {.width = hugeDimension, .height = hugeDimension},
                                    .imageWidth = hugeDimension,
                                    .imageHeight = hugeDimension,
                                    .levelCount = 1};
    directory.tileByteRanges = {{.offset = 100, .byteCount = 8}};

    TiffImageSource source(reader, directory);
    auto tile = source.readTile(TileIndex{.column = 0, .row = 0, .level = 0});
    REQUIRE_FALSE(tile.has_value());
    REQUIRE(tile.error().code() == ptiff::ErrorCode::InvalidArgument);
}
