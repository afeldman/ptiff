#pragma once

/// @file detail/c_abi.hpp
/// @brief Hand-marshalled view of the PTIFF C ABI (`libptiff_c`) used by the
///        wrapper.
///
/// These declarations mirror the cbindgen-generated `target/ptiff_c.h`
/// (produced from `crates/ptiff-c`, whose Rust `extern "C"` signatures are
/// the ABI's source of truth). They are declared here so `ptiff-cpp` is a
/// self-contained wrapper that does not depend on the generated header being
/// present at include time; the struct layout and enum values are pinned by
/// the ABI and must not drift from `ptiff_c.h`.
///
/// Only the slice of the surface the wrapper actually exercises is declared.
/// Every callee-allocated string is released with `ptiff_free_string`;
/// handle-producing functions return `NULL` (with an optional out-param error
/// code / value struct) on failure and are released with their matching
/// `_close`/`_destroy`.

#include <cstdint>

namespace ptiff {
namespace detail {

extern "C" {

// ---- Opaque handles ----
typedef struct ptiff_image ptiff_image;
typedef struct ptiff_source ptiff_source;
typedef struct ptiff_sink ptiff_sink;

// ---- Enums (values pinned by the ABI) ----
typedef enum ptiff_pixel_type {
    PTIFF_PIXEL_UINT8 = 0,
    PTIFF_PIXEL_UINT16 = 1,
    PTIFF_PIXEL_UINT32 = 2,
    PTIFF_PIXEL_FLOAT32 = 3,
    PTIFF_PIXEL_FLOAT64 = 4,
} ptiff_pixel_type;

typedef enum ptiff_compression_kind {
    PTIFF_COMPRESSION_NONE = 0,
    PTIFF_COMPRESSION_LZW = 1,
    PTIFF_COMPRESSION_DEFLATE = 2,
    PTIFF_COMPRESSION_JPEG = 3,
} ptiff_compression_kind;

typedef enum ptiff_error_code {
    PTIFF_ERROR_NOT_IMPLEMENTED = 0,
    PTIFF_ERROR_INVALID_ARGUMENT = 1,
    PTIFF_ERROR_OUT_OF_RANGE = 2,
    PTIFF_ERROR_NOT_FOUND = 3,
    PTIFF_ERROR_UNKNOWN = 4,
} ptiff_error_code;

// ---- Value structs (layout must match ptiff_c.h) ----
typedef struct ptiff_tile_info {
    uint32_t tile_width;
    uint32_t tile_height;
} ptiff_tile_info;

typedef struct ptiff_image_descriptor {
    uint32_t width;
    uint32_t height;
    int32_t pixel_type;
    uint32_t channel_count;
    int32_t has_gsd;
    double gsd;
    int32_t has_tile_info;
    struct ptiff_tile_info tile_info;
    int32_t has_compression;
    int32_t compression;
} ptiff_image_descriptor;

typedef struct ptiff_camera {
    int32_t has_intrinsics;
    double focal_length_x;
    double focal_length_y;
    double principal_x;
    double principal_y;
    double intrinsics[9];
    int32_t has_extrinsics;
    double rotation_w;
    double rotation_x;
    double rotation_y;
    double rotation_z;
    double position_x;
    double position_y;
    double position_z;
    double extrinsics[12];
    double projection[12];
    char timestamp[64];
} ptiff_camera;

typedef struct ptiff_version {
    int32_t major;
    int32_t minor;
    int32_t patch;
} ptiff_version;

typedef struct ptiff_field {
    char* key;
    char* value;
} ptiff_field;

// ---- Functions ----

// version
ptiff_version ptiff_compile_time_version(void);
ptiff_version ptiff_runtime_version(void);

// bridge
char* ptiff_backend_names(void);
void ptiff_free_string(char*);

// image bridge
ptiff_image* ptiff_image_create(const ptiff_image_descriptor* desc);
void ptiff_image_destroy(ptiff_image* img);
uint32_t ptiff_image_width(const ptiff_image* img);
uint32_t ptiff_image_height(const ptiff_image* img);
int32_t ptiff_image_pixel_type(const ptiff_image* img);
uint32_t ptiff_image_channel_count(const ptiff_image* img);
int32_t ptiff_image_gsd(const ptiff_image* img, double* out);
int32_t ptiff_image_tile_info(const ptiff_image* img, ptiff_tile_info* out);
int32_t ptiff_image_compression(const ptiff_image* img, int32_t* out);

// metadata / open
int32_t ptiff_open_path(const char* path, ptiff_image_descriptor* desc);
int32_t ptiff_open_path_camera(const char* path, ptiff_camera* out);
int32_t ptiff_open_path_fields(const char* path, ptiff_field** out, int32_t* out_count);
void ptiff_fields_free(ptiff_field* arr, int32_t count);

// pixel bridge: source (read)
ptiff_source* ptiff_source_open(const char* path, int32_t* err_out);
void ptiff_source_close(ptiff_source* source);
int32_t ptiff_source_descriptor(const ptiff_source* source, ptiff_image_descriptor* desc);
uint32_t ptiff_source_tile_columns(const ptiff_source* source);
uint32_t ptiff_source_tile_rows(const ptiff_source* source);
uintptr_t ptiff_source_tile_byte_size(const ptiff_source* source);
int32_t ptiff_source_read_tile(const ptiff_source* source, uint32_t column, uint32_t row,
                                uint8_t* buffer, uintptr_t buffer_size, uintptr_t* bytes_read);
// pixel bridge: sink (write)
ptiff_sink* ptiff_sink_create(const char* path, const ptiff_image_descriptor* desc);
ptiff_sink* ptiff_sink_create_camera(const char* path, const ptiff_image_descriptor* desc,
                                        const ptiff_camera* camera);
void ptiff_sink_close(ptiff_sink* sink);
uint32_t ptiff_sink_tile_columns(const ptiff_sink* sink);
uint32_t ptiff_sink_tile_rows(const ptiff_sink* sink);
uintptr_t ptiff_sink_tile_byte_size(const ptiff_sink* sink);
int32_t ptiff_sink_write_tile(ptiff_sink* sink, uint32_t column, uint32_t row,
                                const uint8_t* buffer, uintptr_t buffer_size);

} // extern "C"

} // namespace detail
} // namespace ptiff
