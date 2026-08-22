# libptiff

Reference implementation of the PTIFF standard (see `rfcs/RFC-0001-Core.md` in
the repository root).

## Status

**Release `0.4.0`.** TIFF/BigTIFF read/write (LZW, PackBits, Deflate, JPEG,
multi-image IFD chains), the pluggable backends for PDS4, ISIS3 CUB, Zarr,
OpenEXR and in-memory storage, and HTTP-range reads of remote objects are
implemented and covered by the Catch2 suite. New in 0.4.0: `samplesPerPixel`
supports arbitrary band counts (1-512, not just gray/RGB) for multispectral
imagery, with a correctly emitted `ExtraSamples` tag. New in 0.3.0: the PTIFF
private tags **65001–65005** (SPICE, camera geometry, CRS, scientific layers,
provenance) are emitted and parsed as concrete TIFF tags via a versioned
payload codec, so `ptiff.<domain>.*` fields round-trip through the container.
The higher-level `Camera`, `CoordinateReferenceSystem`, `ScientificLayer`
and `Metadata` model classes remain interface stubs until those domains'
normative schemas are standardized. See
[`../ARCHITECTURE.md`](../ARCHITECTURE.md) for the full design.

## Build

Requires a C++23 compiler and CMake >= 3.26.

```bash
conan profile detect --force        # first time only
conan install . --output-folder=build --build=missing
cmake -B build -S . -DCMAKE_TOOLCHAIN_FILE=build/conan_toolchain.cmake
cmake --build build
ctest --test-dir build --output-on-failure
```

vcpkg is also fully supported (`vcpkg.json` at the repo root describes the same dependency set):

```bash
cmake -B build -S . -DCMAKE_TOOLCHAIN_FILE=$VCPKG_ROOT/scripts/buildsystems/vcpkg.cmake
cmake --build build
ctest --test-dir build --output-on-failure
```

## Build options

| Option | Default | Purpose |
|---|---|---|
| `PTIFF_BUILD_TESTS` | `ON` | Build the Catch2 test suite |
| `PTIFF_BUILD_EXAMPLES` | `OFF` | Build `examples/` |
| `PTIFF_BUILD_BENCHMARKS` | `OFF` | Build `benchmarks/` |
| `BUILD_SHARED_LIBS` | `OFF` | Build `ptiff` as a shared library instead of static |
| `PTIFF_ENABLE_ASAN` | `OFF` | Build with AddressSanitizer |
| `PTIFF_ENABLE_UBSAN` | `OFF` | Build with UndefinedBehaviorSanitizer |
| `PTIFF_ENABLE_TSAN` | `OFF` | Build with ThreadSanitizer (mutually exclusive with ASAN) |

## Layout

See [`../ARCHITECTURE.md`](../ARCHITECTURE.md) for the directory layout, namespace structure,
error model, and ownership policy.
