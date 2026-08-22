// Standalone fixture generator for the P-1 interop/overhead experiment
// (documents/paper/ACTION_PLAN.md). Writes a real, complete image file
// through any registered libptiff StorageBackend (tiff/pds4/isis/...) --
// header/label (serializeModel) + actual pixel tile data
// (openImageSink/writeTile) -- so external tools and the format-overhead
// comparison can be pointed at genuine fixtures, not synthetic stubs.
// Pixel content is a deterministic synthetic gradient, not real planetary
// data -- described as such wherever the fixture is used.
//
// Usage: gen_interop_fixture <backend> <output-file> <width> <height> [--no-ptiff-fields]
#include <cstddef>
#include <cstdlib>
#include <filesystem>
#include <iostream>
#include <string>
#include <vector>

#include <ptiff/io/backend_factory.hpp>
#include <ptiff/io/file_binary_writer.hpp>
#include <ptiff/io/storage_model.hpp>

using ptiff::io::BackendFactory;
using ptiff::io::StorageModel;

namespace {

StorageModel makeModel(int width, int height, bool withPtiffFields) {
    StorageModel model;
    model.setField("imageWidth", std::to_string(width));
    model.setField("imageHeight", std::to_string(height));
    model.setField("samplesPerPixel", "1");
    model.setField("pixelType", "UInt8");
    model.setField("tileWidth", std::to_string(width));
    model.setField("tileHeight", std::to_string(height));
    if (!withPtiffFields) {
        return model;
    }
    // 65001 SPICE
    model.setField("ptiff.spice.frame", "IAU_MOON");
    model.setField("ptiff.spice.instrument", "LROC_NAC");
    model.setField("ptiff.spice.time_system", "TDB");
    model.setField("ptiff.spice.observation_time", "2026-08-20T00:00:00.000");
    // 65002 Camera geometry
    model.setField("ptiff.camera.model", "pinhole");
    model.setField("ptiff.camera.focal_length_x", "700.0");
    model.setField("ptiff.camera.focal_length_y", "700.0");
    model.setField("ptiff.camera.principal_x", std::to_string(width / 2.0));
    model.setField("ptiff.camera.principal_y", std::to_string(height / 2.0));
    // 65003 CRS
    model.setField("ptiff.crs.body", "301");
    model.setField("ptiff.crs.projection", "equirectangular");
    model.setField("ptiff.crs.reference_frame", "IAU_MOON_2000");
    // 65004 Scientific layers
    model.setField("ptiff.layers.dem",
                   "dem_" + std::to_string(width) + "x" + std::to_string(height));
    model.setField("ptiff.layers.confidence",
                   "confidence_" + std::to_string(width) + "x" + std::to_string(height));
    // 65005 Provenance
    model.setField("ptiff.provenance.software", "libptiff-0.4.0");
    model.setField("ptiff.provenance.operator", "interop-fixture-generator");
    model.setField("ptiff.provenance.commit", "v0.4.0");
    return model;
}

std::vector<std::byte> makeGradientPixels(int width, int height) {
    std::vector<std::byte> pixels;
    pixels.reserve(static_cast<std::size_t>(width) * static_cast<std::size_t>(height));
    for (int y = 0; y < height; ++y) {
        for (int x = 0; x < width; ++x) {
            const auto value = static_cast<unsigned char>((x * 3 + y * 5) % 256);
            pixels.push_back(std::byte{value});
        }
    }
    return pixels;
}

} // namespace

int main(int argc, char** argv) {
    if (argc < 5) {
        std::cerr << "usage: gen_interop_fixture <backend> <output-file> <width> <height> "
                     "[--no-ptiff-fields]\n";
        return 2;
    }
    const std::string backendName = argv[1];
    const std::string outPath = argv[2];
    const int width = std::atoi(argv[3]);
    const int height = std::atoi(argv[4]);
    const bool withPtiffFields = !(argc >= 6 && std::string(argv[5]) == "--no-ptiff-fields");

    auto backendResult = BackendFactory::instance().create(backendName);
    if (!backendResult.has_value()) {
        std::cerr << "unknown backend: " << backendName << "\n";
        return 1;
    }
    auto& backend = **backendResult;

    const auto model = makeModel(width, height, withPtiffFields);
    const auto pixels = makeGradientPixels(width, height);

    std::filesystem::remove(outPath);
    auto writerResult = ptiff::io::FileBinaryWriter::create(outPath);
    if (!writerResult.has_value()) {
        std::cerr << "FileBinaryWriter::create failed\n";
        return 1;
    }

    auto serializeResult = backend.serializeModel(model, **writerResult);
    if (!serializeResult.has_value()) {
        std::cerr << "serializeModel failed for backend " << backendName << "\n";
        return 1;
    }

    auto sinkResult = backend.openImageSink(**writerResult, model);
    if (!sinkResult.has_value()) {
        std::cerr << "openImageSink failed for backend " << backendName << "\n";
        return 1;
    }

    ptiff::io::tile::Tile tile{
        ptiff::TileId{0},
        ptiff::io::tile::TileIndex{},
        ptiff::io::tile::TileRegion{.x = 0, .y = 0, .extent = (*sinkResult)->layout().tileSize},
        pixels};
    auto writeTileResult = (*sinkResult)->writeTile(tile);
    if (!writeTileResult.has_value()) {
        std::cerr << "writeTile failed for backend " << backendName << "\n";
        return 1;
    }
    if (!(**writerResult).flush().has_value()) {
        std::cerr << "flush failed\n";
        return 1;
    }

    std::cout << "wrote " << outPath << " (" << backendName << ", " << width << "x" << height
              << ", ptiff_fields=" << (withPtiffFields ? "yes" : "no") << ", "
              << std::filesystem::file_size(outPath) << " bytes)\n";
    return 0;
}
