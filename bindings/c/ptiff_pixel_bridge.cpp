/* Implementation of the pixel-tile-read C ABI (ptiff_source). */
#include "ptiff_pixel_bridge.h"

#include <cstddef>
#include <cstdio>
#include <cstring>
#include <memory>
#include <new>
#include <span>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#include <ptiff/image/pixel_type.hpp>
#include <ptiff/io/backend/tiff_backend.hpp>
#include <ptiff/io/file_binary_reader.hpp>
#include <ptiff/io/file_binary_writer.hpp>
#include <ptiff/io/image_sink.hpp>
#include <ptiff/io/image_source.hpp>
#include <ptiff/io/storage_model.hpp>
#include <ptiff/io/tile/tile.hpp>
#include <ptiff/io/tile/tile_index.hpp>
#include <ptiff/io/tile/tile_layout.hpp>
#include <ptiff/io/tile/tile_region.hpp>

#include "ptiff_camera.h" /* ptiff_camera (via ptiff_sink_create_camera) */
#include "ptiff_error.h"  /* PTIFF_ERROR_* for negative rc. */

namespace {

// deserializeModel returns either a flat per-image model directly, or (since the multi-image IFD
// chain support landed) a root model whose first child carries the image fields. Normalize both
// to the flat image model -- same idiom as Pds4Backend's imageModel() helper. Kept outside
// extern "C" (unlike the bridge functions below): it returns a C++ reference type, which is not
// C-linkage-compatible, and it's an internal implementation detail, never part of the public ABI.
const ptiff::io::StorageModel& imageModel(const ptiff::io::StorageModel& model) noexcept {
    return model.children().empty() ? model : model.children().front();
}

} // namespace

extern "C" {

namespace {

// Internal per-TU helpers: marked 'static' so they keep internal linkage even
// though they sit inside an extern "C" block. Without it the anonymous
// namespace does not give them internal linkage under the C language linkage
// specified by the enclosing block, and both this TU and ptiff_image_bridge.cpp
// would export the same symbol (duplicate-symbol link error for shared libs).
static int to_c_error(ptiff::ErrorCode code) {
    return -static_cast<int>(code);
}

static int pixel_type_from_field(std::string_view v) noexcept {
    if (v == "UInt16")
        return PTIFF_PIXEL_UINT16;
    if (v == "UInt32")
        return PTIFF_PIXEL_UINT32;
    if (v == "Float32")
        return PTIFF_PIXEL_FLOAT32;
    if (v == "Float64")
        return PTIFF_PIXEL_FLOAT64;
    return PTIFF_PIXEL_UINT8;
}

static std::size_t bytes_per_sample(int pixel_type) noexcept {
    switch (pixel_type) {
    case PTIFF_PIXEL_UINT8:
        return 1;
    case PTIFF_PIXEL_UINT16:
        return 2;
    case PTIFF_PIXEL_UINT32:
        return 4;
    case PTIFF_PIXEL_FLOAT32:
        return 4;
    case PTIFF_PIXEL_FLOAT64:
        return 8;
    default:
        return 0;
    }
}

} // namespace

struct ptiff_source {
    std::unique_ptr<ptiff::io::FileBinaryReader> reader;
    std::unique_ptr<ptiff::io::ImageSource> source;
    ptiff_image_descriptor descriptor;
};

ptiff_source* ptiff_source_open(const char* path, int* err_out) {
    if (path == nullptr) {
        if (err_out != nullptr) {
            *err_out = -PTIFF_ERROR_INVALID_ARGUMENT;
        }
        return nullptr;
    }

    auto readerResult = ptiff::io::FileBinaryReader::open(path);
    if (!readerResult.has_value()) {
        if (err_out != nullptr) {
            *err_out = to_c_error(readerResult.error().code());
        }
        return nullptr;
    }
    auto reader = std::move(*readerResult);

    const ptiff::io::backend::TiffBackend backend;
    auto model = backend.deserializeModel(*reader);
    if (!model.has_value()) {
        if (err_out != nullptr) {
            *err_out = to_c_error(model.error().code());
        }
        return nullptr;
    }

    const auto& image = imageModel(*model);
    auto width = image.field("imageWidth");
    auto height = image.field("imageHeight");
    auto samples = image.field("samplesPerPixel");
    auto pixelType = image.field("pixelType");
    if (!width.has_value() || !height.has_value() || !samples.has_value() ||
        !pixelType.has_value()) {
        if (err_out != nullptr) {
            *err_out = -PTIFF_ERROR_UNKNOWN;
        }
        return nullptr;
    }

    // deserializeModel left the reader positioned past the header/IFD; openImageSource re-parses
    // the directory from the reader's current position, so rewind first.
    auto seekResult = reader->seek(0);
    if (!seekResult.has_value()) {
        if (err_out != nullptr) {
            *err_out = to_c_error(seekResult.error().code());
        }
        return nullptr;
    }
    auto imageSource = backend.openImageSource(*reader);
    if (!imageSource.has_value()) {
        if (err_out != nullptr) {
            *err_out = to_c_error(imageSource.error().code());
        }
        return nullptr;
    }

    ptiff_image_descriptor desc{};
    desc.width = static_cast<uint32_t>(std::stoul(width->c_str()));
    desc.height = static_cast<uint32_t>(std::stoul(height->c_str()));
    desc.channel_count = static_cast<uint32_t>(std::stoul(samples->c_str()));
    desc.pixel_type = pixel_type_from_field(*pixelType);
    desc.has_gsd = 0;
    desc.gsd = 0.0;
    const auto& layout = (*imageSource)->layout();
    desc.has_tile_info = 1;
    desc.tile_info.tile_width = layout.tileSize.width;
    desc.tile_info.tile_height = layout.tileSize.height;
    desc.has_compression = 0;
    desc.compression = 0;

    auto* result =
        new (std::nothrow) ptiff_source{std::move(reader), std::move(*imageSource), desc};
    if (result == nullptr) {
        if (err_out != nullptr) {
            *err_out = -PTIFF_ERROR_UNKNOWN;
        }
        return nullptr;
    }
    return result;
}

void ptiff_source_close(ptiff_source* source) {
    delete source;
}

int ptiff_source_descriptor(const ptiff_source* source, ptiff_image_descriptor* desc) {
    if (source == nullptr || desc == nullptr) {
        return -PTIFF_ERROR_INVALID_ARGUMENT;
    }
    *desc = source->descriptor;
    return 0;
}

uint32_t ptiff_source_tile_columns(const ptiff_source* source) {
    if (source == nullptr) {
        return 0;
    }
    return source->source->layout().columns();
}

uint32_t ptiff_source_tile_rows(const ptiff_source* source) {
    if (source == nullptr) {
        return 0;
    }
    return source->source->layout().rows();
}

size_t ptiff_source_tile_byte_size(const ptiff_source* source) {
    if (source == nullptr) {
        return 0;
    }
    const auto& d = source->descriptor;
    const std::size_t tileWidth = d.has_tile_info ? d.tile_info.tile_width : d.width;
    const std::size_t tileHeight = d.has_tile_info ? d.tile_info.tile_height : d.height;
    return tileWidth * tileHeight * static_cast<std::size_t>(d.channel_count) *
           bytes_per_sample(d.pixel_type);
}

int ptiff_source_read_tile(ptiff_source* source,
                           uint32_t column,
                           uint32_t row,
                           uint8_t* buffer,
                           size_t buffer_size,
                           size_t* bytes_read) {
    if (source == nullptr || buffer == nullptr || bytes_read == nullptr) {
        return -PTIFF_ERROR_INVALID_ARGUMENT;
    }

    auto tile = source->source->readTile(
        ptiff::io::tile::TileIndex{.column = column, .row = row, .level = 0});
    if (!tile.has_value()) {
        return to_c_error(tile.error().code());
    }

    const auto data = tile->data();
    if (data.size() > buffer_size) {
        return -PTIFF_ERROR_INVALID_ARGUMENT;
    }
    std::memcpy(buffer, data.data(), data.size());
    *bytes_read = data.size();
    return 0;
}

struct ptiff_sink {
    std::unique_ptr<ptiff::io::FileBinaryWriter> writer;
    std::unique_ptr<ptiff::io::ImageSink> sink;
    std::vector<ptiff::io::StorageModel> models; // owns the model tree (children)
    ptiff_image_descriptor descriptor;
};

// Builds the per-image StorageModel the TIFF backend needs, from a C descriptor. Mirrors the
// exact field vocabulary SceneSerializer uses (scene_serializer.cpp) so serializeModelList /
// openImageSinkAt produce the same layout a Reader/Writer round-trip would.
static ptiff::io::StorageModel makeStorageModel(const ptiff_image_descriptor& d) {
    ptiff::io::StorageModel child;
    child.setField("imageWidth", std::to_string(d.width));
    child.setField("imageHeight", std::to_string(d.height));
    child.setField("samplesPerPixel", std::to_string(d.channel_count));
    switch (d.pixel_type) {
    case PTIFF_PIXEL_UINT8:
        child.setField("pixelType", "UInt8");
        break;
    case PTIFF_PIXEL_UINT16:
        child.setField("pixelType", "UInt16");
        break;
    case PTIFF_PIXEL_UINT32:
        child.setField("pixelType", "UInt32");
        break;
    case PTIFF_PIXEL_FLOAT32:
        child.setField("pixelType", "Float32");
        break;
    default:
        child.setField("pixelType", "UInt8");
        break;
    }
    if (d.has_compression) {
        switch (d.compression) {
        case PTIFF_COMPRESSION_LZW:
            child.setField("compression", "LZW");
            break;
        case PTIFF_COMPRESSION_DEFLATE:
            child.setField("compression", "Deflate");
            break;
        case PTIFF_COMPRESSION_JPEG:
            child.setField("compression", "Jpeg");
            break;
        default:
            child.setField("compression", "None");
            break;
        }
    } else {
        child.setField("compression", "None");
    }
    if (d.has_tile_info) {
        child.setField("tileWidth", std::to_string(d.tile_info.tile_width));
        child.setField("tileHeight", std::to_string(d.tile_info.tile_height));
    }
    return child;
}

// Applies the structured camera calibration onto a writable StorageModel as `ptiff.camera.*`
// metadata fields. `camera` must be non-NULL. Only the intrinsics (fx/fy/cx/cy), the extrinsic
// rotation quaternion + world translation, the camera model and the ISO-8601 timestamp are
// written; all four intrinsics must be present for the calibration to be considered complete
// (mirrors the read side in ptiff_image_bridge.cpp's ptiff_open_path_camera). Matrices are NOT
// stored as fields -- they are derived by the C++ Camera model on read.
static void applyCameraFields(ptiff::io::StorageModel& child, const ptiff_camera* camera) {
    if (camera->has_intrinsics) {
        char buf[64];
        std::snprintf(buf, sizeof(buf), "%g", camera->focal_length_x);
        child.setField("ptiff.camera.focal_length_x", buf);
        std::snprintf(buf, sizeof(buf), "%g", camera->focal_length_y);
        child.setField("ptiff.camera.focal_length_y", buf);
        std::snprintf(buf, sizeof(buf), "%g", camera->principal_x);
        child.setField("ptiff.camera.principal_x", buf);
        std::snprintf(buf, sizeof(buf), "%g", camera->principal_y);
        child.setField("ptiff.camera.principal_y", buf);
    }
    if (camera->has_extrinsics) {
        char buf[64];
        std::snprintf(buf, sizeof(buf), "%g", camera->rotation_w);
        child.setField("ptiff.camera.rotation_w", buf);
        std::snprintf(buf, sizeof(buf), "%g", camera->rotation_x);
        child.setField("ptiff.camera.rotation_x", buf);
        std::snprintf(buf, sizeof(buf), "%g", camera->rotation_y);
        child.setField("ptiff.camera.rotation_y", buf);
        std::snprintf(buf, sizeof(buf), "%g", camera->rotation_z);
        child.setField("ptiff.camera.rotation_z", buf);
        std::snprintf(buf, sizeof(buf), "%g", camera->position_x);
        child.setField("ptiff.camera.position_x", buf);
        std::snprintf(buf, sizeof(buf), "%g", camera->position_y);
        child.setField("ptiff.camera.position_y", buf);
        std::snprintf(buf, sizeof(buf), "%g", camera->position_z);
        child.setField("ptiff.camera.position_z", buf);
    }
    if (camera->timestamp[0] != '\0') {
        child.setField("ptiff.camera.timestamp", camera->timestamp);
    }
    // The camera model identifier is stored as-is (always present).
    child.setField("ptiff.camera.model", "pinhole");
}

// Shared sink-creation core: builds the per-image StorageModel (with optional camera metadata)
// and opens the file / image sink. Returns the heap ptiff_sink, or nullptr on failure. `camera`
// may be nullptr (no camera metadata).
static ptiff_sink*
createSinkImpl(const char* path, const ptiff_image_descriptor* desc, const ptiff_camera* camera) {
    // Validate pointers + what the TIFF backend cannot represent.
    if (path == nullptr || desc == nullptr) {
        return nullptr;
    }
    if (desc->width == 0 || desc->height == 0 || desc->channel_count == 0 ||
        desc->channel_count > 3 || desc->pixel_type < PTIFF_PIXEL_UINT8 ||
        desc->pixel_type > PTIFF_PIXEL_FLOAT32 || !desc->has_tile_info ||
        desc->tile_info.tile_width == 0 || desc->tile_info.tile_height == 0) {
        return nullptr;
    }

    auto writerResult = ptiff::io::FileBinaryWriter::create(std::string{path});
    if (!writerResult.has_value()) {
        return nullptr;
    }
    auto writer = std::move(*writerResult);

    ptiff::io::StorageModel child = makeStorageModel(*desc);
    if (camera != nullptr) {
        applyCameraFields(child, camera);
    }
    std::vector<ptiff::io::StorageModel> models;
    models.push_back(std::move(child));

    const ptiff::io::backend::TiffBackend backend;
    auto serializeResult = backend.serializeModelList(models, *writer);
    if (!serializeResult.has_value()) {
        return nullptr;
    }
    auto sinkResult = backend.openImageSinkAt(*writer, models, 0);
    if (!sinkResult.has_value()) {
        return nullptr;
    }

    auto* result = new (std::nothrow)
        ptiff_sink{std::move(writer), std::move(*sinkResult), std::move(models), *desc};
    if (result == nullptr) {
        return nullptr;
    }
    return result;
}

ptiff_sink* ptiff_sink_create(const char* path, const ptiff_image_descriptor* desc) {
    return createSinkImpl(path, desc, nullptr);
}

ptiff_sink* ptiff_sink_create_camera(const char* path,
                                     const ptiff_image_descriptor* desc,
                                     const ptiff_camera* camera) {
    if (camera == nullptr) {
        return nullptr;
    }
    return createSinkImpl(path, desc, camera);
}

uint32_t ptiff_sink_tile_columns(const ptiff_sink* sink) {
    if (sink == nullptr) {
        return 0;
    }
    return sink->sink->layout().columns();
}

uint32_t ptiff_sink_tile_rows(const ptiff_sink* sink) {
    if (sink == nullptr) {
        return 0;
    }
    return sink->sink->layout().rows();
}

size_t ptiff_sink_tile_byte_size(const ptiff_sink* sink) {
    if (sink == nullptr) {
        return 0;
    }
    const auto& d = sink->descriptor;
    return static_cast<std::size_t>(d.tile_info.tile_width) *
           static_cast<std::size_t>(d.tile_info.tile_height) *
           static_cast<std::size_t>(d.channel_count) * bytes_per_sample(d.pixel_type);
}

int ptiff_sink_write_tile(
    ptiff_sink* sink, uint32_t column, uint32_t row, const uint8_t* buffer, size_t buffer_size) {
    if (sink == nullptr || buffer == nullptr) {
        return -PTIFF_ERROR_INVALID_ARGUMENT;
    }
    const std::size_t byteSize = ptiff_sink_tile_byte_size(sink);
    if (buffer_size != byteSize) {
        return -PTIFF_ERROR_INVALID_ARGUMENT;
    }

    const auto& layout = sink->sink->layout();
    if (column >= layout.columns() || row >= layout.rows()) {
        return -PTIFF_ERROR_OUT_OF_RANGE;
    }

    ptiff::io::tile::TileIndex index{.column = column, .row = row, .level = 0};
    auto region = layout.regionFor(index);
    if (!region.has_value()) {
        return -PTIFF_ERROR_OUT_OF_RANGE;
    }

    ptiff::io::tile::Tile tile(
        ptiff::TileId{static_cast<std::uint64_t>(row) * layout.columns() + column},
        index,
        *region,
        std::span<const std::byte>{reinterpret_cast<const std::byte*>(buffer), buffer_size});
    auto result = sink->sink->writeTile(tile);
    if (!result.has_value()) {
        return to_c_error(result.error().code());
    }
    return 0;
}

void ptiff_sink_close(ptiff_sink* sink) {
    if (sink == nullptr) {
        return;
    }
    (void)sink->writer->flush();
    delete sink;
}

} // extern "C"
