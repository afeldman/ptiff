# C ABI binding for libptiff (`libptiff_c`)

A **language-agnostic** C binding over the [libptiff](../..) C++ API. It builds a
single `libptiff_c` library exposing a stable `extern "C"` surface (see the
headers here). Because every foreign-language runtime can call a plain C ABI, this
is the shared substrate that all high-level bindings build on:

```
            libptiff (C++23 core, fmt/spdlog)
                 │
                 ▼
        bindings/c → libptiff_c   (extern "C" veneer)
                 │
      ┌──────────┼──────────┬───────────┐
      ▼          ▼          ▼           ▼
     Go       Rust       Python       Ruby ...
```

Go, Rust, Python and Ruby all consume it ([`../go`](../go), [`../rust`](../rust),
[`../python`](../python), [`../ruby`](../ruby)).

## What it exposes

| libptiff C++ | C ABI (libptiff_c) |
|--------------|--------------------|
| `ptiff::compileTimeVersion()` | `ptiff_compile_time_version()` |
| `ptiff::runtimeVersion()`     | `ptiff_runtime_version()`     |
| `ptiff::ErrorCode`            | `ptiff_error_code` enum        |
| `ptiff::LogLevel`             | `ptiff_log_level` enum         |
| `ptiff::Logger::instance()`   | `ptiff_logger_*`              |
| `ptiff::io::BackendFactory`   | `ptiff_backend_names()`        |
| `ptiff::Image`                | `ptiff_image_*` opaque handle   |
| `ptiff::ImageDescriptor` / value types | `ptiff_image_descriptor`, `ptiff_tile_info` |
| TIFF open-by-path (reads primary image metadata) | `ptiff_open_path(path, desc)` |

## Design notes

- **Ownership.** Anything handed across the boundary is either an opaque handle
  (`ptiff_image*`, heap-owned, released with `ptiff_image_destroy`) or a `malloc`'d
  C string the caller frees with `ptiff_free_string`. This is symmetric from any
  language.
- **PIMPL.** `ptiff::Image` is a move-only PIMPL, so the bridge heap-allocates it
  and hands back an opaque pointer the consumer never dereferences directly.
- **No C++ types cross the boundary.** The headers are pure C (`extern "C"`, only
  `stdint.h`/`stddef.h`).
- **Symbol visibility.** The library builds with hidden visibility; every function
  is explicitly exported via `PTIFF_C_API` (see `ptiff_c_export.h`) so the symbols
  land in the dynamic symbol table for any FFI loader.

## Building

Enable it in the main CMake build:

```bash
cmake -B build -S . -DBUILD_SHARED_LIBS=ON -DPTIFF_BUILD_C_BINDINGS=ON
cmake --build build --target ptiff_c
```

Produces `libptiff_c` (in `build/…/bindings/c/`). Link it together with `libptiff`
and use the headers in this directory as the `ptiff_*` prototypes.

> **Prefer a shared `libptiff`** (`-DBUILD_SHARED_LIBS=ON`) so that the process-wide
> backend self-registration stays visible and `ptiff_backend_names()` reports every
> backend.

## Adding API

Extend the `ptiff_*` surface in the headers + `*_bridge.cpp` implementations, then
wrap it thinly in each language binding. High-level bindings never touch libptiff's
C++ headers.
