#include <algorithm>
#include <cstddef>
#include <iostream>
#include <string>

#include <ptiff/io/backend/tiff_backend.hpp>
#include <ptiff/io/file_binary_reader.hpp>

namespace {

int fail(const ptiff::Error& error) {
    std::cerr << "error: " << error.message() << '\n';
    return 1;
}

} // namespace

int main(int argc, char** argv) {
    if (argc != 2) {
        std::cerr << "usage: " << argv[0] << " <path-to-tiff-file>\n";
        return 1;
    }
    const std::string path = argv[1];

    ptiff::io::backend::TiffBackend backend;

    auto metadataReader = ptiff::io::FileBinaryReader::open(path);
    if (!metadataReader.has_value()) {
        return fail(metadataReader.error());
    }
    auto model = backend.deserializeModel(**metadataReader);
    if (!model.has_value()) {
        return fail(model.error());
    }

    std::cout << "Width:        " << model->field("imageWidth").value() << '\n';
    std::cout << "Height:       " << model->field("imageHeight").value() << '\n';
    std::cout << "PixelType:    " << model->field("pixelType").value() << '\n';
    std::cout << "Compression:  " << model->field("compression").value() << '\n';
    std::cout << "Predictor:    " << model->field("predictor").value() << '\n';
    std::cout << "SamplesPerPixel: " << model->field("samplesPerPixel").value() << '\n';

    auto pixelReader = ptiff::io::FileBinaryReader::open(path);
    if (!pixelReader.has_value()) {
        return fail(pixelReader.error());
    }
    auto source = backend.openImageSource(**pixelReader);
    if (!source.has_value()) {
        return fail(source.error());
    }

    std::cout << "\nReading tile (0,0)...\n";
    auto tile = (*source)->readTile(ptiff::io::tile::TileIndex{.column = 0, .row = 0, .level = 0});
    if (!tile.has_value()) {
        return fail(tile.error());
    }

    std::cout << "  bytes: " << tile->data().size() << '\n';

    if (model->field("pixelType").value() == "UInt8") {
        // std::byte has no operator<, so std::minmax_element needs an explicit comparator that
        // compares the underlying unsigned values.
        const auto byteValue = [](std::byte b) { return std::to_integer<unsigned char>(b); };
        const auto [minIt, maxIt] = std::minmax_element(
            tile->data().begin(), tile->data().end(), [&](std::byte a, std::byte b) {
                return byteValue(a) < byteValue(b);
            });
        std::cout << "  min: " << static_cast<int>(byteValue(*minIt))
                  << "  max: " << static_cast<int>(byteValue(*maxIt)) << '\n';
    } else {
        std::cout << "  (numeric min/max skipped: multi-byte samples need the file's byte order, "
                     "which the public API does not currently expose)\n";
    }

    std::cout << "\nOK - full read pipeline succeeded\n";
    return 0;
}
