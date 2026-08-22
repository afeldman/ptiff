/*
 * ptiff_image_bridge.h
 *
 * C ABI over the ptiff::Image domain type and its value types. Image is a
 * move-only PIMPL type, so the bridge owns instances on the heap and hands the
 * Go side opaque handles that are not equal to the C++ object.
 */
#ifndef PTIFF_IMAGE_BRIDGE_H
#define PTIFF_IMAGE_BRIDGE_H

#include <stdint.h>

#include "ptiff_c_export.h"

#ifdef __cplusplus
extern "C" {
#endif

/* ---- PixelType (matches ptiff::PixelType ordering) ---- */
enum ptiff_pixel_type {
    PTIFF_PIXEL_UINT8 = 0,
    PTIFF_PIXEL_UINT16 = 1,
    PTIFF_PIXEL_UINT32 = 2,
    PTIFF_PIXEL_FLOAT32 = 3,
    PTIFF_PIXEL_FLOAT64 = 4,
};

/* ---- CompressionKind (matches ptiff::CompressionKind ordering) ---- */
enum ptiff_compression_kind {
    PTIFF_COMPRESSION_NONE = 0,
    PTIFF_COMPRESSION_LZW = 1,
    PTIFF_COMPRESSION_DEFLATE = 2,
    PTIFF_COMPRESSION_JPEG = 3,
};

/* ---- Public structs (mirror ptiff::ImageDescriptor / TileInfo) ---- */
typedef struct ptiff_tile_info {
    uint32_t tile_width;
    uint32_t tile_height;
} ptiff_tile_info;

typedef struct ptiff_image_descriptor {
    uint32_t width;
    uint32_t height;
    int pixel_type;
    uint32_t channel_count;
    /* groundSampleDistanceMeters, optional */
    int has_gsd;
    double gsd;
    /* tileInfo, optional */
    int has_tile_info;
    ptiff_tile_info tile_info;
    /* compression, optional */
    int has_compression;
    int compression;
} ptiff_image_descriptor;

/* Opaque handle to a heap-owned ptiff::Image. Never dereference from Go. */
typedef struct ptiff_image ptiff_image;

/* Creates a heap Image from a descriptor. Returns NULL on failure. */
PTIFF_C_API ptiff_image* ptiff_image_create(const ptiff_image_descriptor* desc);
/* Frees a handle returned by ptiff_image_create. NULL is a no-op. */
PTIFF_C_API void ptiff_image_destroy(ptiff_image* img);

/* Accessors -- valid for the lifetime of the handle. */
PTIFF_C_API uint32_t ptiff_image_width(const ptiff_image* img);
PTIFF_C_API uint32_t ptiff_image_height(const ptiff_image* img);
PTIFF_C_API int ptiff_image_pixel_type(const ptiff_image* img);
PTIFF_C_API uint32_t ptiff_image_channel_count(const ptiff_image* img);
/* Ground sample distance. Returns 1 and sets *out if present, else 0. */
PTIFF_C_API int ptiff_image_gsd(const ptiff_image* img, double* out);
/* Tile info. Returns 1 and fills *out if present, else 0. */
PTIFF_C_API int ptiff_image_tile_info(const ptiff_image* img, ptiff_tile_info* out);
/* Compression. Returns 1 and sets *out if present, else 0. */
PTIFF_C_API int ptiff_image_compression(const ptiff_image* img, int* out);

#ifdef __cplusplus
}
#endif

#endif /* PTIFF_IMAGE_BRIDGE_H */
