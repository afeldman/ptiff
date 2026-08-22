/* Implementation of the ptiff::Metadata C ABI (bindings/c/ptiff_metadata.h):
 * read a file's metadata -- the core image descriptor and the flattened PTIFF
 * extension fields -- without materialising a live ptiff::Image handle. */
#include "ptiff_metadata.h"

#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <string>
#include <string_view>
#include <vector>

#include <ptiff/io/backend/tiff_backend.hpp>
#include <ptiff/io/file_binary_reader.hpp>
#include <ptiff/io/image_source.hpp>
#include <ptiff/io/storage_model.hpp>

#include "ptiff_error.h" /* PTIFF_ERROR_* for negative rc. */

namespace {

// deserializeModel returns either a flat per-image model directly, or (since the multi-image IFD
// chain support landed) a root model whose first child carries the image fields. Normalize both
// to the flat image model -- same idiom as Pds4Backend's imageModel() helper. Never part of the
// public ABI.
const ptiff::io::StorageModel& imageModel(const ptiff::io::StorageModel& model) noexcept {
    return model.children().empty() ? model : model.children().front();
}

/// Maps the StorageModel pixelType field back to the C pixel-type enum. The
/// TIFF backend emits "UInt8"/"UInt16"/"UInt32"/"Float32"/"Float64"; unknown
/// values fall back to UInt8 (shouldn't happen for the supported baseline).
/// Marked 'static' for internal linkage (see to_c_error below): inside an
/// extern "C" block an anonymous namespace alone does not hide the symbol, and
/// other bridge .cpp files define the same helper.
static int pixel_type_from_field(std::string_view v) noexcept {
    if (v == "UInt16") {
        return PTIFF_PIXEL_UINT16;
    }
    if (v == "UInt32") {
        return PTIFF_PIXEL_UINT32;
    }
    if (v == "Float32") {
        return PTIFF_PIXEL_FLOAT32;
    }
    if (v == "Float64") {
        return PTIFF_PIXEL_FLOAT64;
    }
    return PTIFF_PIXEL_UINT8;
}

} // namespace

extern "C" {

static int to_c_error(ptiff::ErrorCode code) {
    return -static_cast<int>(code);
}

int ptiff_open_path(const char* path, ptiff_image_descriptor* desc) {
    if (path == nullptr || desc == nullptr) {
        return -PTIFF_ERROR_INVALID_ARGUMENT;
    }

    auto reader = ptiff::io::FileBinaryReader::open(path);
    if (!reader.has_value()) {
        return to_c_error(reader.error().code());
    }

    const ptiff::io::backend::TiffBackend backend;

    // Primary metadata via the format-neutral StorageModel: width, height,
    // samples-per-pixel, pixel type.
    auto model = backend.deserializeModel(**reader);
    if (!model.has_value()) {
        return to_c_error(model.error().code());
    }

    const auto& image = imageModel(*model);
    auto width = image.field("imageWidth");
    auto height = image.field("imageHeight");
    auto samples = image.field("samplesPerPixel");
    auto pixelType = image.field("pixelType");
    if (!width.has_value() || !height.has_value() || !samples.has_value() ||
        !pixelType.has_value()) {
        return -PTIFF_ERROR_UNKNOWN;
    }

    // Tile info via the tile layout (best-effort; struct is fully populated
    // before this point, so a failure here can leave tile info unset).
    bool hasTileInfo = false;
    uint32_t tileWidth = 0;
    uint32_t tileHeight = 0;
    {
        // Re-read the directory from the start for the tile layout.
        (void)(*reader)->seek(0);
        auto source = backend.openImageSource(**reader);
        if (source.has_value()) {
            const auto& layout = (*source)->layout();
            tileWidth = layout.tileSize.width;
            tileHeight = layout.tileSize.height;
            hasTileInfo = true;
        }
    }

    ptiff_image_descriptor out{};
    out.width = static_cast<uint32_t>(std::stoul(width->c_str()));
    out.height = static_cast<uint32_t>(std::stoul(height->c_str()));
    out.channel_count = static_cast<uint32_t>(std::stoul(samples->c_str()));
    out.pixel_type = pixel_type_from_field(*pixelType);
    out.has_gsd = 0;
    out.gsd = 0.0;
    out.has_tile_info = hasTileInfo ? 1 : 0;
    out.tile_info.tile_width = tileWidth;
    out.tile_info.tile_height = tileHeight;
    out.has_compression = 0;
    out.compression = 0;

    *desc = out;
    return 0;
}

int ptiff_open_path_fields(const char* path, ptiff_field** out, int* out_count) {
    if (path == nullptr || out == nullptr || out_count == nullptr) {
        return -PTIFF_ERROR_INVALID_ARGUMENT;
    }

    auto reader = ptiff::io::FileBinaryReader::open(path);
    if (!reader.has_value()) {
        return to_c_error(reader.error().code());
    }

    const ptiff::io::backend::TiffBackend backend;
    auto model = backend.deserializeModel(**reader);
    if (!model.has_value()) {
        return to_c_error(model.error().code());
    }

    const auto& image = imageModel(*model);

    // Collect the flattened PTIFF extension fields (keys `ptiff.<domain>.<name>`,
    // lexicographic order as iterated by the StorageModel's field map).
    std::vector<ptiff_field> fields;
    image.for_each_field([&](std::string_view key, std::string_view value) {
        if (key.substr(0, 6) == "ptiff.") {
            fields.push_back(ptiff_field{nullptr, nullptr});
            auto& f = fields.back();
            f.key = static_cast<char*>(std::malloc(key.size() + 1));
            f.value = static_cast<char*>(std::malloc(value.size() + 1));
            if (f.key != nullptr && f.value != nullptr) {
                std::memcpy(f.key, key.data(), key.size());
                f.key[key.size()] = '\0';
                std::memcpy(f.value, value.data(), value.size());
                f.value[value.size()] = '\0';
            } else {
                // Allocation failure: leave the pair null-terminated-freeable.
                std::free(f.key);
                std::free(f.value);
                f.key = nullptr;
                f.value = nullptr;
            }
        }
    });

    if (fields.empty()) {
        *out = nullptr;
        *out_count = 0;
        return 0;
    }

    ptiff_field* arr = static_cast<ptiff_field*>(std::malloc(fields.size() * sizeof(ptiff_field)));
    if (arr == nullptr) {
        // Mirrors ptiff_fields_free; release the per-field strings already made.
        for (auto& f : fields) {
            std::free(f.key);
            std::free(f.value);
        }
        return -PTIFF_ERROR_UNKNOWN;
    }
    std::memcpy(arr, fields.data(), fields.size() * sizeof(ptiff_field));
    *out = arr;
    *out_count = static_cast<int>(fields.size());
    return 0;
}

void ptiff_fields_free(ptiff_field* arr, int count) {
    if (arr == nullptr) {
        return;
    }
    for (int i = 0; i < count; ++i) {
        std::free(arr[i].key);
        std::free(arr[i].value);
    }
    std::free(arr);
}

} // extern "C"
