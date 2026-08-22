#include <vector>

#include <ptiff/io/backend/tiff/tiff_pixel_format.hpp>

#include <catch2/catch_test_macros.hpp>

using ptiff::io::backend::tiff::bitsPerSampleFor;
using ptiff::io::backend::tiff::bytesPerSample;
using ptiff::io::backend::tiff::pixelTypeFieldValue;
using ptiff::io::backend::tiff::pixelTypeFromFieldValue;
using ptiff::io::backend::tiff::requireUniformBitsPerSample;
using ptiff::io::backend::tiff::resolvePixelType;
using ptiff::io::backend::tiff::sampleFormatFor;

TEST_CASE("resolvePixelType maps unsigned integer formats", "[tiff-pixel-format]") {
    auto uint8 = resolvePixelType(8, 1);
    REQUIRE(uint8.has_value());
    REQUIRE(*uint8 == ptiff::PixelType::UInt8);

    auto uint16 = resolvePixelType(16, 1);
    REQUIRE(uint16.has_value());
    REQUIRE(*uint16 == ptiff::PixelType::UInt16);

    auto uint32 = resolvePixelType(32, 1);
    REQUIRE(uint32.has_value());
    REQUIRE(*uint32 == ptiff::PixelType::UInt32);
}

TEST_CASE("resolvePixelType maps 32-bit float", "[tiff-pixel-format]") {
    auto float32 = resolvePixelType(32, 3);
    REQUIRE(float32.has_value());
    REQUIRE(*float32 == ptiff::PixelType::Float32);
}

TEST_CASE("resolvePixelType rejects float at an unsupported bit depth", "[tiff-pixel-format]") {
    auto result = resolvePixelType(16, 3);
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("resolvePixelType rejects an unsupported BitsPerSample", "[tiff-pixel-format]") {
    auto result = resolvePixelType(12, 1);
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("resolvePixelType rejects an unsupported SampleFormat", "[tiff-pixel-format]") {
    auto result = resolvePixelType(8, 2); // 2 = signed int, unsupported
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("requireUniformBitsPerSample accepts identical values", "[tiff-pixel-format]") {
    auto result = requireUniformBitsPerSample({8, 8, 8});
    REQUIRE(result.has_value());
}

TEST_CASE("requireUniformBitsPerSample rejects non-uniform values", "[tiff-pixel-format]") {
    auto result = requireUniformBitsPerSample({8, 16, 8});
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("requireUniformBitsPerSample rejects an empty list", "[tiff-pixel-format]") {
    auto result = requireUniformBitsPerSample({});
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("pixelTypeFieldValue names every supported PixelType", "[tiff-pixel-format]") {
    REQUIRE(pixelTypeFieldValue(ptiff::PixelType::UInt8) == "UInt8");
    REQUIRE(pixelTypeFieldValue(ptiff::PixelType::UInt16) == "UInt16");
    REQUIRE(pixelTypeFieldValue(ptiff::PixelType::UInt32) == "UInt32");
    REQUIRE(pixelTypeFieldValue(ptiff::PixelType::Float32) == "Float32");
}

TEST_CASE("bytesPerSample maps every PixelType", "[tiff-pixel-format]") {
    REQUIRE(bytesPerSample(ptiff::PixelType::UInt8) == 1);
    REQUIRE(bytesPerSample(ptiff::PixelType::UInt16) == 2);
    REQUIRE(bytesPerSample(ptiff::PixelType::UInt32) == 4);
    REQUIRE(bytesPerSample(ptiff::PixelType::Float32) == 4);
}

TEST_CASE("pixelTypeFromFieldValue is the inverse of pixelTypeFieldValue", "[tiff-pixel-format]") {
    for (auto type : {ptiff::PixelType::UInt8,
                      ptiff::PixelType::UInt16,
                      ptiff::PixelType::UInt32,
                      ptiff::PixelType::Float32}) {
        auto roundTripped = pixelTypeFromFieldValue(pixelTypeFieldValue(type));
        REQUIRE(roundTripped.has_value());
        REQUIRE(*roundTripped == type);
    }
}

TEST_CASE("pixelTypeFromFieldValue rejects an unknown field value", "[tiff-pixel-format]") {
    auto result = pixelTypeFromFieldValue("NotAPixelType");
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("bitsPerSampleFor and sampleFormatFor round-trip through resolvePixelType",
          "[tiff-pixel-format]") {
    for (auto type : {ptiff::PixelType::UInt8,
                      ptiff::PixelType::UInt16,
                      ptiff::PixelType::UInt32,
                      ptiff::PixelType::Float32}) {
        auto resolved = resolvePixelType(bitsPerSampleFor(type), sampleFormatFor(type));
        REQUIRE(resolved.has_value());
        REQUIRE(*resolved == type);
    }
}
