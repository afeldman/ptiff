#ifndef PTIFF_VERSION_H
#define PTIFF_VERSION_H

#include <stddef.h>
#include <stdint.h>

#include "ptiff_c_export.h"

#ifdef __cplusplus
extern "C" {
#endif

/* ---- Version ---- */
typedef struct ptiff_version {
    int major;
    int minor;
    int patch;
} ptiff_version;

PTIFF_C_API ptiff_version ptiff_runtime_version(void);
PTIFF_C_API ptiff_version ptiff_compile_time_version(void);

/* Out-parameter variants so runtimes without C struct-by-value return support
 * (e.g. Ruby Fiddle) can read the version without an aggregate return. Either
 * pair of pointers may be NULL. */
PTIFF_C_API void ptiff_runtime_version_out(int* major, int* minor, int* patch);
PTIFF_C_API void ptiff_compile_time_version_out(int* major, int* minor, int* patch);

#ifdef __cplusplus
}
#endif

#endif // PTIFF_VERSION_H
