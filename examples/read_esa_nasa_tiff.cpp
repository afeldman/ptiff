// examples/read_esa_nasa_tiff.cpp
//
// Reads a real ESA / NASA TIFF or BigTIFF with libptiff and reports what the
// format-neutral StorageModel surface looks like plus a few tiles of pixel
// data. The bundled samples are an ESA Copernicus Sentinel-2 L2A true-colour
// scene and a NASA ASTER (Terra) VNIR scene, but any TIFF/BigTIFF libptiff
// supports (Landsat, MODIS, Sentinel-1, ...) can be passed on the command line.
//
// This is deliberately a *reader* example: it exercises the same public API a
// binding (Python / Go / Rust) or a tool would use to open and inspect a TIFF
// document, decode its metadata and stream a tile back out of the pixel blob.
//
// Build & run (once the data has been fetched, see examples/README.md):
//   cmake --build <build-dir> --target ptiff_example_read_esa_nasa_tiff
//   ./<build-dir>/examples/ptiff_example_read_esa_nasa_tiff <path-to-tiff>
//
// Exit codes: 0 = success, 1 = usage/argument error, 2 = read/metadata error.

#include <algorithm>
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <filesystem>
#include <iostream>
#include <span>
#include <string>
#include <vector>

#include <ptiff/io/backend/tiff_backend.hpp>
#include <ptiff/io/file_binary_reader.hpp>
#include <ptiff/io/image_source.hpp>
#include <ptiff/io/storage_model.hpp>
#include <ptiff/io/tile/tile.hpp>
#include <ptiff/io/tile/tile_layout.hpp>

using ptiff::io::StorageModel;
using ptiff::io::backend::TiffBackend;
using ptiff::io::tile::TileIndex;
using ptiff::io::tile::TileLayout;

namespace {

constexpr const char* kUsage =
    "usage: read_esa_nasa_tiff <path-to-tiff-or-bigtiff>\n"
    "\n"
    "Reads a real ESA/NASA TIFF or BigTIFF (Sentinel-2, ASTER, Landsat,\n"
    "MODIS, ...) with libptiff and prints its metadata plus the first few\n"
    "tiles of pixel data.\n";

int fail(const ptiff::Error& error) {
    std::cerr << "error: " << error.message() << '\n';
    return 2;
}

// Print every field of the model tree. Through TiffBackend a TIFF's IFD chain
// (or COG pyramid overviews) is exposed as nested StorageModel children, so we
// walk the whole tree and number each image node encountered.
// To keep the output readable we stop after the 6 deepest nodes.
void printModel(const StorageModel& model) {
    std::vector<const StorageModel*> work = {&model};
    unsigned image = 0;
    std::size_t node = 0;
    while (node < work.size() && node < 6) {
        const StorageModel& m = *work[node++];
        if (image > 0) {
            std::cout << "    image #" << image << '\n';
        }
        m.for_each_field([&](std::string_view key, std::string_view value) {
            std::cout << "        " << key << " = " << value << '\n';
        });
        for (const StorageModel& child : m.children()) {
            work.push_back(&child);
        }
        ++image;
    }
}

unsigned byteValue(std::byte b) {
    return static_cast<unsigned>(std::to_integer<unsigned char>(b));
}

// Print a compact summary of one tile's raw pixel payload.
void printTileSummary(const ptiff::io::tile::Tile& tile) {
    const auto& region = tile.region();
    const auto& data = tile.data();
    const auto& extent = region.extent;
    std::cout << "      tile pixel region: x=" << region.x << " y=" << region.y;
    std::cout << "  size=" << extent.width << 'x' << extent.height;
    std::cout << "  raw bytes=" << data.size() << '\n';
    if (data.empty()) {
        return;
    }
    // Byte-wise min/max of the raw samples. Only meaningful as *values* for
    // single-byte (UInt8) samples; multi-byte samples depend on the file's byte
    // order, which the public API intentionally does not expose. The range is
    // still useful here as a coarse sanity check that real pixel data came back.
    unsigned lo = byteValue(data.front());
    unsigned hi = lo;
    unsigned long long sum = 0;
    for (std::byte b : data) {
        const unsigned v = byteValue(b);
        lo = std::min(lo, v);
        hi = std::max(hi, v);
        sum += v;
    }
    std::cout << "      raw-byte range: " << lo << ".." << hi;
    std::cout << "   sum: " << sum << '\n';
}

// Read a handful of tiles from the top-left of level 0.
void readFirstTiles(ptiff::io::ImageSource& source) {
    const TileLayout& layout = source.layout();
    std::cout << "    image size:   " << layout.imageWidth << 'x' << layout.imageHeight;
    std::cout << "   levels: " << layout.levelCount << '\n';
    if (layout.tileSize.width == 0 || layout.tileSize.height == 0) {
        std::cout << "    (no tiling declared; skipping tile scan)\n";
        return;
    }
    std::cout << "    tile size:    " << layout.tileSize.width << 'x' << layout.tileSize.height;
    std::cout << '\n';
    std::cout << "    grid:         " << layout.columns() << " cols x " << layout.rows();
    std::cout << " rows\n";

    constexpr std::uint32_t kMaxTiles = 6;
    unsigned printed = 0;
    // Scan top-to-bottom, left-to-right until we have printed kMaxTiles valid
    // tiles (out-of-range / padded tiles are simply skipped).
    for (std::uint32_t row = 0; row < layout.rows() && printed < kMaxTiles; ++row) {
        for (std::uint32_t col = 0; col < layout.columns() && printed < kMaxTiles; ++col) {
            auto tile = source.readTile(TileIndex{.column = col, .row = row, .level = 0});
            if (!tile.has_value()) {
                continue;
            }
            std::cout << "    tile (col=" << col << ", row=" << row << "):\n";
            printTileSummary(*tile);
            ++printed;
        }
    }
    if (printed == 0) {
        std::cout << "    (no readable tiles at top-left of level 0)\n";
    }
}

} // namespace

int main(int argc, char** argv) {
    if (argc < 2) {
        std::cerr << kUsage;
        return 1;
    }
    const std::filesystem::path path = argv[1];
    std::error_code ec;
    if (!std::filesystem::exists(path, ec) || ec) {
        std::cerr << "error: file does not exist: " << path << '\n';
        return 1;
    }
    const std::uintmax_t bytes = std::filesystem::file_size(path, ec);
    std::cout << "ESA/NASA TIFF reader (libptiff)\n";
    std::cout << "  file: " << path.string() << '\n';
    std::printf("  size: %llu bytes (%.2f MiB)\n",
                static_cast<unsigned long long>(bytes),
                static_cast<double>(bytes) / 1048576.0);

    TiffBackend backend;

    // 1) Metadata: header + IFD chain -> StorageModel.
    auto metadataReader = ptiff::io::FileBinaryReader::open(path.string());
    if (!metadataReader.has_value()) {
        return fail(metadataReader.error());
    }
    auto model = backend.deserializeModel(**metadataReader);
    if (!model.has_value()) {
        return fail(model.error());
    }
    std::cout << "\n[metadata]\n";
    printModel(*model);

    // 2) Pixel surface: layout + first tiles.
    auto pixelReader = ptiff::io::FileBinaryReader::open(path.string());
    if (!pixelReader.has_value()) {
        return fail(pixelReader.error());
    }
    auto source = backend.openImageSource(**pixelReader);
    if (!source.has_value()) {
        return fail(source.error());
    }
    std::cout << "\n[image]\n";
    readFirstTiles(**source);

    std::cout << "\nOK — ESA/NASA TIFF read successfully.\n";
    return 0;
}
