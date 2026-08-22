// examples/read_local_sample.cpp
//
// Opens any locally-generated PTIFF (e.g. the output of write_local_sample) and
// reports the format-neutral StorageModel metadata, the pinhole camera
// calibration stored in the ptiff.camera.* fields, and the first tile of pixel
// data. This is the same public API a binding (Python/Rust/Go) or a CLI tool
// uses to inspect a TIFF document.
//
// Build & run:
//   cmake --build <build-dir> --target ptiff_example_read_local_sample
//   ./<build-dir>/examples/ptiff_example_read_local_sample <path-to-ptiff.tif>
//
// Exit codes: 0 = success, 1 = usage/argument error, 2 = read/metadata error.

#include <algorithm>
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <iostream>
#include <string>

#include <ptiff/io/backend/tiff_backend.hpp>
#include <ptiff/io/file_binary_reader.hpp>
#include <ptiff/io/storage_model.hpp>
#include <ptiff/io/tile/tile.hpp>

using ptiff::io::StorageModel;
using ptiff::io::backend::TiffBackend;
using ptiff::io::tile::TileIndex;

namespace {

int fail(const ptiff::Error& error) {
    std::cerr << "error: " << error.message() << '\n';
    return 2;
}

// Prints a single option string field if it is present.
void printField(const StorageModel& model, const std::string& key) {
    if (const auto v = model.field(key); v.has_value()) {
        std::cout << "    " << key << " = " << *v << '\n';
    }
}

// Prints the camera calibration block from the ptiff.camera.* fields.
void printCamera(const StorageModel& model) {
    std::cout << "  camera:\n";
    bool any = false;
    // Record are persisted as flat "ptiff.camera.*" fields; enumerate subset.
    // (Exact key list matches what write_local_sample emits.)
    const char* keys[] = {
        "ptiff.camera.model",
        "ptiff.camera.focal_length_x",
        "ptiff.camera.focal_length_y",
        "ptiff.camera.principal_x",
        "ptiff.camera.principal_y",
        "ptiff.camera.rotation_w",
        "ptiff.camera.rotation_x",
        "ptiff.camera.rotation_y",
        "ptiff.camera.rotation_z",
        "ptiff.camera.position_x",
        "ptiff.camera.position_y",
        "ptiff.camera.position_z",
        "ptiff.camera.timestamp",
    };
    for (const char* key : keys) {
        if (const auto v = model.field(key); v.has_value()) {
            std::cout << "      " << key << " = " << *v << '\n';
            any = true;
        }
    }
    if (!any) {
        std::cout << "      (no camera calibration present)\n";
    }
}

int readSample(const std::string& path) {
    TiffBackend backend;

    auto metadataReader = ptiff::io::FileBinaryReader::open(path);
    if (!metadataReader.has_value()) {
        return fail(metadataReader.error());
    }
    auto model = backend.deserializeModel(**metadataReader);
    if (!model.has_value()) {
        return fail(model.error());
    }

    // The TIFF backend returns the image metadata under the scene's first
    // child (see `deserializeModel`); fall back to the root for safety.
    const StorageModel& image = model->children().empty() ? *model : model->children().front();

    std::cout << "PTIFF local sample reader (libptiff)\n";
    std::cout << "  file: " << path << '\n';
    std::cout << "\n[file]\n";
    printField(image, "imageWidth");
    printField(image, "imageHeight");
    printField(image, "pixelType");
    printField(image, "compression");
    printField(image, "predictor");
    printField(image, "samplesPerPixel");

    std::cout << "\n[camera]\n";
    printCamera(image);

    std::cout << "\n[pixels]\n";
    auto pixelReader = ptiff::io::FileBinaryReader::open(path);
    if (!pixelReader.has_value()) {
        return fail(pixelReader.error());
    }
    auto source = backend.openImageSource(**pixelReader);
    if (!source.has_value()) {
        return fail(source.error());
    }

    auto tile = (*source)->readTile(TileIndex{.column = 0, .row = 0, .level = 0});
    if (!tile.has_value()) {
        return fail(tile.error());
    }
    std::cout << "  first tile bytes: " << tile->data().size() << '\n';

    if (image.field("pixelType").value_or("") == "UInt8" && !tile->data().empty()) {
        const auto byteValue = [](std::byte b) { return std::to_integer<unsigned char>(b); };
        std::uint64_t sum = 0;
        unsigned char lo = 255, hi = 0;
        for (const std::byte b : tile->data()) {
            const auto v = byteValue(b);
            sum += v;
            lo = std::min(lo, v);
            hi = std::max(hi, v);
        }
        std::cout << "  tile min: " << static_cast<int>(lo) << "  max: " << static_cast<int>(hi)
                  << "  sum: " << sum << '\n';
    } else {
        std::cout << "  (numeric stats skipped: multi-byte samples need byte order)\n";
    }

    std::cout << "\nOK - read pipeline succeeded\n";
    return 0;
}

} // namespace

int main(int argc, char** argv) {
    if (argc != 2) {
        std::cerr << "usage: " << argv[0] << " <path-to-ptiff.tif>\n";
        return 1;
    }
    return readSample(argv[1]);
}
