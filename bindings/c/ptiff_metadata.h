/*
 * ptiff_metadata.h
 *
 * C ABI for reading a file's metadata without materialising a live
 * ptiff::Image handle: both the core image descriptor (width/height/pixel
 * type/channel count/tile info) and the flattened PTIFF extension fields
 * (private tags 65001-65005, e.g. "ptiff.camera.model"). Each call opens the
 * TIFF/BigTIFF file at `path` read-only, decodes the metadata, and returns it
 * as C value(s); nothing is owned across calls.
 */
#ifndef PTIFF_METADATA_H
#define PTIFF_METADATA_H

#include "ptiff_c_export.h"
#include "ptiff_image_bridge.h" /* ptiff_image_descriptor */

#ifdef __cplusplus
extern "C" {
#endif

/* Opens a TIFF/BigTIFF file at `path` (read-only) and fills `desc` with the
 * primary image's metadata (width, height, pixel type, channel count and, when
 * present, tile info). Uses libptiff's TIFF backend publicly (FileBinaryReader
 * + TiffBackend). Compression is not yet exposed over this ABI.
 *
 * Returns 0 on success. On failure returns a negative ptiff error code
 * (see ptiff_error.h) and leaves *desc untouched. `path` and `desc` must be
 * non-NULL. */
PTIFF_C_API int ptiff_open_path(const char* path, ptiff_image_descriptor* desc);

/* ---- PTIFF extension fields (private tags 65001-65005) ---- */

/* A single flattened PTIFF extension field (private tags 65001-65005). `key`
 * is the fully qualified field name, e.g. "ptiff.spice.frame" or
 * "ptiff.camera.model". Both strings are malloc'd and owned by the array
 * returned from `ptiff_open_path_fields`; free the whole array with
 * `ptiff_fields_free`. */
typedef struct ptiff_field {
    char* key;
    char* value;
} ptiff_field;

/* Opens a TIFF/BigTIFF file at `path` (read-only) and returns the flattened
 * PTIFF extension fields decoded from the private tags 65001-65005 (SPICE,
 * camera geometry, CRS, scientific layers, provenance) together with the
 * primary image metadata. Field keys carry the `ptiff.<domain>.<name>` shape
 * (e.g. "ptiff.spice.frame"). Ordering is lexicographic by key; domains absent
 * from the file simply contribute no entries.
 *
 * On success, returns 0, sets *out_count to the number of fields and *out to a
 * malloc'd array of that many `ptiff_field`s (which is NULL when count == 0);
 * the caller must call `ptiff_fields_free(*out, *out_count)`. On failure
 * returns a negative ptiff error code and leaves *out / *out_count untouched.
 * `path`, `out` and `out_count` must be non-NULL. */
PTIFF_C_API int ptiff_open_path_fields(const char* path, ptiff_field** out, int* out_count);
/* Frees an array returned by `ptiff_open_path_fields`. NULL array is a no-op;
 * count is required to release each field's two strings. */
PTIFF_C_API void ptiff_fields_free(ptiff_field* arr, int count);

#ifdef __cplusplus
}
#endif

#endif // PTIFF_METADATA_H
