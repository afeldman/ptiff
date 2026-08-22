/*
 * ptiff_c_export.h
 *
 * Export macro for the libptiff_c C ABI. The library builds with hidden symbol
 * visibility by default; every function of the public C surface is explicitly
 * marked PTIFF_C_API so it lands in the dynamic symbol table (and is importable
 * by any FFI consumer: Go cgo, Rust, Python ctypes, Ruby FFI, ...).
 */
#ifndef PTIFF_C_EXPORT_H
#define PTIFF_C_EXPORT_H

#if defined(_WIN32) && !defined(PTIFF_C_STATIC_DEFINE)
#ifdef ptiff_c_EXPORTS
#define PTIFF_C_API __declspec(dllexport)
#else
#define PTIFF_C_API __declspec(dllimport)
#endif
#elif defined(__GNUC__) || defined(__clang__)
#define PTIFF_C_API __attribute__((visibility("default")))
#else
#define PTIFF_C_API
#endif

#endif /* PTIFF_C_EXPORT_H */
