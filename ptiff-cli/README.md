# ptiff-cli

A command-line interface for the PTIFF planetary image standard. The `ptiff`
binary is a thin wrapper over the idiomatic Rust binding (`bindings/rust`),
which talks only to the language-agnostic C ABI `libptiff_c` — the C++ API of
`libptiff` is never compiled or linked from this crate.

## Architecture

```
libptiff (C++, fmt/spdlog)
      │
      bindings/c → libptiff_c           (stable extern "C" ABI)
          │
      bindings/rust → ptiff crate       (idiomatic FFI, reused here)
          │
      ptiff-cli → ptiff binary          (clap CLI)
```

## Commands

| Command                         | Description                                                        |
|---------------------------------|--------------------------------------------------------------------|
| `ptiff version`                 | Prints the runtime and compile-time version of the linked libptiff |
| `ptiff backends`                | Lists the registered backend names (tiff, memory, zarr, …)         |
| `ptiff logger level`            | Prints the current process-wide logger level                       |
| `ptiff logger set <L>`          | Sets the process-wide logger level (same process only)             |
| `ptiff info <file>`             | Reads an on-disk TIFF/BigTIFF file's metadata: primary image descriptor, structured camera calibration (when present) and the PTIFF extension fields |
| `ptiff info --json <file>`      | Same, as JSON — includes the structured camera and the PTIFF private-tag (65001-65005) fields |
| `ptiff make <W> <H> <PIX> …`    | Constructs an `ImageDescriptor` and prints its resolved metadata   |
| `ptiff copy <SRC> <DST>`        | Tile-copies the primary image, preserving the camera calibration    |
| `--log-level <L>` (global)      | Sets the logger before running a command                           |

`pixel type` values: `uint8 | uint16 | uint32 | float32 | float64` (case-insensitive).

`compression` values: `none | lzw | deflate | jpeg`.

`logger level` values: `trace | debug | info | warn | error | critical | off`.

## Examples

```bash
ptiff version
ptiff backends
ptiff --log-level debug version
ptiff info /path/to/image.tif
# Read metadata (incl. the PTIFF private-tag fields) as JSON:
ptiff info --json /path/to/image.tif
# Construct rather than open a file:
ptiff make 1024 1024 uint16 --gsd 0.5 --tile-width 256 --tile-height 256 --compression lzw --channels 3
```

## Building & running

Prereq: build `libptiff_c` (and its shared `libptiff` dependency) via the repo's
CMake/Conan build, e.g. into `build/`:

```bash
cmake -S . -B build -G Ninja \
  -DCMAKE_TOOLCHAIN_FILE="$(pwd)/build/build/Release/generators/conan_toolchain.cmake" \
  -DBUILD_SHARED_LIBS=ON -DPTIFF_BUILD_C_BINDINGS=ON \
  -DPTIFF_BUILD_TESTS=OFF
cmake --build build --target ptiff_c
```

Then build the CLI. The binary locates `libptiff_c`/`libptiff` (and embeds the
rpath so it finds both at runtime) via the same resolution order as
`bindings/rust/build.rs`:

1. **pkg-config `libptiff_c`** for an *installed* prefix (recommended): once
   `libptiff_c` and `libptiff` are installed, point `PKG_CONFIG_PATH` at the
   prefix's `lib/pkgconfig`.
2. **`PTIFF_C_LIB_DIR` / `PTIFF_LIB_DIR`** for fast in-tree dev against a
   CMake/Conan build without installing.

Installed-prefix flow:

```bash
cmake --install /path/to/build --prefix /where/ever
cd ptiff-cli
PKG_CONFIG_PATH=/where/ever/lib/pkgconfig cargo run -- version
```

In-tree dev flow:

```bash
cd ptiff-cli
PTIFF_C_LIB_DIR=/path/to/build/.../bindings/c \
PTIFF_LIB_DIR=/path/to/build/.../libptiff \
cargo run -- version
```

## Testing

```bash
# installed prefix:
PKG_CONFIG_PATH=/where/ever/lib/pkgconfig cargo test
# or in-tree dev:
PTIFF_C_LIB_DIR=/path/to/build/.../bindings/c \
PTIFF_LIB_DIR=/path/to/build/.../libptiff \
cargo test
```

## Status

`ptiff info <file>` reads a file's metadata via the unified
`ptiff::read_metadata` entry point: the primary image descriptor
(`ptiff_open_path`), the PTIFF private-tag (65001-65005) extension fields
(`ptiff_open_path_fields`) and the structured camera calibration
(`ptiff_open_path_camera`). It reports pixel type, dimensions, channel count,
the layout tile/strip size, the camera model / intrinsics / extrinsics / K and
P matrices when present, and all extension fields. `ptiff make …` remains for
constructing an in-memory descriptor without a file. `ptiff copy` mirrors the
source's tile layout and preserves its structured camera calibration on the
destination when present.
