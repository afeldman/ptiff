/* SWIG interface for the libptiff C ABI (bindings/swig).
   Pulls the REAL C ABI headers directly via %include -- no hand-mirrored
   struct/function declarations. The generated wrapper is plain C (no -c++)
   bound only to the extern "C" surface of libptiff_c: no C++ runtime, no
   C++ headers, no libstdc++ involvement.

   One .i drives THREE targets (Python, Go, Ruby) -- pick with `swig -python`,
   `-go`, `-ruby` (see the Makefile). Buffer/out-param typemaps are pulled from
   typemaps.i and are guarded per-language (see the note at the top of that
   file): SWIG typemaps are language-specific and CANNOT be shared.

   -------- Why the shim include dir (real_inc/) for SWIG's parser only --------
   ptiff_c_export.h expands PTIFF_C_API to __attribute__((visibility("default")))
   on GCC/clang, which SWIG cannot parse. `make` builds real_inc/ as a
   byte-identical copy of ../c with only ptiff_c_export.h replaced by
   swig_include/ptiff_c_export.h (PTIFF_C_API expands to nothing). SWIG parses
   against real_inc/; the %{...%} C block below is emitted verbatim into the
   generated wrapper and compiled with the REAL ../c include path, so struct
   layouts and prototypes come from the authoritative headers.

   We %include the export header first so SWIG registers the (empty) macro
   before the real headers reference it, and %include each defining header
   explicitly so SWIG builds concrete classes for the value structs
   (ptiff_version, ptiff_tile_info, ptiff_image_descriptor) rather than leaving
   them opaque pointers. */
%module ptiff

%{
/* C block: emitted verbatim into the generated wrapper and compiled by cc.
   Here we use the REAL headers from bindings/c. */
#include <stdint.h>
#include <stddef.h>
#include "ptiff_bridge.h"       /* ptiff_backend_names */
#include "ptiff_version.h"      /* ptiff_version, ptiff_runtime_version */
#include "ptiff_logger.h"       /* ptiff_log_level, ptiff_logger_* */
#include "ptiff_error.h"        /* ptiff_error_code */
#include "ptiff_camera.h"       /* ptiff_camera, ptiff_open_path_camera */
#include "ptiff_image_bridge.h" /* ptiff_tile_info, ptiff_image_descriptor */
#include "ptiff_metadata.h"      /* ptiff_open_path, ptiff_field, ptiff_open_path_fields */
#include "ptiff_pixel_bridge.h" /* ptiff_source_*, ptiff_sink_* */
%}

%include <stdint.i>

/* Per-language buffer + out-param typemaps (see note in that file). */
%include "typemaps.i"

/* Register the (empty for SWIG) export macro first, then pull the real headers. */
%include "ptiff_c_export.h"
%include "ptiff_bridge.h"
%include "ptiff_version.h"
%include "ptiff_logger.h"
%include "ptiff_error.h"
%include "ptiff_camera.h"
%include "ptiff_image_bridge.h"
%include "ptiff_metadata.h"
%include "ptiff_pixel_bridge.h"

/* Keep the opaque handles opaque on the wrapping side (no value semantics). */
%nodefaultctor ptiff_source;
%nodefaultdtor ptiff_source;
%nodefaultctor ptiff_sink;
%nodefaultdtor ptiff_sink;
