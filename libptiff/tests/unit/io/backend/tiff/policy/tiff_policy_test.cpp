#include <algorithm>
#include <cstddef>
#include <cstdint>
#include <span>
#include <vector>

#include <ptiff/io/backend/tiff/policy/tiff_compression_policy.hpp>
#include <ptiff/io/backend/tiff/policy/tiff_container_policy.hpp>
#include <ptiff/io/backend/tiff/policy/tiff_partition_policy.hpp>
#include <ptiff/io/backend/tiff/policy/tiff_pixel_policy.hpp>
#include <ptiff/io/backend/tiff/policy/tiff_policy_writer.hpp>
#include <ptiff/io/backend/tiff/tiff_directory_writer.hpp>
#include <ptiff/io/backend/tiff/tiff_header_writer.hpp>
#include <ptiff/io/backend/tiff/tiff_ifd_writer.hpp>

#include <catch2/catch_test_macros.hpp>

using namespace ptiff::io::backend::tiff::policy;
namespace tiff = ptiff::io::backend::tiff;

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

/// Serializes the runtime planTiffWrite path (same geometry) into a byte buffer, returning the
/// data offset.
std::pair<std::vector<std::byte>, std::uint64_t> serializeRuntime(uint32_t width, uint32_t height) {
    ptiff::io::StorageModel model;
    model.setField("imageWidth", std::to_string(width));
    model.setField("imageHeight", std::to_string(height));
    model.setField("samplesPerPixel", "1");
    model.setField("pixelType", "UInt8");

    auto plan = tiff::planTiffWrite(model);
    REQUIRE(plan.has_value());

    BufferBinaryWriter writer;
    auto headerOk =
        tiff::writeTiffHeader(writer, /*firstIfdOffset=*/plan->directory.tileByteRanges[0].offset);
    REQUIRE(headerOk.has_value());
    auto ifdOk = tiff::writeTiffIfd(writer, plan->entries, plan->isBigTiff);
    REQUIRE(ifdOk.has_value());
    return {writer.buffer(), plan->directory.tileByteRanges[0].offset};
}

/// Reads the sorted tag-id list from a classic TIFF IFD in `bytes` (8-byte header + 2-byte count
/// + 12-byte records).
std::vector<std::uint16_t> ifdTags(const std::vector<std::byte>& bytes) {
    std::vector<std::uint16_t> tags;
    if (bytes.size() < 10) {
        return tags;
    }
    const std::uint16_t count = static_cast<std::uint16_t>(
        std::to_integer<unsigned>(bytes[8]) | (std::to_integer<unsigned>(bytes[9]) << 8));
    std::size_t pos = 10;
    for (std::uint16_t i = 0; i < count; ++i) {
        tags.push_back(
            static_cast<std::uint16_t>(std::to_integer<unsigned>(bytes[pos]) |
                                       (std::to_integer<unsigned>(bytes[pos + 1]) << 8)));
        pos += 12;
    }
    return tags;
}

} // namespace

// --- Compile-time policy values (static_asserts run at build time; REQUIREs document intent). ---

static_assert(!StripPolicy::isTiled && StripPolicy::tileWidth == 0);
static_assert(TiledPolicy<16, 16>::isTiled && TiledPolicy<16, 16>::tileWidth == 16);
static_assert(TiledPolicy<16, 16>::columns(32) == 2 && TiledPolicy<16, 16>::rows(16) == 1);
static_assert(UInt8Policy::bitsPerSample == 8 && UInt8Policy::sampleFormat == 1);
static_assert(UInt16Policy::bitsPerSample == 16 && UInt32Policy::bitsPerSample == 32);
static_assert(Float32Policy::sampleFormat == 3 && Float32Policy::bytesPerSample == 4);
static_assert(NonePolicy::kTagValue == 1 && PackBitsPolicy::kTagValue == 32773);
static_assert(LzwPolicy::kTagValue == 5);
static_assert(ClassicContainer::isBigTiff == false && ClassicContainer::headerSize == 8);
static_assert(BigTiffContainer::isBigTiff == true && BigTiffContainer::headerSize == 16);

TEST_CASE("PolicyDirectory entryCount reflects strip vs tiled layouts at compile time",
          "[tiff-policy]") {
    using StripDir = PolicyDirectory<UInt8Policy, StripPolicy, NonePolicy, ClassicContainer>;
    using TileDir = PolicyDirectory<UInt8Policy, TiledPolicy<16, 16>, NonePolicy, ClassicContainer>;
    static_assert(StripDir::entryCount == 10);
    static_assert(TileDir::entryCount == 11);
    static_assert(!StripDir::isTiled && TileDir::isTiled);
    REQUIRE(StripDir::entryCount == 10);
    REQUIRE(TileDir::entryCount == 11);
}

TEST_CASE("TiledPolicy geometry helpers give correct tile column/row counts", "[tiff-policy]") {
    using T = TiledPolicy<16, 16>;
    static_assert(T::columns(32) == 2);
    static_assert(T::rows(32) == 2);
    static_assert(T::columns(31) == 2); // edge tile pads to full tile size
    static_assert(T::rows(31) == 2);
    REQUIRE(T::columns(32) == 2);
    REQUIRE(T::rows(31) == 2);
}

TEST_CASE("policy strip-grayscale8 serializer is byte-identical to the runtime planTiffWrite path",
          "[tiff-policy]") {
    constexpr uint32_t width = 32;
    constexpr uint32_t height = 16;

    using Writer = TiffPolicyWriter<UInt8Policy, StripPolicy, NonePolicy, ClassicContainer>;
    BufferBinaryWriter policyWriter;
    auto policyResult = Writer::writeDirectory(policyWriter, width, height);
    REQUIRE(policyResult.has_value());
    const std::uint64_t policyOffset = *policyResult;
    const auto policyBytes = policyWriter.buffer();

    auto [runtimeBytes, runtimeOffset] = serializeRuntime(width, height);

    REQUIRE(policyOffset == runtimeOffset);
    REQUIRE(policyBytes.size() == runtimeBytes.size());
    REQUIRE(policyBytes == runtimeBytes);
}

TEST_CASE("policy writer emits the expected grayscale strip tags in ascending order",
          "[tiff-policy]") {
    using Writer = TiffPolicyWriter<UInt8Policy, StripPolicy, NonePolicy, ClassicContainer>;
    BufferBinaryWriter writer;
    auto result = Writer::writeDirectory(writer, 32, 16);
    REQUIRE(result.has_value());

    const auto tags = ifdTags(writer.buffer());
    const std::vector<std::uint16_t> expected{256, 257, 258, 259, 262, 273, 277, 278, 279, 339};
    REQUIRE(tags == expected);
    REQUIRE(std::is_sorted(tags.begin(), tags.end()));
}

TEST_CASE("tiled policy writer emits TileWidth/TileLength/TileOffsets/TileByteCounts",
          "[tiff-policy]") {
    using Writer = TiffPolicyWriter<UInt8Policy, TiledPolicy<16, 16>, NonePolicy, ClassicContainer>;

    BufferBinaryWriter writer;
    auto result = Writer::writeDirectory(writer, 32, 16); // 2 cols x 1 row = 2 tiles
    REQUIRE(result.has_value());
    const std::uint64_t dataOffset = *result;

    const auto tags = ifdTags(writer.buffer());
    // 7 common + TileWidth/TileLength/TileOffsets/TileByteCounts, in TIFF ascending order; the
    // strip tags (273/278/279) must be absent for a tiled directory.
    const std::vector<std::uint16_t> expected{
        256, 257, 258, 259, 262, 277, 322, 323, 324, 325, 339};
    REQUIRE(tags == expected);
    REQUIRE(std::is_sorted(tags.begin(), tags.end()));
    REQUIRE(std::find(tags.begin(), tags.end(), 273U) == tags.end()); // no StripOffsets
    REQUIRE(std::find(tags.begin(), tags.end(), 278U) == tags.end()); // no RowsPerStrip

    // dataOffset == header (8) + IFD size -> pixel data follows the completed directory.
    REQUIRE(dataOffset == writer.buffer().size());
}
