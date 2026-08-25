/* SWIG interface for the libptiff C ABI (bindings/swig).
   Pulls the REAL C ABI header directly via %include -- no hand-mirrored
   struct/function declarations. The generated wrapper is plain C (no -c++)
   bound only to the extern "C" surface of libptiff_c: no C++ runtime, no
   C++ headers, no libstdc++ involvement.

   One .i drives TWO active targets today (Go, Ruby) -- pick with `swig -go`
   or `swig -ruby` (see the Makefile). This is the *generic* SWIG path for
   "any language other than Python and Octave": Python is bound natively via
   PyO3 (crates/ptiff-python), and Octave via the hand-written C++ MEX adapter
   (bindings/octave/mex), so neither is a SWIG target. Adding another language
   (e.g. `swig -csharp`, `-java`, `-perl`, `-lua`, `-julia (...)`) only needs a
   new language guard block in typemaps.i -- see the note at the top of that
   file for the exact C-ABI patterns each language must map.

   Buffer/out-param typemaps are pulled from typemaps.i and guarded per-language
   (see the note at the top of that file).

   -------- The single source of truth: cbindgen's generated ptiff_c.h --------
   Since the Rust core took over (plan §7.4), the C ABI's source of truth is the
   `extern "C"` declarations in crates/ptiff-c/src/, from which cbindgen
   generates ONE consolidated header `target/ptiff_c.h` (see crates/ptiff-c/
   build.rs). That header defines PTIFF_C_API (e.g.
   __attribute__((visibility("default")))) which SWIG cannot parse. `make` builds
   real_inc/ as a copy of target/ptiff_c.h with the PTIFF_C_API block filtered
   out (PTIFF_C_API expands to nothing). SWIG parses against real_inc/; the
   %{...%} C block below is emitted verbatim into the generated wrapper and
   compiled with the REAL target/ include path, so struct layouts and prototypes
   come from the authoritative header.

   Because the header now carries concrete `typedef struct` bodies for the value
   structs (ptiff_version, ptiff_tile_info, ptiff_image_descriptor, ptiff_camera
   and ptiff_field) and opaque forward-declarations for the handles
   (ptiff_image, ptiff_source, ptiff_sink), a single %include below is enough --
   SWIG builds concrete classes for the former and keeps the latter opaque. */
%module ptiff

%{
/* C block: emitted verbatim into the generated wrapper and compiled by cc.
   Here we use the REAL header from the Rust-cbindgen workspace target/. */
#include <stdint.h>
#include <stddef.h>
#include "ptiff_c.h"
%}

%include <stdint.i>

/* Per-language buffer + out-param typemaps (see note in that file). */
%include "typemaps.i"

/* Pull the whole C ABI from the single generated header. The global include
   guard (#ifndef PTIFF_C_H) is fine for SWIG; PTIFF_C_API is already empty in
   the real_inc/ copy SWIG parses. */
%include "ptiff_c.h"

/* Keep the opaque handles opaque on the wrapping side (no value semantics). */
%nodefaultctor ptiff_image;
%nodefaultdtor ptiff_image;
%nodefaultctor ptiff_source;
%nodefaultdtor ptiff_source;
%nodefaultctor ptiff_sink;
%nodefaultdtor ptiff_sink;
