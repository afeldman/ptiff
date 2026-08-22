#include <algorithm>
#include <cstddef>
#include <span>
#include <vector>

#include <ptiff/io/backend/tiff/tiff_header_writer.hpp>

#include <catch2/catch_test_macros.hpp>

using ptiff::io::backend::tiff::kBigTiffHeaderSize;
using ptiff::io::backend::tiff::kClassicTiffHeaderSize;
using ptiff::io::backend::tiff::writeTiffHeader;

namespace {

class BufferBinaryWriter final : public ptiff::io::BinaryWriter {
public:
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

} // namespace

TEST_CASE("writeTiffHeader writes the 8-byte classic little-endian header",
          "[tiff-header-writer]") {
    REQUIRE(kClassicTiffHeaderSize == 8);

    BufferBinaryWriter writer;
    auto result = writeTiffHeader(writer, 8);
    REQUIRE(result.has_value());

    const auto& bytes = writer.buffer();
    REQUIRE(bytes.size() == 8);
    REQUIRE(bytes[0] == std::byte{'I'});
    REQUIRE(bytes[1] == std::byte{'I'});
    REQUIRE(bytes[2] == std::byte{0x2A}); // magic 42, low byte
    REQUIRE(bytes[3] == std::byte{0x00});
    REQUIRE(bytes[4] == std::byte{8}); // firstIfdOffset, low byte
    REQUIRE(bytes[5] == std::byte{0});
    REQUIRE(bytes[6] == std::byte{0});
    REQUIRE(bytes[7] == std::byte{0});
}

TEST_CASE("writeTiffHeader encodes an arbitrary firstIfdOffset", "[tiff-header-writer]") {
    BufferBinaryWriter writer;
    auto result = writeTiffHeader(writer, 0x000001A2);
    REQUIRE(result.has_value());

    const auto& bytes = writer.buffer();
    REQUIRE(bytes[4] == std::byte{0xA2});
    REQUIRE(bytes[5] == std::byte{0x01});
    REQUIRE(bytes[6] == std::byte{0x00});
    REQUIRE(bytes[7] == std::byte{0x00});
}

TEST_CASE("writeTiffHeader writes the 16-byte BigTIFF little-endian header",
          "[tiff-header-writer]") {
    REQUIRE(kBigTiffHeaderSize == 16);

    BufferBinaryWriter writer;
    auto result = writeTiffHeader(writer, 16, /*isBigTiff=*/true);
    REQUIRE(result.has_value());

    const auto& bytes = writer.buffer();
    REQUIRE(bytes.size() == 16);
    REQUIRE(bytes[0] == std::byte{'I'});
    REQUIRE(bytes[1] == std::byte{'I'});
    REQUIRE(bytes[2] == std::byte{0x2B}); // magic 43, low byte
    REQUIRE(bytes[3] == std::byte{0x00});
    REQUIRE(bytes[4] == std::byte{8}); // offsetByteSize = 8, low byte
    REQUIRE(bytes[5] == std::byte{0});
    REQUIRE(bytes[6] == std::byte{0}); // constant = 0
    REQUIRE(bytes[7] == std::byte{0});
    REQUIRE(bytes[8] == std::byte{16}); // firstIfdOffset, low byte
    REQUIRE(bytes[9] == std::byte{0});
    REQUIRE(bytes[10] == std::byte{0});
    REQUIRE(bytes[11] == std::byte{0});
    REQUIRE(bytes[12] == std::byte{0});
    REQUIRE(bytes[13] == std::byte{0});
    REQUIRE(bytes[14] == std::byte{0});
    REQUIRE(bytes[15] == std::byte{0});
}

TEST_CASE("writeTiffHeader BigTIFF variant encodes an arbitrary 64-bit firstIfdOffset",
          "[tiff-header-writer]") {
    BufferBinaryWriter writer;
    auto result = writeTiffHeader(writer, 0x00000001000001A2ULL, /*isBigTiff=*/true);
    REQUIRE(result.has_value());

    const auto& bytes = writer.buffer();
    REQUIRE(bytes[8] == std::byte{0xA2});
    REQUIRE(bytes[9] == std::byte{0x01});
    REQUIRE(bytes[10] == std::byte{0x00});
    REQUIRE(bytes[11] == std::byte{0x00});
    REQUIRE(bytes[12] == std::byte{0x01});
    REQUIRE(bytes[13] == std::byte{0x00});
    REQUIRE(bytes[14] == std::byte{0x00});
    REQUIRE(bytes[15] == std::byte{0x00});
}
