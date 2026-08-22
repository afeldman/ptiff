#include <vector>

#include <ptiff/io/backend/tiff/tiff_directory.hpp>
#include <ptiff/io/backend/tiff/tiff_ifd.hpp>
#include <ptiff/io/backend/tiff/tiff_tag.hpp>

#include <catch2/catch_test_macros.hpp>

namespace {

using ptiff::io::backend::tiff::interpretTiffIfd;
using ptiff::io::backend::tiff::TagId;
using ptiff::io::backend::tiff::TiffIfd;
using ptiff::io::backend::tiff::toStorageModel;

std::uint16_t tag(TagId id) {
    return static_cast<std::uint16_t>(id);
}

TiffIfd minimalStrippedIfd() {
    TiffIfd ifd;
    ifd.setTag(tag(TagId::ImageWidth), {4});
    ifd.setTag(tag(TagId::ImageLength), {4});
    ifd.setTag(tag(TagId::BitsPerSample), {8});
    ifd.setTag(tag(TagId::Compression), {1});
    ifd.setTag(tag(TagId::PhotometricInterpretation), {1});
    ifd.setTag(tag(TagId::StripOffsets), {100, 200});
    ifd.setTag(tag(TagId::RowsPerStrip), {2});
    ifd.setTag(tag(TagId::StripByteCounts), {8, 8});
    return ifd;
}

} // namespace

TEST_CASE("interpretTiffIfd builds a stripped-layout directory", "[tiff-directory]") {
    auto result = interpretTiffIfd(minimalStrippedIfd());
    REQUIRE(result.has_value());
    REQUIRE(result->imageWidth == 4);
    REQUIRE(result->imageHeight == 4);
    REQUIRE(result->samplesPerPixel == 1);
    REQUIRE(result->pixelType == ptiff::PixelType::UInt8);
    REQUIRE(result->layout.tileSize.width == 4);
    REQUIRE(result->layout.tileSize.height == 2);
    REQUIRE(result->tileByteRanges.size() == 2);
    REQUIRE(result->tileByteRanges[0].offset == 100);
    REQUIRE(result->tileByteRanges[0].byteCount == 8);
    REQUIRE(result->tileByteRanges[1].offset == 200);
}

TEST_CASE("interpretTiffIfd builds a tiled-layout directory", "[tiff-directory]") {
    TiffIfd ifd;
    ifd.setTag(tag(TagId::ImageWidth), {4});
    ifd.setTag(tag(TagId::ImageLength), {4});
    ifd.setTag(tag(TagId::BitsPerSample), {8});
    ifd.setTag(tag(TagId::Compression), {1});
    ifd.setTag(tag(TagId::PhotometricInterpretation), {1});
    ifd.setTag(tag(TagId::TileWidth), {2});
    ifd.setTag(tag(TagId::TileLength), {2});
    ifd.setTag(tag(TagId::TileOffsets), {100, 108, 116, 124});
    ifd.setTag(tag(TagId::TileByteCounts), {4, 4, 4, 4});

    auto result = interpretTiffIfd(ifd);
    REQUIRE(result.has_value());
    REQUIRE(result->layout.tileSize.width == 2);
    REQUIRE(result->layout.tileSize.height == 2);
    REQUIRE(result->tileByteRanges.size() == 4);
}

TEST_CASE("interpretTiffIfd requires a required tag", "[tiff-directory]") {
    TiffIfd ifd = minimalStrippedIfd();
    // Rebuild without ImageWidth.
    TiffIfd missing;
    missing.setTag(tag(TagId::ImageLength), {4});
    missing.setTag(tag(TagId::BitsPerSample), {8});
    missing.setTag(tag(TagId::PhotometricInterpretation), {1});
    missing.setTag(tag(TagId::StripOffsets), {100});
    missing.setTag(tag(TagId::RowsPerStrip), {4});
    missing.setTag(tag(TagId::StripByteCounts), {16});

    auto result = interpretTiffIfd(missing);
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);
    (void)ifd;
}

TEST_CASE("interpretTiffIfd rejects unsupported Compression", "[tiff-directory]") {
    TiffIfd ifd = minimalStrippedIfd();
    ifd.setTag(tag(TagId::Compression), {6}); // old-style JPEG, unsupported
    auto result = interpretTiffIfd(ifd);
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("interpretTiffIfd rejects unsupported PlanarConfiguration", "[tiff-directory]") {
    TiffIfd ifd = minimalStrippedIfd();
    ifd.setTag(tag(TagId::PlanarConfiguration), {2}); // Planar, unsupported
    auto result = interpretTiffIfd(ifd);
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("interpretTiffIfd rejects an image that is neither stripped nor tiled",
          "[tiff-directory]") {
    TiffIfd ifd;
    ifd.setTag(tag(TagId::ImageWidth), {4});
    ifd.setTag(tag(TagId::ImageLength), {4});
    ifd.setTag(tag(TagId::BitsPerSample), {8});
    ifd.setTag(tag(TagId::PhotometricInterpretation), {1});

    auto result = interpretTiffIfd(ifd);
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("interpretTiffIfd rejects an image that is both stripped and tiled", "[tiff-directory]") {
    TiffIfd ifd = minimalStrippedIfd();
    ifd.setTag(tag(TagId::TileWidth), {2});
    ifd.setTag(tag(TagId::TileLength), {2});
    ifd.setTag(tag(TagId::TileOffsets), {300});
    ifd.setTag(tag(TagId::TileByteCounts), {16});

    auto result = interpretTiffIfd(ifd);
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("interpretTiffIfd rejects mismatched StripOffsets/StripByteCounts lengths",
          "[tiff-directory]") {
    TiffIfd ifd = minimalStrippedIfd();
    // StripOffsets has 2 entries but StripByteCounts is shortened to 1: malformed input that
    // must be rejected rather than silently truncated.
    ifd.setTag(tag(TagId::StripByteCounts), {8});

    auto result = interpretTiffIfd(ifd);
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("toStorageModel encodes directory fields as strings", "[tiff-directory]") {
    auto directory = interpretTiffIfd(minimalStrippedIfd());
    REQUIRE(directory.has_value());

    auto model = toStorageModel(*directory);
    REQUIRE(model.field("imageWidth").value() == "4");
    REQUIRE(model.field("imageHeight").value() == "4");
    REQUIRE(model.field("samplesPerPixel").value() == "1");
    REQUIRE(model.field("pixelType").value() == "UInt8");
    REQUIRE(model.field("compression").value() == "None");
    REQUIRE(model.field("predictor").value() == "None");
}

TEST_CASE("toStorageModel encodes LZW compression and Predictor=2", "[tiff-directory]") {
    TiffIfd ifd = minimalStrippedIfd();
    ifd.setTag(tag(TagId::Compression), {5});
    ifd.setTag(tag(TagId::Predictor), {2});
    auto directory = interpretTiffIfd(ifd);
    REQUIRE(directory.has_value());

    auto model = toStorageModel(*directory);
    REQUIRE(model.field("compression").value() == "Lzw");
    REQUIRE(model.field("predictor").value() == "HorizontalDifferencing");
}

TEST_CASE("toStorageModel encodes PackBits compression", "[tiff-directory]") {
    TiffIfd ifd = minimalStrippedIfd();
    ifd.setTag(tag(TagId::Compression), {32773});
    auto directory = interpretTiffIfd(ifd);
    REQUIRE(directory.has_value());

    auto model = toStorageModel(*directory);
    REQUIRE(model.field("compression").value() == "PackBits");
}

TEST_CASE("toStorageModel encodes Deflate compression", "[tiff-directory]") {
    TiffIfd ifd = minimalStrippedIfd();
    ifd.setTag(tag(TagId::Compression), {8});
    auto directory = interpretTiffIfd(ifd);
    REQUIRE(directory.has_value());

    auto model = toStorageModel(*directory);
    REQUIRE(model.field("compression").value() == "Deflate");
}

TEST_CASE("toStorageModel encodes Jpeg compression", "[tiff-directory]") {
    TiffIfd ifd = minimalStrippedIfd();
    ifd.setTag(tag(TagId::Compression), {7});
    auto directory = interpretTiffIfd(ifd);
    REQUIRE(directory.has_value());

    auto model = toStorageModel(*directory);
    REQUIRE(model.field("compression").value() == "Jpeg");
    REQUIRE(model.field("jpegQuality").value() == "90");
}

TEST_CASE("interpretTiffIfd accepts LZW compression", "[tiff-directory]") {
    TiffIfd ifd = minimalStrippedIfd();
    ifd.setTag(tag(TagId::Compression), {5});
    auto result = interpretTiffIfd(ifd);
    REQUIRE(result.has_value());
    REQUIRE(result->compression == ptiff::io::backend::tiff::TiffCompression::Lzw);
}

TEST_CASE("interpretTiffIfd accepts PackBits compression", "[tiff-directory]") {
    TiffIfd ifd = minimalStrippedIfd();
    ifd.setTag(tag(TagId::Compression), {32773});
    auto result = interpretTiffIfd(ifd);
    REQUIRE(result.has_value());
    REQUIRE(result->compression == ptiff::io::backend::tiff::TiffCompression::PackBits);
}

TEST_CASE("interpretTiffIfd accepts Deflate compression (tag value 8)", "[tiff-directory]") {
    TiffIfd ifd = minimalStrippedIfd();
    ifd.setTag(tag(TagId::Compression), {8});
    auto result = interpretTiffIfd(ifd);
    REQUIRE(result.has_value());
    REQUIRE(result->compression == ptiff::io::backend::tiff::TiffCompression::Deflate);
}

TEST_CASE("interpretTiffIfd accepts the legacy Deflate compression tag value (32946)",
          "[tiff-directory]") {
    TiffIfd ifd = minimalStrippedIfd();
    ifd.setTag(tag(TagId::Compression), {32946});
    auto result = interpretTiffIfd(ifd);
    REQUIRE(result.has_value());
    REQUIRE(result->compression == ptiff::io::backend::tiff::TiffCompression::Deflate);
}

TEST_CASE("interpretTiffIfd accepts Jpeg compression (tag value 7)", "[tiff-directory]") {
    TiffIfd ifd = minimalStrippedIfd();
    ifd.setTag(tag(TagId::Compression), {7});
    auto result = interpretTiffIfd(ifd);
    REQUIRE(result.has_value());
    REQUIRE(result->compression == ptiff::io::backend::tiff::TiffCompression::Jpeg);
}

TEST_CASE("interpretTiffIfd accepts Jpeg RGB with YCbCr PhotometricInterpretation",
          "[tiff-directory]") {
    TiffIfd ifd = minimalStrippedIfd();
    ifd.setTag(tag(TagId::Compression), {7});
    ifd.setTag(tag(TagId::SamplesPerPixel), {3});
    ifd.setTag(tag(TagId::PhotometricInterpretation), {6});
    auto result = interpretTiffIfd(ifd);
    REQUIRE(result.has_value());
    REQUIRE(result->compression == ptiff::io::backend::tiff::TiffCompression::Jpeg);
    REQUIRE(result->samplesPerPixel == 3);
}

TEST_CASE("interpretTiffIfd rejects Jpeg compression with a non-UInt8 pixelType",
          "[tiff-directory]") {
    TiffIfd ifd = minimalStrippedIfd();
    ifd.setTag(tag(TagId::Compression), {7});
    ifd.setTag(tag(TagId::BitsPerSample), {16});
    auto result = interpretTiffIfd(ifd);
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("interpretTiffIfd rejects PhotometricInterpretation=YCbCr without Jpeg compression",
          "[tiff-directory]") {
    TiffIfd ifd = minimalStrippedIfd();
    ifd.setTag(tag(TagId::SamplesPerPixel), {3});
    ifd.setTag(tag(TagId::PhotometricInterpretation), {6});
    // Compression left at 1 (None) from minimalStrippedIfd.
    auto result = interpretTiffIfd(ifd);
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("interpretTiffIfd rejects Jpeg grayscale with RGB PhotometricInterpretation",
          "[tiff-directory]") {
    TiffIfd ifd = minimalStrippedIfd();
    ifd.setTag(tag(TagId::Compression), {7});
    ifd.setTag(tag(TagId::PhotometricInterpretation), {2}); // RGB, not WhiteIsZero/BlackIsZero
    auto result = interpretTiffIfd(ifd);
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("interpretTiffIfd rejects Jpeg RGB with BlackIsZero PhotometricInterpretation",
          "[tiff-directory]") {
    TiffIfd ifd = minimalStrippedIfd();
    ifd.setTag(tag(TagId::Compression), {7});
    ifd.setTag(tag(TagId::SamplesPerPixel), {3});
    ifd.setTag(tag(TagId::PhotometricInterpretation), {1}); // BlackIsZero, not YCbCr
    auto result = interpretTiffIfd(ifd);
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("interpretTiffIfd rejects Predictor=2 combined with Jpeg compression",
          "[tiff-directory]") {
    TiffIfd ifd = minimalStrippedIfd();
    ifd.setTag(tag(TagId::Compression), {7});
    ifd.setTag(tag(TagId::Predictor), {2});
    auto result = interpretTiffIfd(ifd);
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("interpretTiffIfd defaults to no predictor", "[tiff-directory]") {
    auto result = interpretTiffIfd(minimalStrippedIfd());
    REQUIRE(result.has_value());
    REQUIRE(result->predictor == ptiff::io::backend::tiff::TiffPredictor::None);
}

TEST_CASE("interpretTiffIfd accepts Predictor=2 horizontal differencing", "[tiff-directory]") {
    TiffIfd ifd = minimalStrippedIfd();
    ifd.setTag(tag(TagId::Compression), {5});
    ifd.setTag(tag(TagId::Predictor), {2});
    auto result = interpretTiffIfd(ifd);
    REQUIRE(result.has_value());
    REQUIRE(result->predictor == ptiff::io::backend::tiff::TiffPredictor::HorizontalDifferencing);
}

TEST_CASE("interpretTiffIfd rejects unsupported Predictor value", "[tiff-directory]") {
    TiffIfd ifd = minimalStrippedIfd();
    ifd.setTag(tag(TagId::Compression), {5});
    ifd.setTag(tag(TagId::Predictor), {3});
    auto result = interpretTiffIfd(ifd);
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("interpretTiffIfd rejects Predictor=2 combined with float32 SampleFormat",
          "[tiff-directory]") {
    TiffIfd ifd = minimalStrippedIfd();
    ifd.setTag(tag(TagId::Compression), {5});
    ifd.setTag(tag(TagId::BitsPerSample), {32});
    ifd.setTag(tag(TagId::SampleFormat), {3}); // float
    ifd.setTag(tag(TagId::Predictor), {2});
    auto result = interpretTiffIfd(ifd);
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);
}
