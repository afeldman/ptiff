#ifndef PTIFF_CAMERA_H
#define PTIFF_CAMERA_H

#include "ptiff_c_export.h"

#ifdef __cplusplus
extern "C" {
#endif

/* ---- Structured camera calibration (ptiff.camera.* fields) ---- */

/* A structured view of the `ptiff.camera.*` extension fields: the pinhole
 * intrinsics (fx, fy, cx, cy), the extrinsics (rotation quaternion + world
 * translation), the ISO-8601 observation timestamp, and the three derived
 * matrices. All matrices are row-major doubles. `has_intrinsics` /
 * `has_extrinsics` are 0 when the corresponding field group is absent from the
 * file; `projection` is set whenever both groups are present. `timestamp` is
 * an empty string when unset. */
typedef struct ptiff_camera {
    /* intrinsics */
    int has_intrinsics;
    double focal_length_x;
    double focal_length_y;
    double principal_x;
    double principal_y;
    double intrinsics[9]; /* 3x3 K (row-major) */
    /* extrinsics */
    int has_extrinsics;
    double rotation_w, rotation_x, rotation_y, rotation_z;
    double position_x, position_y, position_z;
    double extrinsics[12]; /* 3x4 [R|t] (row-major) */
    /* projection: P = K * [R|t], 3x4 (row-major); zeros when not derivable */
    double projection[12];
    /* ISO-8601 UTC observation timestamp, empty string when unset */
    char timestamp[64];
} ptiff_camera;

/* Opens a TIFF/BigTIFF file at `path` (read-only) and fills `out` with the
 * structured camera calibration decoded from the `ptiff.camera.*` extension
 * fields. Returns 0 on success; on failure returns a negative ptiff error
 * code and leaves *out untouched. `path` and `out` must be non-NULL. */
PTIFF_C_API int ptiff_open_path_camera(const char* path, ptiff_camera* out);

#ifdef __cplusplus
}
#endif

#endif // PTIFF_CAMERA_H
