#include <array>
#include <cmath>
#include <cstddef>
#include <filesystem>
#include <fstream>
#include <span>
#include <string>
#include <utility>
#include <vector>

#include <ptiff/io/backend/tiff_backend.hpp>
#include <ptiff/io/file_binary_reader.hpp>
#include <ptiff/io/file_binary_writer.hpp>
#include <ptiff/io/storage_model.hpp>

#include <catch2/catch_test_macros.hpp>

namespace {

std::vector<std::byte> bytes(std::initializer_list<unsigned char> values) {
    std::vector<std::byte> result;
    result.reserve(values.size());
    for (auto v : values) {
        result.push_back(std::byte{v});
    }
    return result;
}

// Appends a classic 12-byte IFD entry: tagId(u16 LE), fieldType(u16 LE), count(u32 LE),
// inline value left-justified in the 4-byte value area (LE).
void appendEntry(std::vector<std::byte>& out,
                 std::uint16_t tagId,
                 std::uint16_t fieldType,
                 std::uint32_t count,
                 std::uint32_t value) {
    auto entry = bytes({static_cast<unsigned char>(tagId & 0xFF),
                        static_cast<unsigned char>(tagId >> 8),
                        static_cast<unsigned char>(fieldType & 0xFF),
                        static_cast<unsigned char>(fieldType >> 8),
                        static_cast<unsigned char>(count & 0xFF),
                        static_cast<unsigned char>((count >> 8) & 0xFF),
                        static_cast<unsigned char>((count >> 16) & 0xFF),
                        static_cast<unsigned char>((count >> 24) & 0xFF),
                        static_cast<unsigned char>(value & 0xFF),
                        static_cast<unsigned char>((value >> 8) & 0xFF),
                        static_cast<unsigned char>((value >> 16) & 0xFF),
                        static_cast<unsigned char>((value >> 24) & 0xFF)});
    out.insert(out.end(), entry.begin(), entry.end());
}

std::filesystem::path writeMinimalTiff() {
    std::vector<std::byte> file = bytes({'I', 'I', 0x2A, 0x00, 0x08, 0x00, 0x00, 0x00});
    auto entryCount = bytes({0x08, 0x00});
    file.insert(file.end(), entryCount.begin(), entryCount.end());

    appendEntry(file, 256, 3, 1, 2);   // ImageWidth = 2 (SHORT)
    appendEntry(file, 257, 3, 1, 2);   // ImageLength = 2 (SHORT)
    appendEntry(file, 258, 3, 1, 8);   // BitsPerSample = 8 (SHORT)
    appendEntry(file, 259, 3, 1, 1);   // Compression = 1 (None)
    appendEntry(file, 262, 3, 1, 1);   // PhotometricInterpretation = 1 (BlackIsZero)
    appendEntry(file, 273, 4, 1, 110); // StripOffsets = 110 (LONG)
    appendEntry(file, 278, 3, 1, 2);   // RowsPerStrip = 2 (SHORT)
    appendEntry(file, 279, 4, 1, 4);   // StripByteCounts = 4 (LONG)

    auto nextIfd = bytes({0x00, 0x00, 0x00, 0x00});
    file.insert(file.end(), nextIfd.begin(), nextIfd.end());

    REQUIRE(file.size() == 110);
    auto pixels = bytes({10, 20, 30, 40});
    file.insert(file.end(), pixels.begin(), pixels.end());

    auto path = std::filesystem::temp_directory_path() / "ptiff_tiff_backend_e2e_test.tif";
    std::ofstream stream(path, std::ios::binary | std::ios::trunc);
    stream.write(reinterpret_cast<const char*>(file.data()),
                 static_cast<std::streamsize>(file.size()));
    stream.close();
    return path;
}

} // namespace

TEST_CASE("TiffBackend::deserializeModel reads a real minimal TIFF file", "[tiff-backend-e2e]") {
    auto path = writeMinimalTiff();
    auto readerResult = ptiff::io::FileBinaryReader::open(path.string());
    REQUIRE(readerResult.has_value());
    auto& reader = **readerResult;

    ptiff::io::backend::TiffBackend backend;
    auto model = backend.deserializeModel(reader);
    REQUIRE(model.has_value());
    REQUIRE(model->children().size() == 1);
    REQUIRE(model->children().front().field("imageWidth").value() == "2");
    REQUIRE(model->children().front().field("imageHeight").value() == "2");
    REQUIRE(model->children().front().field("samplesPerPixel").value() == "1");
    REQUIRE(model->children().front().field("pixelType").value() == "UInt8");

    std::filesystem::remove(path);
}

TEST_CASE("TiffBackend::openImageSource reads real pixel bytes", "[tiff-backend-e2e]") {
    auto path = writeMinimalTiff();
    auto readerResult = ptiff::io::FileBinaryReader::open(path.string());
    REQUIRE(readerResult.has_value());
    auto& reader = **readerResult;

    ptiff::io::backend::TiffBackend backend;
    auto source = backend.openImageSource(reader);
    REQUIRE(source.has_value());

    auto tile = (*source)->readTile(ptiff::io::tile::TileIndex{});
    REQUIRE(tile.has_value());
    REQUIRE(tile->data().size() == 4);
    REQUIRE(tile->data()[0] == std::byte{10});
    REQUIRE(tile->data()[1] == std::byte{20});
    REQUIRE(tile->data()[2] == std::byte{30});
    REQUIRE(tile->data()[3] == std::byte{40});

    std::filesystem::remove(path);
}

TEST_CASE("TiffBackend::capabilities reports tiling and random access", "[tiff-backend-e2e]") {
    ptiff::io::backend::TiffBackend backend;
    auto caps = backend.capabilities();
    REQUIRE(caps.supportsTiling);
    REQUIRE(caps.supportsRandomAccess);
    REQUIRE_FALSE(caps.supportsStreaming);
    REQUIRE(caps.supportsCloudStreaming);
}

namespace {

std::filesystem::path writePackBitsCompressedTiff() {
    std::vector<std::byte> file = bytes({'I', 'I', 0x2A, 0x00, 0x08, 0x00, 0x00, 0x00});
    auto entryCount = bytes({0x08, 0x00});
    file.insert(file.end(), entryCount.begin(), entryCount.end());

    appendEntry(file, 256, 3, 1, 8);     // ImageWidth = 8 (SHORT)
    appendEntry(file, 257, 3, 1, 1);     // ImageLength = 1 (SHORT)
    appendEntry(file, 258, 3, 1, 8);     // BitsPerSample = 8 (SHORT)
    appendEntry(file, 259, 3, 1, 32773); // Compression = 32773 (PackBits)
    appendEntry(file, 262, 3, 1, 1);     // PhotometricInterpretation = 1 (BlackIsZero)
    appendEntry(file, 273, 4, 1, 110);   // StripOffsets = 110 (LONG)
    appendEntry(file, 278, 3, 1, 1);     // RowsPerStrip = 1 (SHORT)
    appendEntry(file, 279, 4, 1, 6);     // StripByteCounts = 6 (LONG) -- the compressed size

    auto nextIfd = bytes({0x00, 0x00, 0x00, 0x00});
    file.insert(file.end(), nextIfd.begin(), nextIfd.end());

    REQUIRE(file.size() == 110);
    // control=2 -> literal {0x01,0x02,0x03}; control=-4 (0xFC) -> repeat 0x09 five times.
    // Decodes to 8 bytes: {0x01,0x02,0x03,0x09,0x09,0x09,0x09,0x09}.
    auto compressedPixels = bytes({0x02, 0x01, 0x02, 0x03, 0xFC, 0x09});
    file.insert(file.end(), compressedPixels.begin(), compressedPixels.end());

    auto path = std::filesystem::temp_directory_path() / "ptiff_tiff_backend_packbits_e2e_test.tif";
    std::ofstream stream(path, std::ios::binary | std::ios::trunc);
    stream.write(reinterpret_cast<const char*>(file.data()),
                 static_cast<std::streamsize>(file.size()));
    stream.close();
    return path;
}

} // namespace

TEST_CASE("TiffBackend::openImageSource decodes a real PackBits-compressed TIFF file",
          "[tiff-backend-e2e]") {
    auto path = writePackBitsCompressedTiff();
    auto readerResult = ptiff::io::FileBinaryReader::open(path.string());
    REQUIRE(readerResult.has_value());
    auto& reader = **readerResult;

    ptiff::io::backend::TiffBackend backend;
    auto source = backend.openImageSource(reader);
    REQUIRE(source.has_value());

    auto tile = (*source)->readTile(ptiff::io::tile::TileIndex{});
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

    std::filesystem::remove(path);
}

namespace {

std::filesystem::path roundTripWriteRead(const ptiff::io::StorageModel& model,
                                         std::span<const std::byte> pixels,
                                         const std::string& fileName) {
    auto path = std::filesystem::temp_directory_path() / fileName;
    std::filesystem::remove(path);

    auto writerResult = ptiff::io::FileBinaryWriter::create(path.string());
    REQUIRE(writerResult.has_value());
    ptiff::io::backend::TiffBackend backend;

    auto serializeResult = backend.serializeModel(model, **writerResult);
    REQUIRE(serializeResult.has_value());

    auto sinkResult = backend.openImageSink(**writerResult, model);
    REQUIRE(sinkResult.has_value());

    ptiff::io::tile::Tile tile{
        ptiff::TileId{0},
        ptiff::io::tile::TileIndex{},
        ptiff::io::tile::TileRegion{.x = 0, .y = 0, .extent = (*sinkResult)->layout().tileSize},
        pixels};
    auto writeTileResult = (*sinkResult)->writeTile(tile);
    REQUIRE(writeTileResult.has_value());
    REQUIRE((**writerResult).flush().has_value());

    return path;
}

} // namespace

TEST_CASE("TiffBackend write -> read round-trips a grayscale image byte-for-byte",
          "[tiff-backend-e2e]") {
    ptiff::io::StorageModel model;
    model.setField("imageWidth", "3");
    model.setField("imageHeight", "2");
    model.setField("samplesPerPixel", "1");
    model.setField("pixelType", "UInt8");

    std::array<std::byte, 6> pixels{
        std::byte{10}, std::byte{20}, std::byte{30}, std::byte{40}, std::byte{50}, std::byte{60}};
    auto path = roundTripWriteRead(model, pixels, "ptiff_tiff_backend_roundtrip_gray.tif");

    auto readerResult = ptiff::io::FileBinaryReader::open(path.string());
    REQUIRE(readerResult.has_value());
    auto& reader = **readerResult;

    ptiff::io::backend::TiffBackend backend;
    auto readModel = backend.deserializeModel(reader);
    REQUIRE(readModel.has_value());
    REQUIRE(readModel->children().size() == 1);
    REQUIRE(readModel->children().front().field("imageWidth").value() == "3");
    REQUIRE(readModel->children().front().field("imageHeight").value() == "2");
    REQUIRE(readModel->children().front().field("samplesPerPixel").value() == "1");
    REQUIRE(readModel->children().front().field("pixelType").value() == "UInt8");

    // The read path is cursor-based, so re-open a fresh reader for the pixel read.
    auto pixelReaderResult = ptiff::io::FileBinaryReader::open(path.string());
    REQUIRE(pixelReaderResult.has_value());
    auto& pixelReader = **pixelReaderResult;
    auto source = backend.openImageSource(pixelReader);
    REQUIRE(source.has_value());
    auto tile = (*source)->readTile(ptiff::io::tile::TileIndex{});
    REQUIRE(tile.has_value());
    REQUIRE(tile->data().size() == pixels.size());
    for (std::size_t i = 0; i < pixels.size(); ++i) {
        REQUIRE(tile->data()[i] == pixels[i]);
    }

    std::filesystem::remove(path);
}

TEST_CASE("TiffBackend write -> read round-trips an RGB image byte-for-byte",
          "[tiff-backend-e2e]") {
    ptiff::io::StorageModel model;
    model.setField("imageWidth", "2");
    model.setField("imageHeight", "1");
    model.setField("samplesPerPixel", "3");
    model.setField("pixelType", "UInt8");

    std::array<std::byte, 6> pixels{
        std::byte{255}, std::byte{0}, std::byte{0}, std::byte{0}, std::byte{255}, std::byte{0}};
    auto path = roundTripWriteRead(model, pixels, "ptiff_tiff_backend_roundtrip_rgb.tif");

    auto readerResult = ptiff::io::FileBinaryReader::open(path.string());
    REQUIRE(readerResult.has_value());
    auto& reader = **readerResult;

    ptiff::io::backend::TiffBackend backend;
    auto readModel = backend.deserializeModel(reader);
    REQUIRE(readModel.has_value());
    REQUIRE(readModel->children().size() == 1);
    REQUIRE(readModel->children().front().field("samplesPerPixel").value() == "3");

    // The read path is cursor-based, so re-open a fresh reader for the pixel read.
    auto pixelReaderResult = ptiff::io::FileBinaryReader::open(path.string());
    REQUIRE(pixelReaderResult.has_value());
    auto& pixelReader = **pixelReaderResult;
    auto source = backend.openImageSource(pixelReader);
    REQUIRE(source.has_value());
    auto tile = (*source)->readTile(ptiff::io::tile::TileIndex{});
    REQUIRE(tile.has_value());
    REQUIRE(tile->data().size() == pixels.size());
    for (std::size_t i = 0; i < pixels.size(); ++i) {
        REQUIRE(tile->data()[i] == pixels[i]);
    }

    std::filesystem::remove(path);
}

TEST_CASE("TiffBackend write -> read round-trips a 6-band multispectral image byte-for-byte",
          "[tiff-backend-e2e]") {
    ptiff::io::StorageModel model;
    model.setField("imageWidth", "2");
    model.setField("imageHeight", "1");
    model.setField("samplesPerPixel", "6");
    model.setField("pixelType", "UInt8");

    std::array<std::byte, 12> pixels{std::byte{0},
                                     std::byte{10},
                                     std::byte{20},
                                     std::byte{30},
                                     std::byte{40},
                                     std::byte{50},
                                     std::byte{60},
                                     std::byte{70},
                                     std::byte{80},
                                     std::byte{90},
                                     std::byte{100},
                                     std::byte{110}};
    auto path = roundTripWriteRead(model, pixels, "ptiff_tiff_backend_roundtrip_multiband.tif");

    auto readerResult = ptiff::io::FileBinaryReader::open(path.string());
    REQUIRE(readerResult.has_value());
    auto& reader = **readerResult;

    ptiff::io::backend::TiffBackend backend;
    auto readModel = backend.deserializeModel(reader);
    REQUIRE(readModel.has_value());
    REQUIRE(readModel->children().size() == 1);
    REQUIRE(readModel->children().front().field("samplesPerPixel").value() == "6");

    // The read path is cursor-based, so re-open a fresh reader for the pixel read.
    auto pixelReaderResult = ptiff::io::FileBinaryReader::open(path.string());
    REQUIRE(pixelReaderResult.has_value());
    auto& pixelReader = **pixelReaderResult;
    auto source = backend.openImageSource(pixelReader);
    REQUIRE(source.has_value());
    auto tile = (*source)->readTile(ptiff::io::tile::TileIndex{});
    REQUIRE(tile.has_value());
    REQUIRE(tile->data().size() == pixels.size());
    for (std::size_t i = 0; i < pixels.size(); ++i) {
        REQUIRE(tile->data()[i] == pixels[i]);
    }

    std::filesystem::remove(path);
}

TEST_CASE("TiffBackend write -> read round-trips a PackBits-compressed grayscale image",
          "[tiff-backend-e2e]") {
    ptiff::io::StorageModel model;
    model.setField("imageWidth", "8");
    model.setField("imageHeight", "4");
    model.setField("samplesPerPixel", "1");
    model.setField("pixelType", "UInt8");
    model.setField("compression", "PackBits");
    // Long runs to make compression worthwhile: 16 bytes of 7 then 16 bytes of 9.
    std::array<std::byte, 32> pixels{};
    for (int i = 0; i < 16; ++i)
        pixels[static_cast<std::size_t>(i)] = std::byte{7};
    for (int i = 16; i < 32; ++i)
        pixels[static_cast<std::size_t>(i)] = std::byte{9};
    auto path = roundTripWriteRead(model, pixels, "ptiff_tiff_e2e_packbits.tif");

    auto readerResult = ptiff::io::FileBinaryReader::open(path.string());
    REQUIRE(readerResult.has_value());
    auto& reader = **readerResult;
    ptiff::io::backend::TiffBackend backend;
    auto readModel = backend.deserializeModel(reader);
    REQUIRE(readModel.has_value());
    REQUIRE(readModel->children().size() == 1);
    REQUIRE(readModel->children().front().field("pixelType").value() == "UInt8");
    // The read path is cursor-based, so re-open a fresh reader for the pixel read.
    auto pixelReaderResult = ptiff::io::FileBinaryReader::open(path.string());
    REQUIRE(pixelReaderResult.has_value());
    auto& pixelReader = **pixelReaderResult;
    auto source = backend.openImageSource(pixelReader);
    REQUIRE(source.has_value());
    auto tile = (*source)->readTile(ptiff::io::tile::TileIndex{});
    REQUIRE(tile.has_value());
    REQUIRE(tile->data().size() == pixels.size());
    for (std::size_t i = 0; i < pixels.size(); ++i) {
        REQUIRE(tile->data()[i] == pixels[i]);
    }
    std::filesystem::remove(path);
}

TEST_CASE("TiffBackend write -> read round-trips an LZW + predictor RGB image",
          "[tiff-backend-e2e]") {
    ptiff::io::StorageModel model;
    model.setField("imageWidth", "2");
    model.setField("imageHeight", "1");
    model.setField("samplesPerPixel", "3");
    model.setField("pixelType", "UInt8");
    model.setField("compression", "LZW");
    model.setField("predictor", "HorizontalDifferencing");
    std::array<std::byte, 6> pixels{std::byte{200},
                                    std::byte{100},
                                    std::byte{50},
                                    std::byte{201},
                                    std::byte{101},
                                    std::byte{51}};
    auto path = roundTripWriteRead(model, pixels, "ptiff_tiff_e2e_lzw_predictor.tif");

    auto readerResult = ptiff::io::FileBinaryReader::open(path.string());
    REQUIRE(readerResult.has_value());
    auto& reader = **readerResult;
    ptiff::io::backend::TiffBackend backend;
    auto readModel = backend.deserializeModel(reader);
    REQUIRE(readModel.has_value());
    // The read path is cursor-based, so re-open a fresh reader for the pixel read.
    auto pixelReaderResult = ptiff::io::FileBinaryReader::open(path.string());
    REQUIRE(pixelReaderResult.has_value());
    auto& pixelReader = **pixelReaderResult;
    auto source = backend.openImageSource(pixelReader);
    REQUIRE(source.has_value());
    auto tile = (*source)->readTile(ptiff::io::tile::TileIndex{});
    REQUIRE(tile.has_value());
    for (std::size_t i = 0; i < pixels.size(); ++i) {
        REQUIRE(tile->data()[i] == pixels[i]);
    }
    std::filesystem::remove(path);
}

TEST_CASE("TiffBackend write -> read round-trips a Deflate + predictor RGB image",
          "[tiff-backend-e2e]") {
    ptiff::io::StorageModel model;
    model.setField("imageWidth", "2");
    model.setField("imageHeight", "1");
    model.setField("samplesPerPixel", "3");
    model.setField("pixelType", "UInt8");
    model.setField("compression", "Deflate");
    model.setField("predictor", "HorizontalDifferencing");
    std::array<std::byte, 6> pixels{std::byte{200},
                                    std::byte{100},
                                    std::byte{50},
                                    std::byte{201},
                                    std::byte{101},
                                    std::byte{51}};
    auto path = roundTripWriteRead(model, pixels, "ptiff_tiff_e2e_deflate_predictor.tif");

    auto readerResult = ptiff::io::FileBinaryReader::open(path.string());
    REQUIRE(readerResult.has_value());
    auto& reader = **readerResult;
    ptiff::io::backend::TiffBackend backend;
    auto readModel = backend.deserializeModel(reader);
    REQUIRE(readModel.has_value());
    // The read path is cursor-based, so re-open a fresh reader for the pixel read.
    auto pixelReaderResult = ptiff::io::FileBinaryReader::open(path.string());
    REQUIRE(pixelReaderResult.has_value());
    auto& pixelReader = **pixelReaderResult;
    auto source = backend.openImageSource(pixelReader);
    REQUIRE(source.has_value());
    auto tile = (*source)->readTile(ptiff::io::tile::TileIndex{});
    REQUIRE(tile.has_value());
    for (std::size_t i = 0; i < pixels.size(); ++i) {
        REQUIRE(tile->data()[i] == pixels[i]);
    }
    std::filesystem::remove(path);
}

TEST_CASE("TiffBackend write -> read round-trips a Jpeg grayscale image within tolerance",
          "[tiff-backend-e2e]") {
    constexpr int kWidth = 16;
    constexpr int kHeight = 16;
    ptiff::io::StorageModel model;
    model.setField("imageWidth", std::to_string(kWidth));
    model.setField("imageHeight", std::to_string(kHeight));
    model.setField("samplesPerPixel", "1");
    model.setField("pixelType", "UInt8");
    model.setField("compression", "Jpeg");

    std::vector<std::byte> pixels(static_cast<std::size_t>(kWidth) * kHeight);
    for (std::size_t i = 0; i < pixels.size(); ++i) {
        pixels[i] = static_cast<std::byte>((i * 7) % 256);
    }
    auto path = roundTripWriteRead(model, pixels, "ptiff_tiff_e2e_jpeg_gray.tif");

    auto pixelReaderResult = ptiff::io::FileBinaryReader::open(path.string());
    REQUIRE(pixelReaderResult.has_value());
    auto& pixelReader = **pixelReaderResult;
    ptiff::io::backend::TiffBackend backend;
    auto source = backend.openImageSource(pixelReader);
    REQUIRE(source.has_value());
    auto tile = (*source)->readTile(ptiff::io::tile::TileIndex{});
    REQUIRE(tile.has_value());
    REQUIRE(tile->data().size() == pixels.size());
    for (std::size_t i = 0; i < pixels.size(); ++i) {
        const int actual = static_cast<int>(tile->data()[i]);
        const int expected = static_cast<int>(pixels[i]);
        REQUIRE(std::abs(actual - expected) <= 15); // lossy codec, tolerance not exact match
    }
    std::filesystem::remove(path);
}

TEST_CASE("TiffBackend write -> read round-trips a Jpeg RGB/YCbCr image within tolerance",
          "[tiff-backend-e2e]") {
    constexpr int kWidth = 16;
    constexpr int kHeight = 16;
    ptiff::io::StorageModel model;
    model.setField("imageWidth", std::to_string(kWidth));
    model.setField("imageHeight", std::to_string(kHeight));
    model.setField("samplesPerPixel", "3");
    model.setField("pixelType", "UInt8");
    model.setField("compression", "Jpeg");
    model.setField("jpegQuality", "95");

    std::vector<std::byte> pixels(static_cast<std::size_t>(kWidth) * kHeight * 3);
    for (std::size_t i = 0; i < pixels.size(); ++i) {
        pixels[i] = static_cast<std::byte>((i * 11) % 256);
    }
    auto path = roundTripWriteRead(model, pixels, "ptiff_tiff_e2e_jpeg_rgb.tif");

    auto readerResult = ptiff::io::FileBinaryReader::open(path.string());
    REQUIRE(readerResult.has_value());
    auto& reader = **readerResult;
    ptiff::io::backend::TiffBackend backend;
    auto readModel = backend.deserializeModel(reader);
    REQUIRE(readModel.has_value());
    REQUIRE(readModel->children().front().field("compression").value() == "Jpeg");

    auto pixelReaderResult = ptiff::io::FileBinaryReader::open(path.string());
    REQUIRE(pixelReaderResult.has_value());
    auto& pixelReader = **pixelReaderResult;
    auto source = backend.openImageSource(pixelReader);
    REQUIRE(source.has_value());
    auto tile = (*source)->readTile(ptiff::io::tile::TileIndex{});
    REQUIRE(tile.has_value());
    REQUIRE(tile->data().size() == pixels.size());
    for (std::size_t i = 0; i < pixels.size(); ++i) {
        const int actual = static_cast<int>(tile->data()[i]);
        const int expected = static_cast<int>(pixels[i]);
        REQUIRE(std::abs(actual - expected) <= 15); // lossy codec, tolerance not exact match
    }
    std::filesystem::remove(path);
}

TEST_CASE("TiffBackend write -> read round-trips a BigTIFF grayscale image byte-for-byte",
          "[tiff-backend-e2e]") {
    ptiff::io::StorageModel model;
    model.setField("imageWidth", "3");
    model.setField("imageHeight", "2");
    model.setField("samplesPerPixel", "1");
    model.setField("pixelType", "UInt8");
    model.setField("container", "BigTiff");

    std::array<std::byte, 6> pixels{
        std::byte{10}, std::byte{20}, std::byte{30}, std::byte{40}, std::byte{50}, std::byte{60}};
    auto path = roundTripWriteRead(model, pixels, "ptiff_tiff_e2e_bigtiff_gray.tif");

    auto readerResult = ptiff::io::FileBinaryReader::open(path.string());
    REQUIRE(readerResult.has_value());
    auto& reader = **readerResult;
    ptiff::io::backend::TiffBackend backend;
    auto readModel = backend.deserializeModel(reader);
    REQUIRE(readModel.has_value());
    REQUIRE(readModel->children().size() == 1);
    REQUIRE(readModel->children().front().field("pixelType").value() == "UInt8");
    // The read path is cursor-based, so re-open a fresh reader for the pixel read.
    auto pixelReaderResult = ptiff::io::FileBinaryReader::open(path.string());
    REQUIRE(pixelReaderResult.has_value());
    auto& pixelReader = **pixelReaderResult;
    auto source = backend.openImageSource(pixelReader);
    REQUIRE(source.has_value());
    auto tile = (*source)->readTile(ptiff::io::tile::TileIndex{});
    REQUIRE(tile.has_value());
    REQUIRE(tile->data().size() == pixels.size());
    for (std::size_t i = 0; i < pixels.size(); ++i) {
        REQUIRE(tile->data()[i] == pixels[i]);
    }
    std::filesystem::remove(path);
}

TEST_CASE("TiffBackend write -> read round-trips a multi-tile grayscale image",
          "[tiff-backend-e2e]") {
    ptiff::io::StorageModel model;
    model.setField("imageWidth", "32");
    model.setField("imageHeight", "16");
    model.setField("samplesPerPixel", "1");
    model.setField("pixelType", "UInt8");
    model.setField("tileWidth", "16");
    model.setField("tileHeight", "16");

    auto path = std::filesystem::temp_directory_path() / "ptiff_tiff_e2e_tiled_gray.tif";
    std::filesystem::remove(path);

    auto writerResult = ptiff::io::FileBinaryWriter::create(path.string());
    REQUIRE(writerResult.has_value());
    ptiff::io::backend::TiffBackend backend;

    auto serializeResult = backend.serializeModel(model, **writerResult);
    REQUIRE(serializeResult.has_value());

    auto sinkResult = backend.openImageSink(**writerResult, model);
    REQUIRE(sinkResult.has_value());

    // 32/16 = 2 columns, 16/16 = 1 row -> tiles (0,0) and (1,0). Fill tile 0 with 5s, tile 1 with
    // 9s so a mixed-up read is detectable.
    std::vector<std::byte> tile0(256, std::byte{5});
    std::vector<std::byte> tile1(256, std::byte{9});

    ptiff::io::tile::Tile writeTile0{
        ptiff::TileId{0},
        ptiff::io::tile::TileIndex{.column = 0, .row = 0, .level = 0},
        ptiff::io::tile::TileRegion{.x = 0, .y = 0, .extent = (*sinkResult)->layout().tileSize},
        tile0};
    REQUIRE((*sinkResult)->writeTile(writeTile0).has_value());

    ptiff::io::tile::Tile writeTile1{
        ptiff::TileId{1},
        ptiff::io::tile::TileIndex{.column = 1, .row = 0, .level = 0},
        ptiff::io::tile::TileRegion{.x = 16, .y = 0, .extent = (*sinkResult)->layout().tileSize},
        tile1};
    REQUIRE((*sinkResult)->writeTile(writeTile1).has_value());

    REQUIRE((**writerResult).flush().has_value());

    auto readerResult = ptiff::io::FileBinaryReader::open(path.string());
    REQUIRE(readerResult.has_value());
    auto& reader = **readerResult;
    auto readModel = backend.deserializeModel(reader);
    REQUIRE(readModel.has_value());

    auto pixelReaderResult = ptiff::io::FileBinaryReader::open(path.string());
    REQUIRE(pixelReaderResult.has_value());
    auto& pixelReader = **pixelReaderResult;
    auto source = backend.openImageSource(pixelReader);
    REQUIRE(source.has_value());

    auto readTile0 =
        (*source)->readTile(ptiff::io::tile::TileIndex{.column = 0, .row = 0, .level = 0});
    REQUIRE(readTile0.has_value());
    REQUIRE(readTile0->data().size() == 256);
    for (auto b : readTile0->data()) {
        REQUIRE(b == std::byte{5});
    }

    auto readTile1 =
        (*source)->readTile(ptiff::io::tile::TileIndex{.column = 1, .row = 0, .level = 0});
    REQUIRE(readTile1.has_value());
    REQUIRE(readTile1->data().size() == 256);
    for (auto b : readTile1->data()) {
        REQUIRE(b == std::byte{9});
    }

    std::filesystem::remove(path);
}

namespace {

// Writes a multi-image TIFF via the backend's serializeModelList + openImageSinkAt path: one
// model + one flat pixel buffer per image, in order. Each image's sink iterates its own layout
// (row-major tile order, matching the reader's readTile order), slicing contiguous `tileBytes`
// spans out of the image's buffer. Returns the temp file path.
std::filesystem::path roundTripWriteReadMulti(const std::vector<ptiff::io::StorageModel>& models,
                                              const std::vector<std::vector<std::byte>>& pixels,
                                              const std::string& fileName) {
    auto path = std::filesystem::temp_directory_path() / fileName;
    std::filesystem::remove(path);

    auto writerResult = ptiff::io::FileBinaryWriter::create(path.string());
    REQUIRE(writerResult.has_value());
    ptiff::io::backend::TiffBackend backend;

    REQUIRE(models.size() == pixels.size());
    auto serializeResult = backend.serializeModelList(models, **writerResult);
    REQUIRE(serializeResult.has_value());

    for (std::size_t i = 0; i < models.size(); ++i) {
        auto sinkResult = backend.openImageSinkAt(**writerResult, models, i);
        REQUIRE(sinkResult.has_value());
        const auto& layout = (*sinkResult)->layout();
        std::size_t cursor = 0;
        for (std::uint32_t level = 0; level < layout.levelCount; ++level) {
            for (std::uint32_t row = 0; row < layout.rows(level); ++row) {
                for (std::uint32_t col = 0; col < layout.columns(level); ++col) {
                    const ptiff::io::tile::TileIndex index{
                        .column = col, .row = row, .level = level};
                    auto region = layout.regionFor(index);
                    REQUIRE(region.has_value());
                    // For the UInt8 / single-sample test images the full tile footprint is
                    // tileSize.width x tileSize.height bytes.
                    const std::size_t frame =
                        std::size_t(layout.tileSize.width) * std::size_t(layout.tileSize.height);
                    ptiff::io::tile::Tile tile{
                        ptiff::TileId{static_cast<std::uint64_t>(frame)},
                        index,
                        *region,
                        std::span<const std::byte>(pixels[i].data() + cursor, frame)};
                    auto writeTileResult = (*sinkResult)->writeTile(tile);
                    REQUIRE(writeTileResult.has_value());
                    cursor += frame;
                }
            }
        }
    }
    REQUIRE((**writerResult).flush().has_value());
    return path;
}

} // namespace

TEST_CASE("TiffBackend writes a two-image IFD chain and reads each image back",
          "[tiff-backend-e2e][multi-image]") {
    ptiff::io::StorageModel model0;
    model0.setField("imageWidth", "3");
    model0.setField("imageHeight", "2");
    model0.setField("samplesPerPixel", "1");
    model0.setField("pixelType", "UInt8");

    // Same shape as image 0 so the only difference is the pixel payload -- catching a mix-up of
    // the two data regions.
    ptiff::io::StorageModel model1;
    model1.setField("imageWidth", "3");
    model1.setField("imageHeight", "2");
    model1.setField("samplesPerPixel", "1");
    model1.setField("pixelType", "UInt8");

    std::vector<std::byte> pixels0{
        std::byte{10}, std::byte{20}, std::byte{30}, std::byte{40}, std::byte{50}, std::byte{60}};
    std::vector<std::byte> pixels1{
        std::byte{1}, std::byte{2}, std::byte{3}, std::byte{200}, std::byte{201}, std::byte{202}};

    std::vector<ptiff::io::StorageModel> models;
    models.push_back(std::move(model0));
    models.push_back(std::move(model1));
    auto path = roundTripWriteReadMulti(models, {pixels0, pixels1}, "ptiff_tiff_e2e_two_image.tif");

    auto readerResult = ptiff::io::FileBinaryReader::open(path.string());
    REQUIRE(readerResult.has_value());
    auto& reader = **readerResult;
    ptiff::io::backend::TiffBackend backend;

    auto chain = backend.deserializeModel(reader);
    REQUIRE(chain.has_value());
    REQUIRE(chain->children().size() == 2);
    REQUIRE(chain->children()[0].field("imageWidth").value() == "3");
    REQUIRE(chain->children()[1].field("imageWidth").value() == "3");

    // Image 0's pixels from its own data region.
    auto src0Result = ptiff::io::FileBinaryReader::open(path.string());
    REQUIRE(src0Result.has_value());
    auto src0 = backend.openImageSourceAt(**src0Result, 0);
    REQUIRE(src0.has_value());
    auto tile0 = (*src0)->readTile(ptiff::io::tile::TileIndex{});
    REQUIRE(tile0.has_value());
    REQUIRE(tile0->data().size() == pixels0.size());
    for (std::size_t i = 0; i < pixels0.size(); ++i) {
        REQUIRE(tile0->data()[i] == pixels0[i]);
    }

    // Image 1's pixels from its own (different) data region.
    auto src1Result = ptiff::io::FileBinaryReader::open(path.string());
    REQUIRE(src1Result.has_value());
    auto src1 = backend.openImageSourceAt(**src1Result, 1);
    REQUIRE(src1.has_value());
    auto tile1 = (*src1)->readTile(ptiff::io::tile::TileIndex{});
    REQUIRE(tile1.has_value());
    REQUIRE(tile1->data().size() == pixels1.size());
    for (std::size_t i = 0; i < pixels1.size(); ++i) {
        REQUIRE(tile1->data()[i] == pixels1[i]);
    }

    // Out-of-range image index from the multi-IFD backend.
    auto srcBadResult = ptiff::io::FileBinaryReader::open(path.string());
    REQUIRE(srcBadResult.has_value());
    auto srcBad = backend.openImageSourceAt(**srcBadResult, 5);
    REQUIRE_FALSE(srcBad.has_value());
    REQUIRE(srcBad.error().code() == ptiff::ErrorCode::NotFound);

    std::filesystem::remove(path);
}

TEST_CASE("TiffBackend::serializeModelList on a single image is byte-identical to serializeModel",
          "[tiff-backend-e2e][multi-image]") {
    ptiff::io::StorageModel model;
    model.setField("imageWidth", "3");
    model.setField("imageHeight", "2");
    model.setField("samplesPerPixel", "1");
    model.setField("pixelType", "UInt8");

    auto singlePath = std::filesystem::temp_directory_path() / "ptiff_e2e_single_serialize.tif";
    std::filesystem::remove(singlePath);
    auto w1Result = ptiff::io::FileBinaryWriter::create(singlePath.string());
    REQUIRE(w1Result.has_value());
    ptiff::io::backend::TiffBackend backend;
    REQUIRE(backend.serializeModel(model, **w1Result).has_value());
    REQUIRE((**w1Result).flush().has_value());

    auto listPath = std::filesystem::temp_directory_path() / "ptiff_e2e_single_list.tif";
    std::filesystem::remove(listPath);
    auto wNResult = ptiff::io::FileBinaryWriter::create(listPath.string());
    REQUIRE(wNResult.has_value());
    std::vector<ptiff::io::StorageModel> models;
    models.push_back(std::move(model));
    REQUIRE(backend.serializeModelList(models, **wNResult).has_value());
    REQUIRE((**wNResult).flush().has_value());

    auto r1 = ptiff::io::FileBinaryReader::open(singlePath.string());
    auto rN = ptiff::io::FileBinaryReader::open(listPath.string());
    REQUIRE(r1.has_value());
    REQUIRE(rN.has_value());
    auto size1 = (**r1).size();
    auto sizeN = (**rN).size();
    REQUIRE(size1.has_value());
    REQUIRE(sizeN.has_value());
    REQUIRE(*size1 == *sizeN);
    std::vector<std::byte> bytes1(static_cast<std::size_t>(*size1));
    std::vector<std::byte> bytesN(static_cast<std::size_t>(*sizeN));
    auto rd1 = (**r1).read(bytes1);
    auto rdN = (**rN).read(bytesN);
    REQUIRE(rd1.has_value());
    REQUIRE(*rd1 == bytes1.size());
    REQUIRE(rdN.has_value());
    REQUIRE(*rdN == bytesN.size());
    REQUIRE(std::equal(bytes1.begin(), bytes1.end(), bytesN.begin()));

    std::filesystem::remove(singlePath);
    std::filesystem::remove(listPath);
}

TEST_CASE("TiffBackend writes a two-image chain with distinct layouts (tiled + PackBits) "
          "and reads each back",
          "[tiff-backend-e2e][multi-image]") {
    // Image 0: tiled 32x32 with 16x16 tiles (4 tiles).
    ptiff::io::StorageModel tiledModel;
    tiledModel.setField("imageWidth", "32");
    tiledModel.setField("imageHeight", "32");
    tiledModel.setField("samplesPerPixel", "1");
    tiledModel.setField("pixelType", "UInt8");
    tiledModel.setField("tileWidth", "16");
    tiledModel.setField("tileHeight", "16");

    // Image 1: a single PackBits-compressed strip (uncompressed size 16*16=256).
    ptiff::io::StorageModel packetsModel;
    packetsModel.setField("imageWidth", "16");
    packetsModel.setField("imageHeight", "16");
    packetsModel.setField("samplesPerPixel", "1");
    packetsModel.setField("pixelType", "UInt8");
    packetsModel.setField("compression", "PackBits");

    std::vector<std::byte> tiledPixels(4 * 256, std::byte{3}); // 4 tiles of 0x03
    std::vector<std::byte> packedPixels(256, std::byte{9});    // compressible run

    std::vector<ptiff::io::StorageModel> models;
    models.push_back(std::move(tiledModel));
    models.push_back(std::move(packetsModel));
    auto path = roundTripWriteReadMulti(
        models, {tiledPixels, packedPixels}, "ptiff_tiff_e2e_two_layout.tif");

    ptiff::io::backend::TiffBackend backend;
    // Image 0 (tiled) -- tile 0 and tile 3 are 0x03.
    auto src0Result = ptiff::io::FileBinaryReader::open(path.string());
    REQUIRE(src0Result.has_value());
    auto src0 = backend.openImageSourceAt(**src0Result, 0);
    REQUIRE(src0.has_value());
    for (std::uint32_t col = 0; col < 2; ++col) {
        for (std::uint32_t row = 0; row < 2; ++row) {
            auto tile = (*src0)->readTile(
                ptiff::io::tile::TileIndex{.column = col, .row = row, .level = 0});
            REQUIRE(tile.has_value());
            REQUIRE(tile->data().size() == 256);
            for (auto b : tile->data()) {
                REQUIRE(b == std::byte{3});
            }
        }
    }

    // Image 1 (compressed) decodes back to 256 bytes of 0x09.
    auto src1Result = ptiff::io::FileBinaryReader::open(path.string());
    REQUIRE(src1Result.has_value());
    auto src1 = backend.openImageSourceAt(**src1Result, 1);
    REQUIRE(src1.has_value());
    auto tile1 = (*src1)->readTile(ptiff::io::tile::TileIndex{});
    REQUIRE(tile1.has_value());
    REQUIRE(tile1->data().size() == packedPixels.size());
    for (auto b : tile1->data()) {
        REQUIRE(b == std::byte{9});
    }

    std::filesystem::remove(path);
}
