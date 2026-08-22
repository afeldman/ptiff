/* SWIG extension-include shim for ptiff_c_export.h.
   SWIG's parser chokes on the visibility("default") attribute that the real
   header expands PTIFF_C_API to on GCC/clang. For SWIG-only parsing we make
   PTIFF_C_API expand to nothing; the C-compiled wrapper still uses the real
   header (via the %{...%} block, which is passed through to the C compiler
   with the real -I path in front). */
#ifndef PTIFF_C_EXPORT_H
#define PTIFF_C_EXPORT_H
#define PTIFF_C_API
#endif
