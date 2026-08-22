# Rust bindings for libptiff

Idiomatic Rust bindings for [libptiff](../..), via the language-agnostic C ABI
([`bindings/c`](../c), built as `libptiff_c`). This crate never links libptiff's
C++ API directly — it talks only to the stable `extern "C"` surface, exactly like
the Go, Python and Ruby bindings.

## Consuming the library (build.rs)

The [`build.rs`](./build.rs) locates `libptiff_c` (and its transitive `libptiff`)
with the following resolution order:

1. **`PTIFF_C_LIB_DIR`** env var (plus `PTIFF_LIB_DIR`) — for in-tree dev/CI builds
   that compile from source and point at the CMake/Conan output without installing.
2. **pkg-config `libptiff_c`** — the standard, layout-agnostic path for an
   *installed* library. The generated `libptiff_c.pc` (see
   [`bindings/c/CMakeLists.txt`](../c/CMakeLists.txt)) is relocatable, so it works
   against any prefix on `PKG_CONFIG_PATH`.

The `pkg-config` build-dependency emits the `-L`/`-l`/`-rpath` flags, so the shared
libraries resolve both at link and run time.

## Build / test

From an install prefix (recommended):

```bash
# Build + install libptiff_c (and libptiff) once:
cmake -B build -S . -DBUILD_SHARED_LIBS=ON -DPTIFF_BUILD_C_BINDINGS=ON
cmake --build build --target ptiff_c
cmake --install build --prefix /where/ever

# Then build/test the binding against it:
PKG_CONFIG_PATH=/where/ever/lib/pkgconfig \
DYLD_LIBRARY_PATH=/where/ever/lib \
cargo test
```

For fast in-tree dev without installing:

```bash
PTIFF_C_LIB_DIR=/path/to/build/bindings/c \
PTIFF_LIB_DIR=/path/to/build/libptiff \
cargo test
```

## Covered API

The idiomatic crate mirrors the language-agnostic C ABI 1:1 and adds a few
convenience helpers:

- `Version` / `runtime_version()` / `compile_time_version()`
- `ErrorCode`, `LogLevel`, `PixelType` (+ `PixelType::name()` friendly names),
  `CompressionKind`
- `backend_names()`
- `Logger` (reference to the process-wide singleton)
- `Image` (RAII over the opaque handle) + `ImageDescriptor` / `TileInfo` /
  `ImageDescriptorBuilder`
- **One-stop inspect**: `read_metadata(path)` → `Metadata { file, fields,
  camera }`, with `Metadata::field(key)` lookup — uniform with the MCP binding's
  `read_metadata` tool
  - `open_path(path)` → `FileMetadata` (primary image descriptor)
  - `open_path_fields(path)` → `Vec<(String, String)>` (flattened
    `ptiff.<domain>.<name>` extension fields from private tags 65001-65005)
  - `open_path_camera(path)` → `Camera` (structured calibration incl. `model`,
    K / [R|t] / P matrices and ISO-8601 timestamp)
- `Camera` — structured calibration carried by a file; `Camera::pinhole(...)`
  builds one for **writing**
- `Sink` (RAII write side): `Sink::create(path, builder)` for a plain tiled
  image, `Sink::create_with_camera(path, builder, &camera)` to persist the
  calibration as `ptiff.camera.*`; `tile_columns/rows/byte_size`, `write_tile`
- `Source` (RAII read side): `Source::open(path)`, `descriptor()`,
  `descriptor_builder()`, `tile_columns/rows/byte_size`, `read_tile`

See the `src/` doc comments for the exact semantics and the `cfg(test)` module
for roundtrip tests covering write→read of both plain and camera-annotated
tiled images.

## FFI generation (bindgen)

The raw `unsafe extern "C"` declarations are **not hand-maintained**: `build.rs`
runs [`rust-bindgen`](https://rust-lang.github.io/rust-bindgen/) (via the
`bindgen` build-dependency) over the C ABI headers (`ptiff_bridge.h`,
`ptiff_image_bridge.h`, `ptiff_pixel_bridge.h`, `ptiff_camera.h`) and writes
`$OUT_DIR/ptiff_ffi.rs`.
`src/lib.rs` re-exports the C value-typed structs and opaque handles from that
generated `ffi` module, and routes every call through `ffi::ptiff_*`. The C
headers under `bindings/c` are therefore the single source of truth — updating
them (e.g. adding a `ptiff_source_*` function) automatically refreshes the FFI on
the next build.
