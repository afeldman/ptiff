// examples/copy_local_sample.cpp
//
// Copies a locally-generated PTIFF to a new file, pixel-for-pixel, while
// preserving the camera calibration stored in the ptiff.camera.* extension
// fields. This is the C++ side of the CLI's `copy` behaviour.
//
// Build & run:
//   cmake --build <build-dir> --target ptiff_example_copy_local_sample
//   ./<build-dir>/examples/ptiff_example_copy_local_sample <src.tif> <dst.tif>
//
// Exit codes: 0 = success, 1 = usage/argument error, 2 = read/write error.

#include <cstddef>
#include <cstdint>
#include <iostream>
#include <span>
#include <string>
#include <vector>

#include <ptiff/io/backend/tiff_backend.hpp>
#include <ptiff/io/file_binary_reader.hpp>
#include <ptiff/io/file_binary_writer.hpp>
#include <ptiff/io/storage_model.hpp>
#include <ptiff/io/tile/tile.hpp>
#include <ptiff/io/tile/tile_index.hpp>
#include <ptiff/io/tile/tile_layout.hpp>

using ptiff::io::StorageModel;
using ptiff::io::backend::TiffBackend;
using ptiff::io::tile::Tile;
using ptiff::io::tile::TileIndex;
using ptiff::io::tile::TileLayout;

namespace {

int fail(const ptiff::Error& error) {
    std::cerr << "error: " << error.message() << '\n';
    return 2;
}

// True if the model carries any ptiff.camera.* calibration field.
bool hasCamera(const StorageModel& model) {
    const char* keys[] = {
        "ptiff.camera.model", "ptiff.camera.focal_length_x", "ptiff.camera.principal_x"};
    for (const char* key : keys) {
        if (model.field(key).has_value())
            return true;
    }
    return false;
}

int copySample(const std::string& src, const std::string& dst) {
    TiffBackend backend;

    // ---- read source model + source image ----
    auto srcReader = ptiff::io::FileBinaryReader::open(src);
    if (!srcReader.has_value())
        return fail(srcReader.error());
    auto decoded = backend.deserializeModel(**srcReader);
    if (!decoded.has_value())
        return fail(decoded.error());

    // The TIFF backend returns the image metadata as the scene's first child.
    // Rebuild a flat image model (the shape `openImageSink` expects) by copying
    // the child node's fields through the public `for_each_field` interface.
    const StorageModel& image =
        decoded->children().empty() ? *decoded : decoded->children().front();
    StorageModel model;
    image.for_each_field(
        [&](std::string_view k, std::string_view v) { model.setField(k, std::string{v}); });

    auto pixReader = ptiff::io::FileBinaryReader::open(src);
    if (!pixReader.has_value())
        return fail(pixReader.error());
    auto source = backend.openImageSource(**pixReader);
    if (!source.has_value())
        return fail(source.error());
    const TileLayout& layout = (*source)->layout();

    // The tile geometry is not persisted as a model field by the TIFF backend;
    // re-seed it explicitly so the destination sink tiles exactly like the
    // source.
    model.setField("tileWidth", std::to_string(layout.tileSize.width));
    model.setField("tileHeight", std::to_string(layout.tileSize.height));

    // ---- write destination: same model (keeps camera fields), same tiles ----
    auto writer = ptiff::io::FileBinaryWriter::create(dst);
    if (!writer.has_value())
        return fail(writer.error());
    if (auto r = backend.serializeModel(model, **writer); !r.has_value()) {
        return fail(r.error());
    }
    auto sink = backend.openImageSink(**writer, model);
    if (!sink.has_value())
        return fail(sink.error());

    for (std::uint32_t row = 0; row < layout.rows(); ++row) {
        for (std::uint32_t col = 0; col < layout.columns(); ++col) {
            TileIndex idx{.column = col, .row = row, .level = 0};
            auto tile = (*source)->readTile(idx);
            if (!tile.has_value())
                return fail(tile.error());

            Tile out(ptiff::TileId{0},
                     idx,
                     (*sink)->layout().regionFor(idx).value(),
                     std::span<const std::byte>(tile->data().data(), tile->data().size()));
            if (auto r = (*sink)->writeTile(out); !r.has_value()) {
                return fail(r.error());
            }
        }
    }

    std::cout << "copied " << src << " -> " << dst << " (" << layout.columns() << "x"
              << layout.rows() << " tiles" << (hasCamera(model) ? ", camera preserved" : "")
              << ")\n";
    return 0;
}

} // namespace

int main(int argc, char** argv) {
    if (argc != 3) {
        std::cerr << "usage: " << argv[0] << " <src.tif> <dst.tif>\n";
        return 1;
    }
    return copySample(argv[1], argv[2]);
}
