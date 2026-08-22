#ifndef PTIFF_ERROR_H
#define PTIFF_ERROR_H

#include <stddef.h>
#include <stdint.h>

#include "ptiff_c_export.h"

#ifdef __cplusplus
extern "C" {
#endif

/* ---- ErrorCode (must match ptiff::ErrorCode ordering) ---- */
enum ptiff_error_code {
    PTIFF_ERROR_NOT_IMPLEMENTED = 0,
    PTIFF_ERROR_INVALID_ARGUMENT = 1,
    PTIFF_ERROR_OUT_OF_RANGE = 2,
    PTIFF_ERROR_NOT_FOUND = 3,
    PTIFF_ERROR_UNKNOWN = 4,
};

#ifdef __cplusplus
}
#endif

#endif // PTIFF_ERROR_H
