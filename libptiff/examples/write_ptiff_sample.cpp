// Writes a real, on-disk PTIFF sample: a tiled classic TIFF with actual pixel
// data (a synthetic radial-gradient "crater" pattern, so an opened image looks
// like something) plus the five PTIFF private tags (65001-65005) populated
// with representative values across all domains (SPICE, camera, CRS, layer,
// provenance) -- mirrors the field set used by the "PTIFF metadata tags
// serialize to the golden SHA-256 digest" golden test, but keeps the file on
// disk instead of deleting it, for external interop testing
// (documents/paper/ptiff experiments/interop_check.py).
#include <cmath>
#include <cstdint>
#include <iostream>
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

constexpr std::uint32_t kSize = 64;
constexpr std::uint32_t kTile = 16;

StorageModel buildModel(bool withPtiffTags) {
    StorageModel model;
    model.setField("imageWidth", std::to_string(kSize));
    model.setField("imageHeight", std::to_string(kSize));
    model.setField("samplesPerPixel", "1");
    model.setField("pixelType", "UInt8");
    model.setField("tileWidth", std::to_string(kTile));
    model.setField("tileHeight", std::to_string(kTile));
    if (withPtiffTags) {
        model.setField("ptiff.spice.frame", "IAU_MOON");
        model.setField("ptiff.spice.time_system", "TDB");
        model.setField("ptiff.camera.model", "pinhole");
        model.setField("ptiff.camera.focal_length_x", "100.0");
        model.setField("ptiff.crs.body", "301");
        model.setField("ptiff.layers.dem", "dem");
        model.setField("ptiff.provenance.software", "libptiff-example-0.3.0");
    }
    return model;
}

std::vector<std::byte> tilePixels(std::uint32_t col, std::uint32_t row) {
    std::vector<std::byte> pixels(static_cast<std::size_t>(kTile) * kTile, std::byte{0});
    const double cx = kSize / 2.0;
    const double cy = kSize / 2.0;
    const double maxR = kSize / 2.0;
    for (std::uint32_t y = 0; y < kTile; ++y) {
        for (std::uint32_t x = 0; x < kTile; ++x) {
            const double px = static_cast<double>(col * kTile + x);
            const double py = static_cast<double>(row * kTile + y);
            const double r = std::sqrt((px - cx) * (px - cx) + (py - cy) * (py - cy)) / maxR;
            // Inverted-bowl-like radial gradient (bright rim, dark floor).
            const double v = std::clamp(255.0 * std::min(r, 1.0), 0.0, 255.0);
            pixels[static_cast<std::size_t>(y) * kTile + x] =
                std::byte(static_cast<unsigned char>(v));
        }
    }
    return pixels;
}

int writeSample(const std::string& path, bool withPtiffTags) {
    TiffBackend backend;
    StorageModel model = buildModel(withPtiffTags);

    auto writer = ptiff::io::FileBinaryWriter::create(path);
    if (!writer.has_value()) {
        std::cerr << "create: " << writer.error().message() << '\n';
        return 1;
    }
    if (auto r = backend.serializeModel(model, **writer); !r.has_value()) {
        std::cerr << "serializeModel: " << r.error().message() << '\n';
        return 1;
    }
    auto sink = backend.openImageSink(**writer, model);
    if (!sink.has_value()) {
        std::cerr << "openImageSink: " << sink.error().message() << '\n';
        return 1;
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
                return 1;
            }
        }
    }
    std::cout << "wrote " << path << " (ptiff tags: " << (withPtiffTags ? "yes" : "no") << ")\n";
    return 0;
}

} // namespace

int main(int argc, char** argv) {
    if (argc != 3 || (std::string(argv[2]) != "with-tags" && std::string(argv[2]) != "plain")) {
        std::cerr << "usage: " << argv[0] << " <output-path> <with-tags|plain>\n";
        return 1;
    }
    return writeSample(argv[1], std::string(argv[2]) == "with-tags");
}
