// P-6 (documents/paper/ACTION_PLAN.md): PTIFF against genuine NASA planetary data, not just
// synthetic gradient fixtures.
//
// Two independent checks:
//   read-nac <path>       Opens a real external LRO NAC-derived DTM GeoTIFF (NOT produced by
//                          libptiff) through ptiff::Reader::open()/imageSource()/readTile() --
//                          proves baseline-TIFF interop against a genuine NASA-pipeline product,
//                          not just round-tripping our own writer's output.
//   roundtrip-lola <raw-f32-path> <width> <height> <out.tif>
//                          Writes a real LOLA elevation crop (raw float32, row-major, as dumped
//                          by dump_lola_crop.py from the same cached LOLA-64ppd DEM used
//                          throughout src/sfs) through the TIFF backend with the 5 ptiff.*
//                          private-tag domains, reads it back, and verifies the pixels are
//                          bit-exact -- proves lossless roundtrip of real scientific (not
//                          synthetic) payload data.
#include <algorithm>
#include <cstddef>
#include <cstdint>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <string>
#include <vector>

#include <ptiff/io/backend_factory.hpp>
#include <ptiff/io/file_binary_writer.hpp>
#include <ptiff/io/reader.hpp>
#include <ptiff/io/storage_model.hpp>

using ptiff::io::BackendFactory;
using ptiff::io::StorageModel;

namespace {

std::uint64_t fnv1a(const std::byte* data, std::size_t n) {
    std::uint64_t h = 1469598103934665603ull;
    for (std::size_t i = 0; i < n; ++i) {
        h ^= static_cast<std::uint64_t>(data[i]);
        h *= 1099511628211ull;
    }
    return h;
}

int readNac(const std::string& path) {
    auto readerResult = ptiff::Reader::open(path);
    if (!readerResult.has_value()) {
        std::cerr << "Reader::open failed for " << path << "\n";
        return 1;
    }
    auto& reader = **readerResult;

    auto sourceResult = reader.imageSource(ptiff::ImageId{0});
    if (!sourceResult.has_value()) {
        std::cerr << "imageSource(0) failed\n";
        return 1;
    }
    auto& source = **sourceResult;
    const auto& layout = source.layout();
    std::cout << "opened " << path << "\n"
              << "  base resolution: " << layout.imageWidth << "x" << layout.imageHeight << "\n"
              << "  tile size: " << layout.tileSize.width << "x" << layout.tileSize.height << "\n"
              << "  pyramid levels: " << layout.levelCount << "\n"
              << "  tile grid (level 0): " << layout.columns() << "x" << layout.rows() << "\n";

    auto tileResult = source.readTile(ptiff::io::tile::TileIndex{0, 0, 0});
    if (!tileResult.has_value()) {
        std::cerr << "readTile(0,0,0) failed\n";
        return 1;
    }
    const auto& tile = *tileResult;
    std::cout << "  read tile (0,0,0): " << tile.data().size() << " bytes, fnv1a=" << std::hex
              << fnv1a(tile.data().data(), tile.data().size()) << std::dec << "\n";
    return 0;
}

int roundtripLola(const std::string& rawPath, int width, int height, const std::string& outPath) {
    std::ifstream in(rawPath, std::ios::binary);
    if (!in) {
        std::cerr << "cannot open " << rawPath << "\n";
        return 1;
    }
    const std::size_t nBytes =
        static_cast<std::size_t>(width) * static_cast<std::size_t>(height) * sizeof(float);
    std::vector<std::byte> pixels(nBytes);
    in.read(reinterpret_cast<char*>(pixels.data()), static_cast<std::streamsize>(nBytes));
    if (!in) {
        std::cerr << "short read on " << rawPath << "\n";
        return 1;
    }
    const auto srcChecksum = fnv1a(pixels.data(), pixels.size());

    StorageModel model;
    model.setField("imageWidth", std::to_string(width));
    model.setField("imageHeight", std::to_string(height));
    model.setField("samplesPerPixel", "1");
    model.setField("pixelType", "Float32");
    model.setField("tileWidth", std::to_string(width));
    model.setField("tileHeight", std::to_string(height));
    model.setField("ptiff.spice.frame", "IAU_MOON");
    model.setField("ptiff.spice.instrument", "LOLA");
    model.setField("ptiff.crs.body", "301");
    model.setField("ptiff.crs.projection", "equirectangular");
    model.setField("ptiff.layers.dem", "lola_64ppd_crop");
    model.setField("ptiff.provenance.software", "libptiff-0.4.0");
    model.setField("ptiff.provenance.operator", "test_real_data (P-6)");

    auto backendResult = BackendFactory::instance().create("tiff");
    if (!backendResult.has_value()) {
        std::cerr << "unknown backend: tiff\n";
        return 1;
    }
    auto& backend = **backendResult;

    std::filesystem::remove(outPath);
    auto writerResult = ptiff::io::FileBinaryWriter::create(outPath);
    if (!writerResult.has_value()) {
        std::cerr << "FileBinaryWriter::create failed\n";
        return 1;
    }
    if (!backend.serializeModel(model, **writerResult).has_value()) {
        std::cerr << "serializeModel failed\n";
        return 1;
    }
    auto sinkResult = backend.openImageSink(**writerResult, model);
    if (!sinkResult.has_value()) {
        std::cerr << "openImageSink failed\n";
        return 1;
    }
    ptiff::io::tile::Tile tile{
        ptiff::TileId{0},
        ptiff::io::tile::TileIndex{},
        ptiff::io::tile::TileRegion{.x = 0, .y = 0, .extent = (*sinkResult)->layout().tileSize},
        pixels};
    if (!(*sinkResult)->writeTile(tile).has_value()) {
        std::cerr << "writeTile failed\n";
        return 1;
    }
    if (!(**writerResult).flush().has_value()) {
        std::cerr << "flush failed\n";
        return 1;
    }
    const auto fileBytes = std::filesystem::file_size(outPath);

    auto readerResult = ptiff::Reader::open(outPath);
    if (!readerResult.has_value()) {
        std::cerr << "read-back Reader::open failed\n";
        return 1;
    }
    auto sourceResult = (*readerResult)->imageSource(ptiff::ImageId{0});
    if (!sourceResult.has_value()) {
        std::cerr << "read-back imageSource failed\n";
        return 1;
    }
    auto readBackTile = (*sourceResult)->readTile(ptiff::io::tile::TileIndex{0, 0, 0});
    if (!readBackTile.has_value()) {
        std::cerr << "read-back readTile failed\n";
        return 1;
    }
    const auto dstChecksum = fnv1a(readBackTile->data().data(), readBackTile->data().size());
    const bool bitExact = readBackTile->data().size() == pixels.size() &&
                          std::equal(pixels.begin(), pixels.end(), readBackTile->data().begin());

    std::cout << "roundtrip " << width << "x" << height << " Float32 real LOLA elevation crop\n"
              << "  wrote " << outPath << " (" << fileBytes << " bytes)\n"
              << "  src checksum (fnv1a):       " << std::hex << srcChecksum << std::dec << "\n"
              << "  read-back checksum (fnv1a): " << std::hex << dstChecksum << std::dec << "\n"
              << "  bit-exact: " << (bitExact ? "YES" : "NO") << "\n";
    return bitExact ? 0 : 1;
}

} // namespace

int main(int argc, char** argv) {
    if (argc < 2) {
        std::cerr
            << "usage: test_real_data read-nac <path>\n"
               "       test_real_data roundtrip-lola <raw-f32-path> <width> <height> <out.tif>\n";
        return 2;
    }
    const std::string mode = argv[1];
    if (mode == "read-nac" && argc >= 3) {
        return readNac(argv[2]);
    }
    if (mode == "roundtrip-lola" && argc >= 6) {
        return roundtripLola(argv[2], std::atoi(argv[3]), std::atoi(argv[4]), argv[5]);
    }
    std::cerr << "bad usage\n";
    return 2;
}
