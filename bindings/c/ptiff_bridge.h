/*
 * ptiff_bridge.h
 *
 * Minimal C ABI over libptiff's stable, implemented core API. This is the bridge cgo
 * consumes: keeping it "extern C" and free of any C++ types means the Go side never has to
 * be rebuilt when internal C++ details change, only when the public C ABI changes.
 */
#ifndef PTIFF_BRIDGE_H
#define PTIFF_BRIDGE_H

#include <stddef.h>
#include <stdint.h>

#include "ptiff_c_export.h"

#ifdef __cplusplus
extern "C" {
#endif

/* ---- BackendFactory ---- */
/* Returns a newly allocated C string (caller frees with ptiff_free_string) containing a
 * comma-space-joined list of registered backend names, or NULL if none / on error. */
PTIFF_C_API char* ptiff_backend_names(void);
PTIFF_C_API void ptiff_free_string(char* s);

#ifdef __cplusplus
}
#endif

#endif /* PTIFF_BRIDGE_H */
