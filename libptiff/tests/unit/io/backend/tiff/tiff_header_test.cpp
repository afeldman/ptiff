#include <algorithm>
#include <cstddef>
#include <cstdint>
#include <span>
#include <vector>

#include <ptiff/io/backend/tiff/tiff_header.hpp>
#include <ptiff/io/binary_reader.hpp>

#include <catch2/catch_test_macros.hpp>

namespace {

using ptiff::io::backend::tiff::Endian;
using ptiff::io::backend::tiff::readTiffHeader;
using ptiff::io::backend::tiff::TiffHeader;

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

std::vector<std::byte> bytes(std::initializer_list<unsigned char> values) {
    std::vector<std::byte> result;
    result.reserve(values.size());
    for (auto v : values) {
        result.push_back(std::byte{v});
    }
    return result;
}

} // namespace

TEST_CASE("readTiffHeader parses classic little-endian header", "[tiff-header]") {
    BufferBinaryReader reader(bytes({'I', 'I', 0x2A, 0x00, 0x08, 0x00, 0x00, 0x00}));
    auto result = readTiffHeader(reader);
    REQUIRE(result.has_value());
    REQUIRE(result->endian == Endian::Little);
    REQUIRE_FALSE(result->isBigTiff);
    REQUIRE(result->firstIfdOffset == 8);
}

TEST_CASE("readTiffHeader parses classic big-endian header", "[tiff-header]") {
    BufferBinaryReader reader(bytes({'M', 'M', 0x00, 0x2A, 0x00, 0x00, 0x00, 0x10}));
    auto result = readTiffHeader(reader);
    REQUIRE(result.has_value());
    REQUIRE(result->endian == Endian::Big);
    REQUIRE_FALSE(result->isBigTiff);
    REQUIRE(result->firstIfdOffset == 16);
}

TEST_CASE("readTiffHeader parses BigTIFF little-endian header", "[tiff-header]") {
    BufferBinaryReader reader(bytes({'I',
                                     'I',
                                     0x2B,
                                     0x00,
                                     0x08,
                                     0x00,
                                     0x00,
                                     0x00,
                                     0x10,
                                     0x00,
                                     0x00,
                                     0x00,
                                     0x00,
                                     0x00,
                                     0x00,
                                     0x00}));
    auto result = readTiffHeader(reader);
    REQUIRE(result.has_value());
    REQUIRE(result->endian == Endian::Little);
    REQUIRE(result->isBigTiff);
    REQUIRE(result->firstIfdOffset == 16);
}

TEST_CASE("readTiffHeader parses BigTIFF big-endian header", "[tiff-header]") {
    BufferBinaryReader reader(bytes({'M',
                                     'M',
                                     0x00,
                                     0x2B,
                                     0x00,
                                     0x08,
                                     0x00,
                                     0x00,
                                     0x00,
                                     0x00,
                                     0x00,
                                     0x00,
                                     0x00,
                                     0x00,
                                     0x00,
                                     0x20}));
    auto result = readTiffHeader(reader);
    REQUIRE(result.has_value());
    REQUIRE(result->endian == Endian::Big);
    REQUIRE(result->isBigTiff);
    REQUIRE(result->firstIfdOffset == 32);
}

TEST_CASE("readTiffHeader rejects an unrecognized byte-order mark", "[tiff-header]") {
    BufferBinaryReader reader(bytes({'X', 'X', 0x2A, 0x00, 0x08, 0x00, 0x00, 0x00}));
    auto result = readTiffHeader(reader);
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("readTiffHeader rejects an unrecognized magic number", "[tiff-header]") {
    BufferBinaryReader reader(bytes({'I', 'I', 0x99, 0x00, 0x08, 0x00, 0x00, 0x00}));
    auto result = readTiffHeader(reader);
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("readTiffHeader rejects a truncated file", "[tiff-header]") {
    BufferBinaryReader reader(bytes({'I', 'I', 0x2A}));
    auto result = readTiffHeader(reader);
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("readTiffHeader rejects a malformed BigTIFF offset-byte-size field", "[tiff-header]") {
    BufferBinaryReader reader(bytes({'I',
                                     'I',
                                     0x2B,
                                     0x00,
                                     0x04,
                                     0x00,
                                     0x00,
                                     0x00,
                                     0x10,
                                     0x00,
                                     0x00,
                                     0x00,
                                     0x00,
                                     0x00,
                                     0x00,
                                     0x00}));
    auto result = readTiffHeader(reader);
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);
}
