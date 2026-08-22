#include <algorithm>
#include <cstddef>
#include <cstdint>
#include <span>
#include <vector>

#include <ptiff/io/backend/tiff/tiff_ifd.hpp>
#include <ptiff/io/backend/tiff/tiff_tag.hpp>
#include <ptiff/io/binary_reader.hpp>

#include <catch2/catch_test_macros.hpp>

namespace {

using ptiff::io::backend::tiff::Endian;
using ptiff::io::backend::tiff::readTiffIfd;
using ptiff::io::backend::tiff::TagId;
using ptiff::io::backend::tiff::TiffIfd;

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

std::uint16_t tag(TagId id) {
    return static_cast<std::uint16_t>(id);
}

} // namespace

TEST_CASE("readTiffIfd resolves an inline SHORT entry", "[tiff-ifd]") {
    // IFD at offset 0: 1 entry (ImageWidth, SHORT, count=1, value=7 inline), next-IFD=0.
    BufferBinaryReader reader(bytes({
        0x01,
        0x00, // entry count = 1
        0x00,
        0x01,
        0x03,
        0x00,
        0x01,
        0x00,
        0x00,
        0x00,
        0x07,
        0x00,
        0x00,
        0x00, // ImageWidth=7
        0x00,
        0x00,
        0x00,
        0x00, // next IFD = 0
    }));

    auto result = readTiffIfd(reader, 0, Endian::Little, false);
    REQUIRE(result.has_value());
    auto value = result->singleValue(tag(TagId::ImageWidth));
    REQUIRE(value.has_value());
    REQUIRE(*value == 7);
}

TEST_CASE("readTiffIfd resolves an inline LONG entry", "[tiff-ifd]") {
    BufferBinaryReader reader(bytes({
        0x01,
        0x00,
        0x11,
        0x01,
        0x04,
        0x00,
        0x01,
        0x00,
        0x00,
        0x00,
        0xC8,
        0x00,
        0x00,
        0x00, // StripOffsets=200
        0x00,
        0x00,
        0x00,
        0x00,
    }));

    auto result = readTiffIfd(reader, 0, Endian::Little, false);
    REQUIRE(result.has_value());
    auto value = result->singleValue(tag(TagId::StripOffsets));
    REQUIRE(value.has_value());
    REQUIRE(*value == 200);
}

TEST_CASE("readTiffIfd resolves an offset-indirected array entry", "[tiff-ifd]") {
    // BitsPerSample, SHORT, count=3 (6 bytes, doesn't fit in the 4-byte value area) -> offset 30.
    // Entry table: offset 0..1 count, 2..13 entry (12 bytes), 14..17 next-IFD.
    // Out-of-line array at offset 30: three SHORTs {8, 8, 8}.
    std::vector<std::byte> data = bytes({
        0x01,
        0x00,
        0x02,
        0x01,
        0x03,
        0x00,
        0x03,
        0x00,
        0x00,
        0x00,
        0x1E,
        0x00,
        0x00,
        0x00, // BitsPerSample
        0x00,
        0x00,
        0x00,
        0x00,
    });
    data.resize(30);
    const auto arrayBytes = bytes({0x08, 0x00, 0x08, 0x00, 0x08, 0x00});
    data.insert(data.end(), arrayBytes.begin(), arrayBytes.end());

    BufferBinaryReader reader(std::move(data));
    auto result = readTiffIfd(reader, 0, Endian::Little, false);
    REQUIRE(result.has_value());
    auto values = result->tag(tag(TagId::BitsPerSample));
    REQUIRE(values.has_value());
    REQUIRE(*values == std::vector<std::uint64_t>{8, 8, 8});
}

TEST_CASE("readTiffIfd resolves a BigTIFF LONG8 entry", "[tiff-ifd]") {
    // BigTIFF entries are 20 bytes: tag(2) type(2) count(8) valueArea(8).
    BufferBinaryReader reader(bytes({
        0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // entry count = 1 (8 bytes, BigTIFF)
        0x11, 0x01, 0x10, 0x00,                         // StripOffsets, LONG8
        0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // count = 1
        0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // value = 512, inline (fits in 8 bytes)
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // next IFD = 0
    }));

    auto result = readTiffIfd(reader, 0, Endian::Little, true);
    REQUIRE(result.has_value());
    auto value = result->singleValue(tag(TagId::StripOffsets));
    REQUIRE(value.has_value());
    REQUIRE(*value == 512);
}

TEST_CASE("readTiffIfd skips an entry with an unsupported field type", "[tiff-ifd]") {
    // Real-world GeoTIFF files carry extension tags (ModelPixelScaleTag, GDAL metadata, ...) in
    // field types this backend never reads (RATIONAL, ASCII, DOUBLE, ...) -- these must not fail
    // the whole parse, only be skipped.
    BufferBinaryReader reader(bytes({
        0x01,
        0x00,
        0x00,
        0x01,
        0x0B,
        0x00,
        0x01,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00, // FLOAT (11) unsupported
        0x00,
        0x00,
        0x00,
        0x00,
    }));

    auto result = readTiffIfd(reader, 0, Endian::Little, false);
    REQUIRE(result.has_value());
    auto value = result->tag(0x0100);
    REQUIRE_FALSE(value.has_value());
    REQUIRE(value.error().code() == ptiff::ErrorCode::NotFound);
}

TEST_CASE("readTiffIfd still resolves later entries after skipping an unsupported one",
          "[tiff-ifd]") {
    // Entry 1: ImageLength, FLOAT (unsupported) -- must be skipped.
    // Entry 2: StripOffsets, LONG, value=200 -- must still resolve correctly.
    BufferBinaryReader reader(bytes({
        0x02,
        0x00, // entry count = 2
        0x01, 0x01, 0x0B, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, // ImageLength, FLOAT (11) unsupported
        0x11, 0x01, 0x04, 0x00, 0x01, 0x00, 0x00, 0x00, 0xC8, 0x00, 0x00,
        0x00, // StripOffsets, LONG, value=200
        0x00, 0x00, 0x00,
        0x00, // next IFD = 0
    }));

    auto result = readTiffIfd(reader, 0, Endian::Little, false);
    REQUIRE(result.has_value());
    auto value = result->singleValue(tag(TagId::StripOffsets));
    REQUIRE(value.has_value());
    REQUIRE(*value == 200);
}

TEST_CASE("readTiffIfd rejects a truncated entry", "[tiff-ifd]") {
    BufferBinaryReader reader(bytes({0x01, 0x00, 0x00, 0x01, 0x03, 0x00}));
    auto result = readTiffIfd(reader, 0, Endian::Little, false);
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("TiffIfd::tag reports NotFound for an absent tag", "[tiff-ifd]") {
    BufferBinaryReader reader(bytes({0x00, 0x00, 0x00, 0x00, 0x00, 0x00}));
    auto result = readTiffIfd(reader, 0, Endian::Little, false);
    REQUIRE(result.has_value());
    auto value = result->tag(tag(TagId::ImageWidth));
    REQUIRE_FALSE(value.has_value());
    REQUIRE(value.error().code() == ptiff::ErrorCode::NotFound);
}

TEST_CASE("TiffIfd::singleValueOr falls back when the tag is absent", "[tiff-ifd]") {
    BufferBinaryReader reader(bytes({0x00, 0x00, 0x00, 0x00, 0x00, 0x00}));
    auto result = readTiffIfd(reader, 0, Endian::Little, false);
    REQUIRE(result.has_value());
    auto value = result->singleValueOr(tag(TagId::PlanarConfiguration), 1);
    REQUIRE(value.has_value());
    REQUIRE(*value == 1);
}

TEST_CASE("readTiffIfd rejects an entry whose count overflows total byte size", "[tiff-ifd]") {
    // BigTIFF entry: StripOffsets, LONG8 (elementSize 8), count = 0xFFFFFFFFFFFFFFFF.
    // 8 * count overflows uint64_t -- must be rejected before it wraps to a small totalBytes.
    BufferBinaryReader reader(bytes({
        0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // entry count = 1 (8 bytes, BigTIFF)
        0x11, 0x01, 0x10, 0x00,                         // StripOffsets, LONG8
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, // count = UINT64_MAX
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // valueArea (irrelevant, never reached)
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // next IFD = 0
    }));

    auto result = readTiffIfd(reader, 0, Endian::Little, true);
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("readTiffIfd rejects an out-of-line value whose size exceeds the file", "[tiff-ifd]") {
    // Classic entry: BitsPerSample, SHORT (elementSize 2), count = 0xFFFFFFFF (no overflow:
    // totalBytes
    // ~= 8.6 GB, far larger than this ~18-byte buffer). Must be rejected via the file-size bound
    // before any allocation of that size is attempted.
    BufferBinaryReader reader(bytes({
        0x01,
        0x00, // entry count = 1
        0x02,
        0x01,
        0x03,
        0x00,
        0xFF,
        0xFF,
        0xFF,
        0xFF,
        0x1E,
        0x00,
        0x00,
        0x00, // BitsPerSample
        0x00,
        0x00,
        0x00,
        0x00, // next IFD = 0
    }));

    auto result = readTiffIfd(reader, 0, Endian::Little, false);
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);
}
