#include <algorithm>
#include <cstddef>
#include <cstdint>
#include <span>
#include <vector>

#include <ptiff/io/backend/tiff/tiff_ifd_writer.hpp>

#include <catch2/catch_test_macros.hpp>

using ptiff::io::backend::tiff::FieldType;
using ptiff::io::backend::tiff::tiffIfdByteSize;
using ptiff::io::backend::tiff::TiffIfdEntryToWrite;
using ptiff::io::backend::tiff::writeTiffIfd;

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

std::uint16_t readU16At(const std::vector<std::byte>& buffer, std::size_t offset) {
    return static_cast<std::uint16_t>(std::to_integer<unsigned>(buffer[offset]) |
                                      (std::to_integer<unsigned>(buffer[offset + 1]) << 8));
}

std::uint32_t readU32At(const std::vector<std::byte>& buffer, std::size_t offset) {
    std::uint32_t value = 0;
    for (int i = 0; i < 4; ++i) {
        value |= std::to_integer<std::uint32_t>(buffer[offset + static_cast<std::size_t>(i)])
                 << (8 * i);
    }
    return value;
}

} // namespace

TEST_CASE("tiffIfdByteSize counts fixed entries with no out-of-line values", "[tiff-ifd-writer]") {
    std::vector<TiffIfdEntryToWrite> entries{
        {.tagId = 256, .fieldType = FieldType::Long, .values = {2}},
        {.tagId = 257, .fieldType = FieldType::Long, .values = {2}},
    };
    // 2 (count) + 2*12 (entries) + 4 (next-IFD offset) = 30, no out-of-line values.
    REQUIRE(tiffIfdByteSize(entries) == 30);
}

TEST_CASE("tiffIfdByteSize adds out-of-line bytes for an entry that overflows the inline area",
          "[tiff-ifd-writer]") {
    std::vector<TiffIfdEntryToWrite> entries{
        {.tagId = 258, .fieldType = FieldType::Short, .values = {8, 8, 8}}, // 6 bytes > 4 inline
    };
    // 2 + 1*12 + 4 = 18 fixed, + 6 out-of-line = 24.
    REQUIRE(tiffIfdByteSize(entries) == 24);
}

TEST_CASE(
    "writeTiffIfd writes entry count, sorts by tag id, and terminates with a 0 next-IFD offset",
    "[tiff-ifd-writer]") {
    BufferBinaryWriter writer;
    std::vector<TiffIfdEntryToWrite> entries{
        {.tagId = 279, .fieldType = FieldType::Long, .values = {4}},
        {.tagId = 256, .fieldType = FieldType::Long, .values = {2}},
    };
    auto result = writeTiffIfd(writer, entries);
    REQUIRE(result.has_value());

    const auto& bytes = writer.buffer();
    REQUIRE(bytes.size() == tiffIfdByteSize(entries));
    REQUIRE(readU16At(bytes, 0) == 2); // entry count

    // First entry after sorting must be tag 256.
    REQUIRE(readU16At(bytes, 2) == 256);
    REQUIRE(readU32At(bytes, 10) == 2); // inline value, LONG fits in the 4-byte area

    // Second entry is tag 279.
    REQUIRE(readU16At(bytes, 14) == 279);
    REQUIRE(readU32At(bytes, 22) == 4);

    // Next-IFD offset (4 bytes right after the 2nd entry) is 0.
    REQUIRE(readU32At(bytes, 26) == 0);
}

TEST_CASE("writeTiffIfd places an out-of-line array right after the fixed IFD structure and "
          "records its offset inline",
          "[tiff-ifd-writer]") {
    BufferBinaryWriter writer;
    std::vector<TiffIfdEntryToWrite> entries{
        {.tagId = 258, .fieldType = FieldType::Short, .values = {8, 8, 8}},
    };
    auto result = writeTiffIfd(writer, entries);
    REQUIRE(result.has_value());

    const auto& bytes = writer.buffer();
    const std::uint64_t fixedSize = 2 + 12 + 4; // count + 1 entry + next-IFD offset = 18
    REQUIRE(bytes.size() == fixedSize + 6);     // + 3 SHORT values out-of-line

    REQUIRE(readU16At(bytes, 2) == 258);
    REQUIRE(readU32At(bytes, 10) == fixedSize); // out-of-line offset, relative to IFD start (0)

    REQUIRE(readU16At(bytes, static_cast<std::size_t>(fixedSize)) == 8);
    REQUIRE(readU16At(bytes, static_cast<std::size_t>(fixedSize) + 2) == 8);
    REQUIRE(readU16At(bytes, static_cast<std::size_t>(fixedSize) + 4) == 8);
}

TEST_CASE("tiffIfdByteSize computes BigTIFF layout size with 20-byte entries",
          "[tiff-ifd-writer]") {
    std::vector<TiffIfdEntryToWrite> entries{
        {.tagId = 256, .fieldType = FieldType::Long, .values = {2}},
        {.tagId = 257, .fieldType = FieldType::Long, .values = {2}},
    };
    // BigTIFF: 8 (count) + 2*20 (entries) + 8 (next-IFD) = 56, no out-of-line values (each entry's
    // single LONG value is 4 bytes, fits the 8-byte inline value area).
    REQUIRE(tiffIfdByteSize(entries, /*isBigTiff=*/true) == 56);
}

TEST_CASE("writeTiffIfd writes a BigTIFF IFD with 20-byte entries and 8-byte counts",
          "[tiff-ifd-writer]") {
    std::vector<TiffIfdEntryToWrite> entries{
        {.tagId = 256, .fieldType = FieldType::Long, .values = {7}},
    };
    BufferBinaryWriter writer;
    auto result = writeTiffIfd(writer, entries, /*isBigTiff=*/true);
    REQUIRE(result.has_value());

    const auto& bytes = writer.buffer();
    // 8 (entry count, LE u64) + 1*20 (entry) + 8 (next-IFD) = 36 bytes total, no out-of-line data.
    REQUIRE(bytes.size() == 36);
    REQUIRE(bytes[0] == std::byte{1}); // entry count = 1 (LE u64)
    for (std::size_t i = 1; i < 8; ++i) {
        REQUIRE(bytes[i] == std::byte{0});
    }
    // Entry record starts at offset 8: tagId(2) + fieldType(2) + count(8) + value(8) = 20 bytes.
    REQUIRE(bytes[8] == std::byte{0});  // tagId low byte (256 = 0x0100)
    REQUIRE(bytes[9] == std::byte{1});  // tagId high byte
    REQUIRE(bytes[10] == std::byte{4}); // fieldType = Long (4), low byte
    REQUIRE(bytes[11] == std::byte{0});
    REQUIRE(bytes[12] == std::byte{1}); // count = 1 (LE u64), low byte
    REQUIRE(bytes[20] == std::byte{7}); // inline value at offset 8+12=20, low byte
}
