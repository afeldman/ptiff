/* Round-trips a real TIFF file through the C ABI's pixel-tile-read surface
 * (ptiff_source_*): writes via the C++ core, reads back through nothing but the
 * plain C functions every language binding calls, and checks the bytes match. */
#include <algorithm>
#include <array>
#include <cstddef>
#include <cstring>
#include <filesystem>
#include <string>
#include <vector>

#include <ptiff/io/backend/tiff_backend.hpp>
#include <ptiff/io/file_binary_writer.hpp>
#include <ptiff/io/storage_model.hpp>
#include <ptiff/io/tile/tile.hpp>
#include <ptiff/io/tile/tile_index.hpp>
#include <ptiff/io/tile/tile_region.hpp>

#include "ptiff_pixel_bridge.h"

#include <catch2/catch_test_macros.hpp>

namespace {

std::filesystem::path writeGrayscaleTiff(const std::string& fileName) {
    auto path = std::filesystem::temp_directory_path() / fileName;
    std::filesystem::remove(path);

    ptiff::io::StorageModel model;
    model.setField("imageWidth", "3");
    model.setField("imageHeight", "2");
    model.setField("samplesPerPixel", "1");
    model.setField("pixelType", "UInt8");

    auto writerResult = ptiff::io::FileBinaryWriter::create(path.string());
    REQUIRE(writerResult.has_value());
    ptiff::io::backend::TiffBackend backend;

    REQUIRE(backend.serializeModel(model, **writerResult).has_value());
    auto sinkResult = backend.openImageSink(**writerResult, model);
    REQUIRE(sinkResult.has_value());

    std::array<std::byte, 6> pixels{
        std::byte{10}, std::byte{20}, std::byte{30}, std::byte{40}, std::byte{50}, std::byte{60}};
    ptiff::io::tile::Tile tile{
        ptiff::TileId{0},
        ptiff::io::tile::TileIndex{},
        ptiff::io::tile::TileRegion{.x = 0, .y = 0, .extent = (*sinkResult)->layout().tileSize},
        pixels};
    REQUIRE((*sinkResult)->writeTile(tile).has_value());
    REQUIRE((**writerResult).flush().has_value());

    return path;
}

} // namespace

TEST_CASE("ptiff_source_open reads back real pixel bytes written by the C++ core",
          "[c-abi][pixel-bridge]") {
    auto path = writeGrayscaleTiff("ptiff_c_abi_pixel_bridge_gray.tif");

    int err = 0;
    ptiff_source* source = ptiff_source_open(path.string().c_str(), &err);
    REQUIRE(source != nullptr);

    ptiff_image_descriptor desc{};
    REQUIRE(ptiff_source_descriptor(source, &desc) == 0);
    REQUIRE(desc.width == 3);
    REQUIRE(desc.height == 2);
    REQUIRE(desc.pixel_type == PTIFF_PIXEL_UINT8);
    REQUIRE(desc.channel_count == 1);

    REQUIRE(ptiff_source_tile_columns(source) == 1);
    REQUIRE(ptiff_source_tile_rows(source) == 1);
    REQUIRE(ptiff_source_tile_byte_size(source) == 6);

    std::vector<uint8_t> buffer(ptiff_source_tile_byte_size(source));
    size_t bytesRead = 0;
    REQUIRE(ptiff_source_read_tile(source, 0, 0, buffer.data(), buffer.size(), &bytesRead) == 0);
    REQUIRE(bytesRead == 6);
    const std::array<uint8_t, 6> expected{10, 20, 30, 40, 50, 60};
    for (size_t i = 0; i < expected.size(); ++i) {
        REQUIRE(buffer[i] == expected[i]);
    }

    ptiff_source_close(source);
    std::filesystem::remove(path);
}

TEST_CASE("ptiff_source_open returns NULL and a negative error code for a missing file",
          "[c-abi][pixel-bridge]") {
    int err = 0;
    ptiff_source* source = ptiff_source_open("/nonexistent/path/does_not_exist.tif", &err);
    REQUIRE(source == nullptr);
    REQUIRE(err < 0);
}

TEST_CASE("ptiff_source_read_tile rejects a buffer that's too small", "[c-abi][pixel-bridge]") {
    auto path = writeGrayscaleTiff("ptiff_c_abi_pixel_bridge_small_buffer.tif");

    ptiff_source* source = ptiff_source_open(path.string().c_str(), nullptr);
    REQUIRE(source != nullptr);

    std::vector<uint8_t> tooSmall(2);
    size_t bytesRead = 0;
    REQUIRE(ptiff_source_read_tile(source, 0, 0, tooSmall.data(), tooSmall.size(), &bytesRead) < 0);

    ptiff_source_close(source);
    std::filesystem::remove(path);
}

TEST_CASE("ptiff_source_close(NULL) and NULL-argument calls are no-ops/safe",
          "[c-abi][pixel-bridge]") {
    ptiff_source_close(nullptr);
    REQUIRE(ptiff_source_descriptor(nullptr, nullptr) < 0);
    REQUIRE(ptiff_source_tile_columns(nullptr) == 0);
    REQUIRE(ptiff_source_tile_rows(nullptr) == 0);
    REQUIRE(ptiff_source_tile_byte_size(nullptr) == 0);
}

// ---------------------------------------------------------------------------
// Write side (ptiff_sink_*): a sink writes a fresh tiled TIFF, then we read it
// back only through the plain C functions every binding uses and check the
// pixel bytes round-trip.
// ---------------------------------------------------------------------------

namespace {

// Writes a 2x2 grid of 16x16 UInt8 grayscale tiles (a 32x32 image) via the C ABI
// sink surface, with the i-th tile's 256 sample values filled on write. Returns
// the temp file path.
std::filesystem::path writeThroughSink(
    const char* fileName, uint32_t width, uint32_t height, int pixelType, uint32_t channels) {
    auto path = std::filesystem::temp_directory_path() / fileName;
    std::filesystem::remove(path);
    const uint32_t tileW = 16, tileH = 16;
    const size_t sampleBytes = (pixelType == PTIFF_PIXEL_FLOAT32) ? 4u : 1u;

    ptiff_image_descriptor desc{};
    desc.width = width;
    desc.height = height;
    desc.pixel_type = pixelType;
    desc.channel_count = channels;
    desc.has_tile_info = 1;
    desc.tile_info.tile_width = tileW;
    desc.tile_info.tile_height = tileH;
    desc.has_compression = 1;
    desc.compression = PTIFF_COMPRESSION_NONE;

    ptiff_sink* sink = ptiff_sink_create(path.string().c_str(), &desc);
    REQUIRE(sink != nullptr);

    const uint32_t cols = ptiff_sink_tile_columns(sink);
    const uint32_t rows = ptiff_sink_tile_rows(sink);
    const size_t tileBytes = ptiff_sink_tile_byte_size(sink);
    const size_t perTile = static_cast<size_t>(tileW) * tileH * channels * sampleBytes;
    REQUIRE(cols == (width + tileW - 1) / tileW);
    REQUIRE(rows == (height + tileH - 1) / tileH);
    REQUIRE(tileBytes == perTile);

    std::vector<uint8_t> buffer(tileBytes);
    for (uint32_t r = 0; r < rows; ++r) {
        for (uint32_t c = 0; c < cols; ++c) {
            const uint32_t base = r * cols + c;
            if (pixelType == PTIFF_PIXEL_FLOAT32) {
                const float value = static_cast<float>(base + 1);
                std::vector<uint8_t> sample(sizeof(float));
                std::memcpy(sample.data(), &value, sizeof(float));
                for (size_t i = 0; i < perTile; i += sizeof(float)) {
                    std::copy(sample.begin(),
                              sample.end(),
                              buffer.begin() + static_cast<std::ptrdiff_t>(i));
                }
            } else {
                std::fill(buffer.begin(), buffer.end(), static_cast<uint8_t>(base + 1));
            }
            REQUIRE(ptiff_sink_write_tile(sink, c, r, buffer.data(), buffer.size()) == 0);
        }
    }

    ptiff_sink_close(sink);
    return path;
}

} // namespace

TEST_CASE("ptiff_sink writes a tiled TIFF that ptiff_source reads back",
          "[c-abi][pixel-bridge][sink]") {
    const uint32_t width = 32, height = 32;
    auto path =
        writeThroughSink("ptiff_c_abi_sink_roundtrip.tif", width, height, PTIFF_PIXEL_UINT8, 1);

    // Read back through the plain C read surface.
    int err = 0;
    ptiff_source* source = ptiff_source_open(path.string().c_str(), &err);
    REQUIRE(source != nullptr);

    ptiff_image_descriptor desc{};
    REQUIRE(ptiff_source_descriptor(source, &desc) == 0);
    REQUIRE(desc.width == width);
    REQUIRE(desc.height == height);
    REQUIRE(desc.pixel_type == PTIFF_PIXEL_UINT8);
    REQUIRE(desc.channel_count == 1);
    REQUIRE(desc.has_tile_info == 1);

    const size_t bytesPerTile = static_cast<size_t>(16) * 16;
    REQUIRE(ptiff_source_tile_byte_size(source) == bytesPerTile);
    REQUIRE(ptiff_source_read_tile(source, 0, 0, nullptr, 0, nullptr) < 0); // null args rejected

    std::vector<uint8_t> buffer(bytesPerTile);
    for (uint32_t r = 0; r < 2; ++r) {
        for (uint32_t c = 0; c < 2; ++c) {
            size_t bytesRead = 0;
            REQUIRE(ptiff_source_read_tile(
                        source, c, r, buffer.data(), buffer.size(), &bytesRead) == 0);
            REQUIRE(bytesRead == bytesPerTile);
            const uint8_t expect = static_cast<uint8_t>(r * 2 + c + 1);
            for (size_t i = 0; i < bytesPerTile; ++i) {
                REQUIRE(buffer[i] == expect);
            }
        }
    }

    ptiff_source_close(source);
    std::filesystem::remove(path);
}

TEST_CASE("ptiff_sink_* NULL-argument and validation calls are safe",
          "[c-abi][pixel-bridge][sink]") {
    ptiff_sink_close(nullptr);
    REQUIRE(ptiff_sink_tile_columns(nullptr) == 0);
    REQUIRE(ptiff_sink_tile_rows(nullptr) == 0);
    REQUIRE(ptiff_sink_tile_byte_size(nullptr) == 0);
    REQUIRE(ptiff_sink_write_tile(nullptr, 0, 0, nullptr, 0) < 0);

    // Untiled descriptor is rejected.
    ptiff_image_descriptor d{};
    d.width = 32;
    d.height = 32;
    d.pixel_type = PTIFF_PIXEL_UINT8;
    d.channel_count = 1;
    d.has_tile_info = 0;
    ptiff_sink* sink = ptiff_sink_create("ptiff_c_abi_sink_invalid.tif", &d);
    REQUIRE(sink == nullptr);
}

TEST_CASE("ptiff_sink round-trips Float32 pixels", "[c-abi][pixel-bridge][sink]") {
    const uint32_t width = 16, height = 16;
    auto path =
        writeThroughSink("ptiff_c_abi_sink_float32.tif", width, height, PTIFF_PIXEL_FLOAT32, 1);

    int err = 0;
    ptiff_source* source = ptiff_source_open(path.string().c_str(), &err);
    REQUIRE(source != nullptr);

    const size_t bytesPerTile = static_cast<size_t>(16) * 16 * 4;
    std::vector<uint8_t> buffer(bytesPerTile);
    size_t bytesRead = 0;
    REQUIRE(ptiff_source_read_tile(source, 0, 0, buffer.data(), buffer.size(), &bytesRead) == 0);
    REQUIRE(bytesRead == bytesPerTile);
    float value = 0.0f;
    std::memcpy(&value, buffer.data(), sizeof(float));
    REQUIRE(value == 1.0f); // tile (0,0) carries base+1 == 1

    ptiff_source_close(source);
    std::filesystem::remove(path);
}

TEST_CASE("ptiff_sink rejects out-of-range tile indices and wrong buffer sizes",
          "[c-abi][pixel-bridge][sink]") {
    auto path = writeThroughSink("ptiff_c_abi_sink_edgetest.tif", 32, 32, PTIFF_PIXEL_UINT8, 1);
    ptiff_image_descriptor desc{};
    desc.width = 32;
    desc.height = 32;
    desc.pixel_type = PTIFF_PIXEL_UINT8;
    desc.channel_count = 1;
    desc.has_tile_info = 1;
    desc.tile_info.tile_width = 16;
    desc.tile_info.tile_height = 16;
    ptiff_sink* sink = ptiff_sink_create(path.string().c_str(), &desc);
    REQUIRE(sink != nullptr);

    std::vector<uint8_t> buffer(ptiff_sink_tile_byte_size(sink));
    // Out-of-range tile index (grid is 2x2).
    REQUIRE(ptiff_sink_write_tile(sink, 5, 0, buffer.data(), buffer.size()) < 0);
    // Wrong buffer size (too small).
    REQUIRE(ptiff_sink_write_tile(sink, 0, 0, buffer.data(), buffer.size() - 1) < 0);

    ptiff_sink_close(sink);
    std::filesystem::remove(path);
}

TEST_CASE("ptiff_sink_create_camera writes camera metadata that ptiff_open_path_camera reads",
          "[c-abi][pixel-bridge][sink][camera]") {
    // Build a structured camera to persist.
    ptiff_camera cam{};
    cam.has_intrinsics = 1;
    cam.focal_length_x = 700.0;
    cam.focal_length_y = 715.0;
    cam.principal_x = 32.0;
    cam.principal_y = 24.0;
    cam.has_extrinsics = 1;
    cam.rotation_w = 1.0;
    cam.rotation_x = 0.0;
    cam.rotation_y = 0.0;
    cam.rotation_z = 0.0;
    cam.position_x = 1.0;
    cam.position_y = 2.0;
    cam.position_z = 3.0;
    std::strncpy(cam.timestamp, "2026-08-21T12:34:56.000Z", sizeof(cam.timestamp) - 1);

    const std::filesystem::path path =
        std::filesystem::temp_directory_path() / "ptiff_c_abi_sink_camera.tif";
    std::filesystem::remove(path);

    ptiff_image_descriptor desc{};
    desc.width = 32;
    desc.height = 32;
    desc.pixel_type = PTIFF_PIXEL_UINT8;
    desc.channel_count = 1;
    desc.has_tile_info = 1;
    desc.tile_info.tile_width = 16;
    desc.tile_info.tile_height = 16;

    ptiff_sink* sink = ptiff_sink_create_camera(path.string().c_str(), &desc, &cam);
    REQUIRE(sink != nullptr);
    // Write a single tile so the file is valid and complete.
    std::vector<uint8_t> buffer(ptiff_sink_tile_byte_size(sink), 7);
    REQUIRE(ptiff_sink_write_tile(sink, 0, 0, buffer.data(), buffer.size()) == 0);
    ptiff_sink_close(sink);

    // Read the camera back through the read-only C ABI function.
    ptiff_camera read{};
    REQUIRE(ptiff_open_path_camera(path.string().c_str(), &read) == 0);
    REQUIRE(read.has_intrinsics == 1);
    REQUIRE(read.focal_length_x == 700.0);
    REQUIRE(read.focal_length_y == 715.0);
    REQUIRE(read.principal_x == 32.0);
    REQUIRE(read.principal_y == 24.0);
    REQUIRE(read.has_extrinsics == 1);
    REQUIRE(read.position_x == 1.0);
    REQUIRE(read.position_y == 2.0);
    REQUIRE(read.position_z == 3.0);
    REQUIRE(std::string(read.timestamp) == "2026-08-21T12:34:56.000Z");

    std::filesystem::remove(path);
}

TEST_CASE("ptiff_sink_create_camera rejects a NULL camera", "[c-abi][pixel-bridge][sink][camera]") {
    ptiff_image_descriptor desc{};
    desc.width = 32;
    desc.height = 32;
    desc.pixel_type = PTIFF_PIXEL_UINT8;
    desc.channel_count = 1;
    desc.has_tile_info = 1;
    desc.tile_info.tile_width = 16;
    desc.tile_info.tile_height = 16;
    ptiff_sink* sink = ptiff_sink_create_camera("ptiff_c_abi_sink_nullcam.tif", &desc, nullptr);
    REQUIRE(sink == nullptr);
}
