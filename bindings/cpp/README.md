# ptiff-cpp — modern C++ wrapper over the PTIFF C ABI

`bindings/cpp` is the Phase 8 C++ wrapper (`ptiff-cpp`): a **modern, hand-written
C++ facade over the stable C ABI** (`libptiff_c`, built by `crates/ptiff-c` from
the Rust core). It reproduces the API contract the historical C++ reference
library exposed — the type names `Image`, `Scene`, `Reader`, `Writer`, `Result<T>`,
`Error`, `ErrorCode`, `ImageDescriptor`, `PixelType`, `CompressionKind`,
`TileInfo` and `Camera` — but speaks only the `extern "C"` surface of the Rust
core. It never compiles any C++ from the (now-deleted) C++ reference.

## Architecture

```
C++ (Consumer)
   │
   ▼
ptiff-cpp  (bindings/cpp/include/ptiff/*.hpp — header-only, modern, RAII)
   │
   ▼
C ABI      (libptiff_c — crates/ptiff-c, cbindgen authorises target/ptiff_c.h)
   │
   ▼
Rust Core  (ptiff-core)
```

- **`Result<T> = std::expected<T, Error>`** — every fallible call returns a value
  (domain failures), never throws. `Error` carries a stable `ErrorCode`, a
  message and a `std::source_location`, mirroring the historical contract.
- **RAII** — each opaque C handle is wrapped in a move-only owning object whose
  destructor calls the matching `_close`/`_destroy` (`Image` → `ptiff_image_*`,
  `Source` → `ptiff_source_*`, `Writer` → `ptiff_sink_*`). `NULL` is always a
  no-op.
- **STL** — `std::vector<uint8_t>` for tile buffers, `std::optional`,
  `std::string_view`, `std::reference_wrapper`.

## What is implemented

| ptiff-cpp type | C ABI backing |
|----------------|---------------|
| `ErrorCode` / `Error` / `Result<T>` | `ptiff_error_code` (stable, additive) |
| `Id<Tag>` / `ImageId` | — (id semantics over image indices) |
| `PixelType` / `CompressionKind` / `TileInfo` | `ptiff_pixel_type` / `ptiff_compression_kind` / `ptiff_tile_info` |
| `ImageDescriptor` | `ptiff_image_descriptor` (marshalled) |
| `Image` | `ptiff_image_create` / `ptiff_image_*` accessors |
| `Scene` | in-memory container of `Image`s (addImage/image/imageAt/imageCount) |
| `Camera` | `ptiff_open_path_camera` (read) + `ptiff_sink_create_camera` (write) |
| `Reader::open` / `scene()` / `imageSource()` | `ptiff_source_open` / `ptiff_open_path` / `ptiff_source_read_tile` |
| `Writer::create` / `write(scene, provider)` | `ptiff_sink_create` / `ptiff_sink_write_tile` |
| `compileTimeVersion()` / `runtimeVersion()` | `ptiff_compile_time_version` / `ptiff_runtime_version` |

The `detail/c_abi.hpp` header hand-marshals the exact slice of the C ABI the
wrapper uses (struct layout + enum values pinned to `target/ptiff_c.h`, the
cibindgen-generated source of truth).

## Build & test

The wrapper is **header-only** — consumers just `#include <ptiff/ptiff.hpp>` and
link `-lptiff_c`. The `Makefile` builds the round-trip + API-parity test against
the Rust-built `libptiff_c` in `<workspace>/target/release`:

```sh
cargo build -p ptiff-c --release   # build libptiff_c (once)
make test                          # build + run the C++ test
```

`make test` runs `test_ptiff_cpp` which covers: version accessors, error-code
mapping, `Image`/`Scene`/`ImageDescriptor` construction, a full
`Writer`→`Reader` pixel round-trip (tiled image, pattern-verified tiles),
structured `Camera` write+read, and negative paths (`NotFound` on missing file,
`OutOfRange` on bad image id). Expected output: `ptiff_cpp: ALL OK`.

Override the library location with `PTIFF_C_LIB_DIR=<dir>` if not in the default
release dir.

## Test files

- `tests/test_ptiff_cpp.cpp` — round-trip + API-parity test (Phase 8 DoD).
