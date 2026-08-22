// examples/write_local_sample.cpp
//
// Generates a small, fully-local PTIFF file (no Internet, no big downloads): a
// 128x128 UInt8 radial-gradient "crater" pattern, tiled 32x32, plus a complete
// pinhole camera calibration written into the standard ptiff.camera.* extension
// fields. The resulting file can be inspected with read_local_sample and copied
// with copy_local_sample.
//
// Build & run:
//   cmake --build <build-dir> --target ptiff_example_write_local_sample
//   ./<build-dir>/examples/ptiff_example_write_local_sample <output-path.tif>
//
// Exit codes: 0 = success, 1 = usage/argument error, 2 = write error.

#include <algorithm>
#include <cmath>
#include <cstddef>
#include <cstdint>
#include <iostream>
#include <span>
#include <string>
#include <vector>

#include <ptiff/io/backend/tiff_backend.hpp>
#include <ptiff/io/file_binary_writer.hpp>
#include <ptiff/io/storage_model.hpp>
#include <ptiff/io/tile/tile.hpp>
#include <ptiff/io/tile/tile_index.hpp>

using ptiff::io::StorageModel;
using ptiff::io::backend::TiffBackend;
using ptiff::io::tile::Tile;
using ptiff::io::tile::TileIndex;

namespace {

constexpr std::uint32_t kSize = 128; // image: 128 x 128
constexpr std::uint32_t kTile = 32;  // tile:  32 x 32  -> 4x4 grid

// Builds the StorageModel with the image descriptor and a complete pinhole
// camera calibration persisted as ptiff.camera.* extension fields.
StorageModel buildModel() {
    StorageModel model;
    model.setField("imageWidth", std::to_string(kSize));
    model.setField("imageHeight", std::to_string(kSize));
    model.setField("samplesPerPixel", "1");
    model.setField("pixelType", "UInt8");
    model.setField("tileWidth", std::to_string(kTile));
    model.setField("tileHeight", std::to_string(kTile));

    // Full pinhole camera calibration (matches the fields read back by the
    // Rust/Python bindings' Camera type).
    model.setField("ptiff.camera.model", "pinhole");
    model.setField("ptiff.camera.focal_length_x", "700.0");
    model.setField("ptiff.camera.focal_length_y", "700.0");
    model.setField("ptiff.camera.principal_x", "64.0");
    model.setField("ptiff.camera.principal_y", "64.0");
    // Camera-to-world rotation quaternion (identity pose here).
    model.setField("ptiff.camera.rotation_w", "1.0");
    model.setField("ptiff.camera.rotation_x", "0.0");
    model.setField("ptiff.camera.rotation_y", "0.0");
    model.setField("ptiff.camera.rotation_z", "0.0");
    // Camera position in world units.
    model.setField("ptiff.camera.position_x", "0.0");
    model.setField("ptiff.camera.position_y", "0.0");
    model.setField("ptiff.camera.position_z", "100.0");
    model.setField("ptiff.camera.timestamp", "2026-08-21T12:34:56.000Z");

    model.setField("ptiff.provenance.software", "ptiff-example-write-local");

    return model;
}

// One tile worth of pixels of the synthetic "crater" radial gradient, plus a
// faint fiducial crosshair at the top/left edges so tile boundaries are
// visually obvious.
std::vector<std::byte> tilePixels(std::uint32_t col, std::uint32_t row) {
    const std::size_t tileBytes = static_cast<std::size_t>(kTile) * kTile;
    std::vector<std::byte> pixels(tileBytes, std::byte{0});
    const double cx = kSize / 2.0;
    const double cy = kSize / 2.0;
    const double maxR = kSize / 2.0;
    for (std::uint32_t y = 0; y < kTile; ++y) {
        for (std::uint32_t x = 0; x < kTile; ++x) {
            const double px = static_cast<double>(col * kTile + x);
            const double py = static_cast<double>(row * kTile + y);
            const double r = std::sqrt((px - cx) * (px - cx) + (py - cy) * (py - cy)) / maxR;
            double v = 255.0 * std::clamp(r, 0.0, 1.0); // radial gradient

            // faint fiducial crosshair along the tile grid
            const bool onCross = (col == 0 && x == 0) || (row == 0 && y == 0);
            if (onCross)
                v = std::max(v, 80.0);

            pixels[static_cast<std::size_t>(y) * kTile + x] =
                std::byte(static_cast<unsigned char>(v));
        }
    }
    return pixels;
}

int writeSample(const std::string& path) {
    TiffBackend backend;
    StorageModel model = buildModel();

    auto writer = ptiff::io::FileBinaryWriter::create(path);
    if (!writer.has_value()) {
        std::cerr << "create: " << writer.error().message() << '\n';
        return 2;
    }
    if (auto r = backend.serializeModel(model, **writer); !r.has_value()) {
        std::cerr << "serializeModel: " << r.error().message() << '\n';
        return 2;
    }
    auto sink = backend.openImageSink(**writer, model);
    if (!sink.has_value()) {
        std::cerr << "openImageSink: " << sink.error().message() << '\n';
        return 2;
    }
    const std::uint32_t cols = kSize / kTile;
    const std::uint32_t rows = kSize / kTile;
    for (std::uint32_t row = 0; row < rows; ++row) {
        for (std::uint32_t col = 0; col < cols; ++col) {
            std::vector<std::byte> pixels = tilePixels(col, row);
            Tile tile(ptiff::TileId{0},
                      TileIndex{.column = col, .row = row, .level = 0},
                      (*sink)
                          ->layout()
                          .regionFor(TileIndex{.column = col, .row = row, .level = 0})
                          .value(),
                      std::span<const std::byte>(pixels.data(), pixels.size()));
            if (auto r = (*sink)->writeTile(tile); !r.has_value()) {
                std::cerr << "writeTile(" << col << "," << row << "): " << r.error().message()
                          << '\n';
                return 2;
            }
        }
    }
    std::cout << "wrote " << path << " (" << kSize << "x" << kSize << ", " << cols << "x" << rows
              << " tiles, pinhole camera)\n";
    return 0;
}

} // namespace

int main(int argc, char** argv) {
    if (argc != 2) {
        std::cerr << "usage: " << argv[0] << " <output-path.tif>\n";
        return 1;
    }
    return writeSample(argv[1]);
}
