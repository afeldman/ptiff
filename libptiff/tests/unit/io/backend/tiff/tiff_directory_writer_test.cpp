#include <algorithm>

#include <ptiff/io/backend/tiff/tiff_directory_writer.hpp>
#include <ptiff/io/backend/tiff/tiff_header_writer.hpp>
#include <ptiff/io/backend/tiff/tiff_ifd_writer.hpp>
#include <ptiff/io/storage_model.hpp>

#include <catch2/catch_test_macros.hpp>

using ptiff::io::StorageModel;
using ptiff::io::backend::tiff::kClassicTiffHeaderSize;
using ptiff::io::backend::tiff::planTiffWrite;
using ptiff::io::backend::tiff::TiffCompression;
using ptiff::io::backend::tiff::tiffIfdByteSize;
using ptiff::io::backend::tiff::TiffPredictor;

namespace {

StorageModel grayscaleModel() {
    StorageModel model;
    model.setField("imageWidth", "4");
    model.setField("imageHeight", "2");
    model.setField("samplesPerPixel", "1");
    model.setField("pixelType", "UInt8");
    return model;
}

} // namespace

TEST_CASE("planTiffWrite builds a directory matching the model's dimensions",
          "[tiff-directory-writer]") {
    auto plan = planTiffWrite(grayscaleModel());
    REQUIRE(plan.has_value());
    REQUIRE(plan->directory.imageWidth == 4);
    REQUIRE(plan->directory.imageHeight == 2);
    REQUIRE(plan->directory.samplesPerPixel == 1);
    REQUIRE(plan->directory.pixelType == ptiff::PixelType::UInt8);
    REQUIRE(plan->directory.compression == TiffCompression::None);
}

TEST_CASE("planTiffWrite computes exactly one strip covering the whole image",
          "[tiff-directory-writer]") {
    auto plan = planTiffWrite(grayscaleModel());
    REQUIRE(plan.has_value());
    REQUIRE(plan->directory.tileByteRanges.size() == 1);
    // 4 wide * 2 tall * 1 sample * 1 byte/sample = 8 bytes.
    REQUIRE(plan->directory.tileByteRanges[0].byteCount == 8);
    // The strip must start exactly where the header + IFD end.
    REQUIRE(plan->directory.tileByteRanges[0].offset == 8 + tiffIfdByteSize(plan->entries));
}

TEST_CASE("planTiffWrite computes a larger strip for RGB (3 samples)", "[tiff-directory-writer]") {
    StorageModel model;
    model.setField("imageWidth", "2");
    model.setField("imageHeight", "2");
    model.setField("samplesPerPixel", "3");
    model.setField("pixelType", "UInt8");

    auto plan = planTiffWrite(model);
    REQUIRE(plan.has_value());
    REQUIRE(plan->directory.samplesPerPixel == 3);
    // 2 * 2 * 3 * 1 = 12 bytes.
    REQUIRE(plan->directory.tileByteRanges[0].byteCount == 12);
}

TEST_CASE("planTiffWrite accepts a 5-band multispectral image", "[tiff-directory-writer]") {
    StorageModel model;
    model.setField("imageWidth", "2");
    model.setField("imageHeight", "2");
    model.setField("samplesPerPixel", "5");
    model.setField("pixelType", "UInt8");

    auto plan = planTiffWrite(model);
    REQUIRE(plan.has_value());
    REQUIRE(plan->directory.samplesPerPixel == 5);
    // 2 * 2 * 5 * 1 = 20 bytes.
    REQUIRE(plan->directory.tileByteRanges[0].byteCount == 20);
}

TEST_CASE("planTiffWrite emits ExtraSamples for a multispectral image but not for RGB/gray",
          "[tiff-directory-writer]") {
    StorageModel model;
    model.setField("imageWidth", "2");
    model.setField("imageHeight", "2");
    model.setField("samplesPerPixel", "5");
    model.setField("pixelType", "UInt8");

    auto plan = planTiffWrite(model);
    REQUIRE(plan.has_value());
    const auto hasExtraSamples =
        std::any_of(plan->entries.begin(), plan->entries.end(), [](const auto& e) {
            return e.tagId == 338;
        }); // TagId::ExtraSamples
    REQUIRE(hasExtraSamples);

    // RGB (3 bands) must NOT get an ExtraSamples tag (regression guard).
    auto rgbPlan = planTiffWrite(grayscaleModel()); // samplesPerPixel=1, no ExtraSamples either
    REQUIRE(rgbPlan.has_value());
    const auto grayHasExtraSamples = std::any_of(rgbPlan->entries.begin(),
                                                 rgbPlan->entries.end(),
                                                 [](const auto& e) { return e.tagId == 338; });
    REQUIRE_FALSE(grayHasExtraSamples);
}

TEST_CASE("planTiffWrite rejects a missing required field", "[tiff-directory-writer]") {
    StorageModel model;
    model.setField("imageHeight", "2");
    model.setField("samplesPerPixel", "1");
    model.setField("pixelType", "UInt8");

    auto plan = planTiffWrite(model);
    REQUIRE_FALSE(plan.has_value());
    REQUIRE(plan.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("planTiffWrite rejects a zero samplesPerPixel", "[tiff-directory-writer]") {
    StorageModel model = grayscaleModel();
    model.setField("samplesPerPixel", "0");

    auto plan = planTiffWrite(model);
    REQUIRE_FALSE(plan.has_value());
    REQUIRE(plan.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("planTiffWrite rejects a samplesPerPixel above the defensive cap",
          "[tiff-directory-writer]") {
    StorageModel model = grayscaleModel();
    model.setField("samplesPerPixel", "513");

    auto plan = planTiffWrite(model);
    REQUIRE_FALSE(plan.has_value());
    REQUIRE(plan.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("planTiffWrite rejects an unsupported compression request", "[tiff-directory-writer]") {
    StorageModel model = grayscaleModel();
    // "Lzw"/"PackBits"/"Deflate"/"Jpeg" are all supported now; use a made-up value that is not.
    model.setField("compression", "Ccitt");

    auto plan = planTiffWrite(model);
    REQUIRE_FALSE(plan.has_value());
    REQUIRE(plan.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("planTiffWrite accepts PackBits compression and computes a patch offset",
          "[tiff-directory-writer]") {
    ptiff::io::StorageModel model;
    model.setField("imageWidth", "3");
    model.setField("imageHeight", "2");
    model.setField("samplesPerPixel", "1");
    model.setField("pixelType", "UInt8");
    model.setField("compression", "PackBits");
    auto plan = ptiff::io::backend::tiff::planTiffWrite(model);
    REQUIRE(plan.has_value());
    REQUIRE(plan->directory.compression == ptiff::io::backend::tiff::TiffCompression::PackBits);
    REQUIRE(plan->directory.stripByteCountsPatchOffset != 0);
    // StripByteCounts entry (tag 279) writes its value at: 8 (header) + 2 (count) + 8*12 + 8
    REQUIRE(plan->directory.stripByteCountsPatchOffset ==
            ptiff::io::backend::tiff::kClassicTiffHeaderSize + 2 + 8 * 12 + 8);
}

TEST_CASE("planTiffWrite accepts Deflate compression and computes a patch offset",
          "[tiff-directory-writer]") {
    ptiff::io::StorageModel model;
    model.setField("imageWidth", "3");
    model.setField("imageHeight", "2");
    model.setField("samplesPerPixel", "1");
    model.setField("pixelType", "UInt8");
    model.setField("compression", "Deflate");
    auto plan = ptiff::io::backend::tiff::planTiffWrite(model);
    REQUIRE(plan.has_value());
    REQUIRE(plan->directory.compression == ptiff::io::backend::tiff::TiffCompression::Deflate);
    REQUIRE(plan->directory.stripByteCountsPatchOffset != 0);
}

TEST_CASE("planTiffWrite accepts Jpeg compression and computes a patch offset",
          "[tiff-directory-writer]") {
    ptiff::io::StorageModel model;
    model.setField("imageWidth", "3");
    model.setField("imageHeight", "2");
    model.setField("samplesPerPixel", "1");
    model.setField("pixelType", "UInt8");
    model.setField("compression", "Jpeg");
    auto plan = ptiff::io::backend::tiff::planTiffWrite(model);
    REQUIRE(plan.has_value());
    REQUIRE(plan->directory.compression == ptiff::io::backend::tiff::TiffCompression::Jpeg);
    REQUIRE(plan->directory.stripByteCountsPatchOffset != 0);
    REQUIRE(plan->directory.jpegQuality == 90);
}

TEST_CASE("planTiffWrite accepts Jpeg RGB compression with YCbCr photometric and subsampling tag",
          "[tiff-directory-writer]") {
    ptiff::io::StorageModel model;
    model.setField("imageWidth", "3");
    model.setField("imageHeight", "2");
    model.setField("samplesPerPixel", "3");
    model.setField("pixelType", "UInt8");
    model.setField("compression", "Jpeg");
    auto plan = ptiff::io::backend::tiff::planTiffWrite(model);
    REQUIRE(plan.has_value());

    bool foundPhotometric = false;
    bool foundSubsampling = false;
    for (const auto& entry : plan->entries) {
        if (entry.tagId == static_cast<std::uint16_t>(
                               ptiff::io::backend::tiff::TagId::PhotometricInterpretation)) {
            REQUIRE(entry.values == std::vector<std::uint32_t>{6});
            foundPhotometric = true;
        }
        if (entry.tagId ==
            static_cast<std::uint16_t>(ptiff::io::backend::tiff::TagId::YCbCrSubSampling)) {
            REQUIRE(entry.values == std::vector<std::uint32_t>{1, 1});
            foundSubsampling = true;
        }
    }
    REQUIRE(foundPhotometric);
    REQUIRE(foundSubsampling);
}

TEST_CASE("planTiffWrite parses an explicit jpegQuality field", "[tiff-directory-writer]") {
    StorageModel model = grayscaleModel();
    model.setField("compression", "Jpeg");
    model.setField("jpegQuality", "75");
    auto plan = planTiffWrite(model);
    REQUIRE(plan.has_value());
    REQUIRE(plan->directory.jpegQuality == 75);
}

TEST_CASE("planTiffWrite rejects a jpegQuality outside [0, 100]", "[tiff-directory-writer]") {
    StorageModel model = grayscaleModel();
    model.setField("compression", "Jpeg");
    model.setField("jpegQuality", "150");
    auto plan = planTiffWrite(model);
    REQUIRE_FALSE(plan.has_value());
    REQUIRE(plan.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("planTiffWrite rejects Jpeg compression with a non-UInt8 pixelType",
          "[tiff-directory-writer]") {
    StorageModel model = grayscaleModel();
    model.setField("pixelType", "UInt16");
    model.setField("compression", "Jpeg");
    auto plan = planTiffWrite(model);
    REQUIRE_FALSE(plan.has_value());
    REQUIRE(plan.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("planTiffWrite rejects Jpeg compression combined with a predictor",
          "[tiff-directory-writer]") {
    StorageModel model = grayscaleModel();
    model.setField("compression", "Jpeg");
    model.setField("predictor", "HorizontalDifferencing");
    auto plan = planTiffWrite(model);
    REQUIRE_FALSE(plan.has_value());
    REQUIRE(plan.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("planTiffWrite accepts LZW with horizontal differencing on integer samples",
          "[tiff-directory-writer]") {
    ptiff::io::StorageModel model;
    model.setField("imageWidth", "3");
    model.setField("imageHeight", "2");
    model.setField("samplesPerPixel", "3");
    model.setField("pixelType", "UInt8");
    model.setField("compression", "LZW");
    model.setField("predictor", "HorizontalDifferencing");
    auto plan = ptiff::io::backend::tiff::planTiffWrite(model);
    REQUIRE(plan.has_value());
    REQUIRE(plan->directory.compression == ptiff::io::backend::tiff::TiffCompression::Lzw);
    REQUIRE(plan->directory.predictor ==
            ptiff::io::backend::tiff::TiffPredictor::HorizontalDifferencing);
}

TEST_CASE("planTiffWrite rejects horizontal differencing on Float32 samples",
          "[tiff-directory-writer]") {
    ptiff::io::StorageModel model;
    model.setField("imageWidth", "3");
    model.setField("imageHeight", "2");
    model.setField("samplesPerPixel", "1");
    model.setField("pixelType", "Float32");
    model.setField("predictor", "HorizontalDifferencing");
    auto plan = ptiff::io::backend::tiff::planTiffWrite(model);
    REQUIRE_FALSE(plan.has_value()); // NaN-predictor semantics are undefined -> reject
}

TEST_CASE("planTiffWrite defaults to classic container when \"container\" is absent",
          "[tiff-directory-writer]") {
    ptiff::io::StorageModel model;
    model.setField("imageWidth", "4");
    model.setField("imageHeight", "2");
    model.setField("samplesPerPixel", "1");
    model.setField("pixelType", "UInt8");

    auto plan = planTiffWrite(model);
    REQUIRE(plan.has_value());
    REQUIRE(plan->isBigTiff == false);
}

TEST_CASE("planTiffWrite sets isBigTiff when container is BigTiff", "[tiff-directory-writer]") {
    ptiff::io::StorageModel model;
    model.setField("imageWidth", "4");
    model.setField("imageHeight", "2");
    model.setField("samplesPerPixel", "1");
    model.setField("pixelType", "UInt8");
    model.setField("container", "BigTiff");

    auto plan = planTiffWrite(model);
    REQUIRE(plan.has_value());
    REQUIRE(plan->isBigTiff == true);

    // Build the identical model but classic, to prove the BigTIFF header+IFD occupies more bytes.
    ptiff::io::StorageModel classicModel;
    classicModel.setField("imageWidth", "4");
    classicModel.setField("imageHeight", "2");
    classicModel.setField("samplesPerPixel", "1");
    classicModel.setField("pixelType", "UInt8");
    auto classicPlan = planTiffWrite(classicModel);
    REQUIRE(classicPlan.has_value());

    REQUIRE(plan->directory.tileByteRanges.front().offset >
            classicPlan->directory.tileByteRanges.front().offset);
}

TEST_CASE("planTiffWrite rejects an unsupported container value", "[tiff-directory-writer]") {
    ptiff::io::StorageModel model;
    model.setField("imageWidth", "4");
    model.setField("imageHeight", "2");
    model.setField("samplesPerPixel", "1");
    model.setField("pixelType", "UInt8");
    model.setField("container", "Nonsense");

    auto plan = planTiffWrite(model);
    REQUIRE_FALSE(plan.has_value());
    REQUIRE(plan.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("planTiffWrite emits Tile* tags and a TileByteRange per tile when tileWidth/tileHeight "
          "are set",
          "[tiff-directory-writer]") {
    ptiff::io::StorageModel model;
    model.setField("imageWidth", "32");
    model.setField("imageHeight", "16");
    model.setField("samplesPerPixel", "1");
    model.setField("pixelType", "UInt8");
    model.setField("tileWidth", "16");
    model.setField("tileHeight", "16");

    auto plan = ptiff::io::backend::tiff::planTiffWrite(model);
    REQUIRE(plan.has_value());

    // 32/16 = 2 columns, 16/16 = 1 row -> 2 tiles.
    REQUIRE(plan->directory.tileByteRanges.size() == 2);
    REQUIRE(plan->directory.tileByteRanges[0].byteCount == 256); // 16*16*1 byte/sample
    REQUIRE(plan->directory.tileByteRanges[1].byteCount == 256);
    // Uncompressed tiles are laid out back-to-back in row-major order.
    REQUIRE(plan->directory.tileByteRanges[1].offset ==
            plan->directory.tileByteRanges[0].offset + plan->directory.tileByteRanges[0].byteCount);

    bool sawTileWidth = false;
    bool sawTileLength = false;
    bool sawTileOffsets = false;
    bool sawTileByteCounts = false;
    bool sawStripOffsets = false;
    for (const auto& entry : plan->entries) {
        if (entry.tagId == static_cast<std::uint16_t>(ptiff::io::backend::tiff::TagId::TileWidth)) {
            sawTileWidth = true;
            REQUIRE(entry.values == std::vector<std::uint32_t>{16});
        }
        if (entry.tagId ==
            static_cast<std::uint16_t>(ptiff::io::backend::tiff::TagId::TileLength)) {
            sawTileLength = true;
            REQUIRE(entry.values == std::vector<std::uint32_t>{16});
        }
        if (entry.tagId ==
            static_cast<std::uint16_t>(ptiff::io::backend::tiff::TagId::TileOffsets)) {
            sawTileOffsets = true;
            REQUIRE(entry.values.size() == 2);
            REQUIRE(entry.values[0] != 0);
            REQUIRE(entry.values[1] != 0);
        }
        if (entry.tagId ==
            static_cast<std::uint16_t>(ptiff::io::backend::tiff::TagId::TileByteCounts)) {
            sawTileByteCounts = true;
            REQUIRE(entry.values == std::vector<std::uint32_t>{256, 256});
        }
        if (entry.tagId ==
            static_cast<std::uint16_t>(ptiff::io::backend::tiff::TagId::StripOffsets)) {
            sawStripOffsets = true;
        }
    }
    REQUIRE(sawTileWidth);
    REQUIRE(sawTileLength);
    REQUIRE(sawTileOffsets);
    REQUIRE(sawTileByteCounts);
    REQUIRE_FALSE(sawStripOffsets); // tiled write must not also emit strip tags
}

TEST_CASE("planTiffWrite rejects tileWidth set without tileHeight", "[tiff-directory-writer]") {
    ptiff::io::StorageModel model;
    model.setField("imageWidth", "32");
    model.setField("imageHeight", "16");
    model.setField("samplesPerPixel", "1");
    model.setField("pixelType", "UInt8");
    model.setField("tileWidth", "16");

    auto plan = ptiff::io::backend::tiff::planTiffWrite(model);
    REQUIRE_FALSE(plan.has_value());
}

TEST_CASE("planTiffWrite rejects a tileWidth that isn't a multiple of 16",
          "[tiff-directory-writer]") {
    ptiff::io::StorageModel model;
    model.setField("imageWidth", "32");
    model.setField("imageHeight", "16");
    model.setField("samplesPerPixel", "1");
    model.setField("pixelType", "UInt8");
    model.setField("tileWidth", "10");
    model.setField("tileHeight", "16");

    auto plan = ptiff::io::backend::tiff::planTiffWrite(model);
    REQUIRE_FALSE(plan.has_value());
}

TEST_CASE("planTiffWrite rejects compression combined with a tiled layout",
          "[tiff-directory-writer]") {
    ptiff::io::StorageModel model;
    model.setField("imageWidth", "32");
    model.setField("imageHeight", "16");
    model.setField("samplesPerPixel", "1");
    model.setField("pixelType", "UInt8");
    model.setField("tileWidth", "16");
    model.setField("tileHeight", "16");
    model.setField("compression", "PackBits");

    auto plan = ptiff::io::backend::tiff::planTiffWrite(model);
    REQUIRE_FALSE(plan.has_value());
}
