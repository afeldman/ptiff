/*
 * ptiff_pixel_bridge.h
 *
 * C ABI for reading an open image's actual pixel data, tile by tile. Complements
 * ptiff_image_bridge.h's metadata-only ptiff_open_path: a ptiff_source is a live handle over an
 * open file (owns the reader), positioned at its primary image, from which tiles can be read
 * repeatedly without re-parsing the file each time.
 */
#ifndef PTIFF_PIXEL_BRIDGE_H
#define PTIFF_PIXEL_BRIDGE_H

#include <stddef.h>
#include <stdint.h>

#include "ptiff_c_export.h"
#include "ptiff_camera.h"       /* ptiff_camera for ptiff_sink_create_camera */
#include "ptiff_image_bridge.h" /* ptiff_image_descriptor */

#ifdef __cplusplus
extern "C" {
#endif

/* Opaque handle over an open TIFF/BigTIFF file positioned at its primary image, ready for tile
 * reads. Never dereference from Go/Rust/Python/Ruby. */
typedef struct ptiff_source ptiff_source;

/* Opaque handle over an open TIFF/BigTIFF file being written, positioned at its primary image,
 * ready for tile writes. Never dereference from Go/Rust/Python/Ruby. */
typedef struct ptiff_sink ptiff_sink;

/* Opens the primary image in the TIFF/BigTIFF file at `path` for pixel reading. Returns a
 * heap-owned handle on success, or NULL on failure -- when `err_out` is non-NULL, *err_out is
 * then set to a negative ptiff error code (see ptiff_bridge.h). `path` must be non-NULL. The
 * returned handle owns the underlying file reader; release it with ptiff_source_close. */
PTIFF_C_API ptiff_source* ptiff_source_open(const char* path, int* err_out);

/* Releases a handle returned by ptiff_source_open. NULL is a no-op. */
PTIFF_C_API void ptiff_source_close(ptiff_source* source);

/* Fills `desc` with the opened image's metadata -- same fields ptiff_open_path fills. Returns 0
 * on success, a negative ptiff error code on failure (e.g. NULL source/desc). */
PTIFF_C_API int ptiff_source_descriptor(const ptiff_source* source, ptiff_image_descriptor* desc);

/* Tile grid dimensions at level 0. For a non-tiled (single-strip or striped) image this is
 * columns=1, rows=1 for a single strip covering the whole image, or more rows for a multi-strip
 * image -- every strip is exposed as a tile. Returns 0 for a NULL source. */
PTIFF_C_API uint32_t ptiff_source_tile_columns(const ptiff_source* source);
PTIFF_C_API uint32_t ptiff_source_tile_rows(const ptiff_source* source);

/* Byte size of one decoded tile's pixel data. Every tile (including edge tiles/strips, which
 * TIFF pads to a uniform size) is exactly this many bytes. Returns 0 for a NULL source. */
PTIFF_C_API size_t ptiff_source_tile_byte_size(const ptiff_source* source);

/* Reads tile (column, row) at level 0 -- decompressed and with any predictor already undone --
 * into `buffer`, which must be at least ptiff_source_tile_byte_size(source) bytes; `buffer_size`
 * is checked against that requirement and PTIFF_ERROR_INVALID_ARGUMENT is returned if it's too
 * small. On success returns 0 and sets *bytes_read to the number of bytes written (always
 * ptiff_source_tile_byte_size(source)). On failure returns a negative ptiff error code (see
 * ptiff_bridge.h) and leaves *bytes_read untouched. `source`, `buffer` and `bytes_read` must be
 * non-NULL. */
PTIFF_C_API int ptiff_source_read_tile(ptiff_source* source,
                                       uint32_t column,
                                       uint32_t row,
                                       uint8_t* buffer,
                                       size_t buffer_size,
                                       size_t* bytes_read);

/* ---------------------------------------------------------------------------
 * Write side (ptiff_sink)
 * ------------------------------------------------------------------------ */

/* Creates the TIFF/BigTIFF file at `path` for writing a single image described by `desc` and
 * returns a heap-owned handle positioned for tile writes. The header + IFD directory are
 * written immediately (so the file is valid on success); tiles are then written with
 * ptiff_sink_write_tile in any order, and the handle must be closed with ptiff_sink_close to
 * flush the underlying writer.
 *
 * The image must be tiled (desc->has_tile_info) -- TIFF tile I/O requires a tile layout. Pixel
 * type must be UInt8/UInt16/UInt32/Float32 (Float64 is not a supported TIFF sample type) and
 * channel_count must be 1 or 3; both are rejected with PTIFF_ERROR_INVALID_ARGUMENT otherwise.
 *
 * Returns NULL on failure -- when `err_out` is non-NULL, *err_out is then set to a negative
 * ptiff error code (see ptiff_bridge.h). `path` and `desc` must be non-NULL.
 *
 * Usage:
 *   ptiff_image_descriptor d{};
 *   d.width = 64, d.height = 32;
 *   d.pixel_type = PTIFF_PIXEL_UINT16, d.channel_count = 1;
 *   d.has_tile_info = 1, d.tile_info = {64, 32};
 *   ptiff_sink* s = ptiff_sink_create("out.tif", &d);   // or NULL + err_out
 *   // ... ptiff_sink_write_tile(s, c, r, buf, n) ...
 *   ptiff_sink_close(s);
 */
PTIFF_C_API ptiff_sink* ptiff_sink_create(const char* path, const ptiff_image_descriptor* desc);

/* Same as ptiff_sink_create, but additionally persists the structured camera calibration
 * `ptiff.camera.*` into the file's metadata (private tag 65002). The camera's intrinsics
 * (fx/fy/cx/cy), extrinsics (rotation quaternion + world translation) and ISO-8601 timestamp are
 * written as metadata fields, so a later ptiff_open_path_camera on the file returns the same
 * calibration. `camera` must be non-NULL; pass NULL to fall back to ptiff_sink_create (no camera
 * metadata). Returns NULL on failure (same rules as ptiff_sink_create). */
PTIFF_C_API ptiff_sink* ptiff_sink_create_camera(const char* path,
                                                 const ptiff_image_descriptor* desc,
                                                 const ptiff_camera* camera);

/* Tile grid dimensions of the sink's image (its layout), at level 0. Returns 0 for a NULL sink. */
PTIFF_C_API uint32_t ptiff_sink_tile_columns(const ptiff_sink* sink);
PTIFF_C_API uint32_t ptiff_sink_tile_rows(const ptiff_sink* sink);

/* Byte size of one written tile's pixel data. Every tile (including edge tiles, which TIFF pads
 * to a uniform size) is exactly this many bytes. Returns 0 for a NULL sink. */
PTIFF_C_API size_t ptiff_sink_tile_byte_size(const ptiff_sink* sink);

/* Writes one tile of pixel data. `buffer` is `buffer_size` bytes of raw, uncompressed sample
 * data for tile (column, row) at level 0; it must be exactly ptiff_sink_tile_byte_size(sink)
 * bytes (the sink checks that and returns PTIFF_ERROR_INVALID_ARGUMENT on a mismatch). The tile
 * index must lie within the sink's grid, or PTIFF_ERROR_OUT_OF_RANGE is returned. On success
 * returns 0. On failure returns a negative ptiff error code (see ptiff_bridge.h). `sink` and
 * `buffer` must be non-NULL. */
PTIFF_C_API int ptiff_sink_write_tile(
    ptiff_sink* sink, uint32_t column, uint32_t row, const uint8_t* buffer, size_t buffer_size);

/* Flushes the underlying writer (making the file complete and readable) and releases the handle
 * returned by ptiff_sink_create. NULL is a no-op. The file is not valid for reading until
 * ptiff_sink_close is called. */
PTIFF_C_API void ptiff_sink_close(ptiff_sink* sink);

#ifdef __cplusplus
}
#endif

#endif /* PTIFF_PIXEL_BRIDGE_H */
