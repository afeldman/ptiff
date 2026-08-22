/* Implementation of the ptiff::Image C ABI. */
#include "ptiff_image_bridge.h"

#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <memory>
#include <new>
#include <stdexcept>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#include <ptiff/geometry/camera.hpp>
#include <ptiff/image.hpp>
#include <ptiff/io/backend/tiff_backend.hpp>
#include <ptiff/io/file_binary_reader.hpp>
#include <ptiff/io/image_source.hpp>
#include <ptiff/io/storage_model.hpp>
#include <ptiff/io/tile/tile_layout.hpp>

#include "ptiff_camera.h" /* ptiff_open_path_camera / ptiff_camera */
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

static int to_c_error(ptiff::ErrorCode code) {
    return -static_cast<int>(code);
}

static_assert(static_cast<int>(ptiff::PixelType::UInt8) == PTIFF_PIXEL_UINT8);
static_assert(static_cast<int>(ptiff::PixelType::UInt16) == PTIFF_PIXEL_UINT16);
static_assert(static_cast<int>(ptiff::PixelType::UInt32) == PTIFF_PIXEL_UINT32);
static_assert(static_cast<int>(ptiff::PixelType::Float32) == PTIFF_PIXEL_FLOAT32);
static_assert(static_cast<int>(ptiff::PixelType::Float64) == PTIFF_PIXEL_FLOAT64);

static_assert(static_cast<int>(ptiff::CompressionKind::None) == PTIFF_COMPRESSION_NONE);
static_assert(static_cast<int>(ptiff::CompressionKind::Lzw) == PTIFF_COMPRESSION_LZW);
static_assert(static_cast<int>(ptiff::CompressionKind::Deflate) == PTIFF_COMPRESSION_DEFLATE);
static_assert(static_cast<int>(ptiff::CompressionKind::Jpeg) == PTIFF_COMPRESSION_JPEG);

ptiff_image* ptiff_image_create(const ptiff_image_descriptor* desc) {
    if (desc == nullptr) {
        return nullptr;
    }

    ptiff::ImageDescriptor d;
    d.width = desc->width;
    d.height = desc->height;
    d.pixelType = static_cast<ptiff::PixelType>(desc->pixel_type);
    d.channelCount = desc->channel_count;
    if (desc->has_gsd) {
        d.groundSampleDistanceMeters = desc->gsd;
    }
    if (desc->has_tile_info) {
        d.tileInfo = ptiff::TileInfo{desc->tile_info.tile_width, desc->tile_info.tile_height};
    }
    if (desc->has_compression) {
        d.compression = static_cast<ptiff::CompressionKind>(desc->compression);
    }

    // Image is move-only, so heap-allocate it and hand back the raw pointer;
    // the Go side never touches the pointee directly.
    return reinterpret_cast<ptiff_image*>(new (std::nothrow) ptiff::Image(std::move(d)));
}

void ptiff_image_destroy(ptiff_image* img) {
    delete reinterpret_cast<ptiff::Image*>(img);
}

uint32_t ptiff_image_width(const ptiff_image* img) {
    const auto* i = reinterpret_cast<const ptiff::Image*>(img);
    return i->width();
}

uint32_t ptiff_image_height(const ptiff_image* img) {
    const auto* i = reinterpret_cast<const ptiff::Image*>(img);
    return i->height();
}

int ptiff_image_pixel_type(const ptiff_image* img) {
    const auto* i = reinterpret_cast<const ptiff::Image*>(img);
    return static_cast<int>(i->pixelType());
}

uint32_t ptiff_image_channel_count(const ptiff_image* img) {
    const auto* i = reinterpret_cast<const ptiff::Image*>(img);
    return i->channelCount();
}

int ptiff_image_gsd(const ptiff_image* img, double* out) {
    const auto* i = reinterpret_cast<const ptiff::Image*>(img);
    auto v = i->groundSampleDistanceMeters();
    if (!v.has_value()) {
        return 0;
    }
    if (out != nullptr) {
        *out = *v;
    }
    return 1;
}

int ptiff_image_tile_info(const ptiff_image* img, ptiff_tile_info* out) {
    const auto* i = reinterpret_cast<const ptiff::Image*>(img);
    auto t = i->tileInfo();
    if (!t.has_value()) {
        return 0;
    }
    if (out != nullptr) {
        out->tile_width = t->tileWidth;
        out->tile_height = t->tileHeight;
    }
    return 1;
}

int ptiff_image_compression(const ptiff_image* img, int* out) {
    const auto* i = reinterpret_cast<const ptiff::Image*>(img);
    auto c = i->compression();
    if (!c.has_value()) {
        return 0;
    }
    if (out != nullptr) {
        *out = static_cast<int>(*c);
    }
    return 1;
}

namespace {} // namespace

int ptiff_open_path_camera(const char* path, ptiff_camera* out) {
    if (path == nullptr || out == nullptr) {
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

    // Helper: read a single ptiff.camera.* field value ("" when absent).
    auto field = [&](std::string_view name) -> std::string {
        auto v = image.field(std::string("ptiff.camera.").append(name));
        return v.has_value() ? *v : std::string{};
    };
    auto parse_double = [](const std::string& s, double& dst) mutable -> bool {
        if (s.empty()) {
            return false;
        }
        try {
            dst = std::stod(s);
            return true;
        } catch (const std::exception&) {
            return false;
        }
    };

    ptiff_camera cam{};
    std::memset(&cam, 0, sizeof(cam));

    // Intrinsics.
    double fx = 0.0, fy = 0.0, cx = 0.0, cy = 0.0;
    bool hasFx = parse_double(field("focal_length_x"), fx);
    bool hasFy = parse_double(field("focal_length_y"), fy);
    bool hasCx = parse_double(field("principal_x"), cx);
    bool hasCy = parse_double(field("principal_y"), cy);
    if (hasFx && hasFy && hasCx && hasCy) {
        cam.has_intrinsics = 1;
        cam.focal_length_x = fx;
        cam.focal_length_y = fy;
        cam.principal_x = cx;
        cam.principal_y = cy;
        // K = [ fx  0  cx ]
        //     [  0 fy  cy ]
        //     [  0  0   1 ]
        cam.intrinsics[0] = fx;
        cam.intrinsics[4] = fy;
        cam.intrinsics[2] = cx;
        cam.intrinsics[5] = cy;
        cam.intrinsics[8] = 1.0;
    }

    // Extrinsics (rotation quaternion + world position).
    double rw = 1.0, rx = 0.0, ry = 0.0, rz = 0.0;
    double px = 0.0, py = 0.0, pz = 0.0;
    bool hasRot = parse_double(field("rotation_w"), rw) && parse_double(field("rotation_x"), rx) &&
                  parse_double(field("rotation_y"), ry) && parse_double(field("rotation_z"), rz);
    bool hasPos = parse_double(field("position_x"), px) && parse_double(field("position_y"), py) &&
                  parse_double(field("position_z"), pz);
    bool hasExt = hasRot || hasPos;

    // Build the derived matrices via the C++ Camera model so the projection
    // P = K * [R | t] and the individual matrices stay in one authoritative place.
    ptiff::Camera camera(
        field("model"),
        ptiff::Intrinsics{fx, fy, cx, cy},
        ptiff::Extrinsics{ptiff::Quaternion{rw, rx, ry, rz}, ptiff::Vec3{px, py, pz}},
        field("timestamp"));
    std::memcpy(cam.intrinsics, camera.intrinsicsMatrix().data(), sizeof(cam.intrinsics));
    std::memcpy(cam.extrinsics, camera.extrinsicsMatrix().data(), sizeof(cam.extrinsics));
    std::memcpy(cam.projection, camera.projectionMatrix().data(), sizeof(cam.projection));

    if (hasExt) {
        cam.has_extrinsics = 1;
        cam.rotation_w = rw;
        cam.rotation_x = rx;
        cam.rotation_y = ry;
        cam.rotation_z = rz;
        cam.position_x = px;
        cam.position_y = py;
        cam.position_z = pz;
    }

    // Timestamp (truncate safely into the fixed buffer).
    const std::string ts = field("timestamp");
    std::memset(cam.timestamp, 0, sizeof(cam.timestamp));
    if (!ts.empty()) {
        std::snprintf(cam.timestamp, sizeof(cam.timestamp), "%s", ts.c_str());
    }

    *out = cam;
    return 0;
}

} // extern "C"
