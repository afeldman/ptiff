# PTIFF Architecture

> **⚠️ Historical (pre-Rust) document.** This file describes the former
> **C++** architecture of the reference implementation (`libptiff`, since
> removed). The reference implementation is now a **pure-Rust workspace** — see
> [`RUST-WORKSPACE.md`](RUST-WORKSPACE.md) and
> [`PTIFF-1.0-RUST-CORE-PLAN.md`](PTIFF-1.0-RUST-CORE-PLAN.md) for the current
> architecture (`ptiff-core` → `ptiff-rust` → `ptiff-cli`/`ptiff-c`). This
> legacy page is retained for historical context only.

This document describes `libptiff`, the (former C++) reference implementation of the PTIFF standard
(see `rfcs/RFC-0001-Core.md`). It reflects the state established through Sprint 3: architecture,
build system, domain model, and storage/I/O architecture -- no TIFF/BigTIFF/PDS4/ISIS reading,
writing, or parsing exists yet.

## Directory layout

```
libptiff/
  CMakeLists.txt
  include/ptiff/            # public, installed headers -- small, stable surface
    ptiff.hpp                 # umbrella
    core/{result.hpp, error.hpp, id.hpp, version.hpp (generated), precondition.hpp}
    logging/logger.hpp
    io/{reader.hpp, writer.hpp, binary_reader.hpp, binary_writer.hpp}
    io/{serializer.hpp, deserializer.hpp, storage_model.hpp}
    io/{storage_backend.hpp, backend_capabilities.hpp, backend_factory.hpp}
    io/{image_source.hpp, image_sink.hpp}
    io/tile/{tile_index.hpp, tile_extent.hpp, tile_region.hpp, tile_layout.hpp, tile.hpp}
    io/tile/{tile_cache.hpp, tile_storage.hpp, tile_iterator.hpp}
    io/backend/{tiff,memory,pds4,isis,zarr,cloud,openexr}_backend.hpp
    metadata/{metadata.hpp, scientific_layer.hpp, history.hpp, layer_kind.hpp, mission.hpp}
    geometry/{camera.hpp, coordinate_reference_system.hpp, lens_model.hpp, projection.hpp}
    geometry/{ellipsoid.hpp, extrinsics.hpp, intrinsics.hpp, planet.hpp, quaternion.hpp,
      vector3.hpp}
    geometry.hpp
    image/{compression_kind.hpp, image_descriptor.hpp, pixel_type.hpp, tile_info.hpp}
    image.hpp
    scene.hpp
    annotation.hpp
  src/                       # impl, private headers + .cpp, never installed
    core/ detail/ io/ io/backend/ logging/ metadata/ geometry/
  tests/{unit,integration,golden,conformance}/   # only unit/ has content this sprint
  examples/   benchmarks/   cmake/
```

`include/` vs `src/` is the standard split for keeping the ABI-facing surface small and stable
while internals churn freely.

## Namespaces

- `ptiff::` -- public vocabulary: `Image`, `Scene`, `Reader`, `Writer`, `Metadata`, `Camera`,
  `CoordinateReferenceSystem`, `ScientificLayer`, `Version`, `Result<T>`, `Error`, `Logger`,
  `LogLevel`.
- `ptiff::detail::` -- PIMPL bodies, never named by consumers.
- `ptiff::io::`, `ptiff::metadata::`, `ptiff::geometry::` -- internal representations backing the
  public facades above.
- `ptiff::camera::`, `ptiff::compression::`, `ptiff::utility::` -- reserved, declared in
  `libptiff/src/detail/namespaces.hpp` only. No files/directories exist for them yet; they're
  created for real the sprint something actually lives in them.

## Error model

Recoverable/domain errors use `Result<T> = std::expected<T, Error>` (`ptiff/core/result.hpp`,
`ptiff/core/error.hpp`). Exceptions never cross the public API for expected failure modes.
Exceptions are reserved for genuine programmer-error contract violations via
`PTIFF_PRECONDITION(cond)` (`ptiff/core/precondition.hpp`), which aborts unconditionally --
including in release builds, since a violated precondition is a bug, not something to skip.

Full rationale and the project-wide rule ("Result<T> for domain errors, exceptions only for
contract violations") are in `docs/CODING_GUIDELINES.md`.

## Versioning

`ptiff/core/version.hpp` is generated at configure time from `version.hpp.in`, combining
`PROJECT_VERSION` with an optional git describe hash (`libptiff/cmake/GitVersion.cmake`; falls
back to `"unknown"` outside a git checkout). Two entry points exist deliberately:

- `compileTimeVersion()` -- resolved from the headers a consumer compiles against.
- `runtimeVersion()` -- resolved from the linked library.

Comparing the two is a future ABI-mismatch check once shared builds are common. Pre-1.0: any
release may break API/ABI, per SemVer itself.

## Logging

`ptiff::Logger` (`ptiff/logging/logger.hpp`) is a thread-safe, process-wide facade. It exposes
zero spdlog types -- spdlog lives entirely behind `ptiff::detail::SpdlogLogger`
(`src/logging/detail/spdlog_logger.{hpp,cpp}`). This is deliberate: a third-party type leaking
into a public header becomes a compile-time dependency for every consumer, forever.

## Memory / ownership policy

- Value types (`Error`, `Version`, `LogLevel`, `Result<T>`) have no PIMPL -- small and stable,
  indirection would be pure overhead.
- Growing domain types (`Image`, `Scene`, `Metadata`, `Camera`, `CoordinateReferenceSystem`,
  `ScientificLayer`) use PIMPL via `std::unique_ptr<Impl>`, with move ctor/assign/dtor defined in
  the `.cpp` (required for the incomplete-type deleter). Move-only this sprint; copy semantics are
  decided per-type once there's real state.
- `Reader`/`Writer` are abstract interfaces (pure virtual), with concrete backends living in
  `detail::` and selected via a factory (`Reader::open`, `Writer::create`) returning
  `Result<std::unique_ptr<...>>`. This is the one deliberate polymorphism spot: future backends
  (mmap / stream / object-storage range-reads) need to be swappable without touching the public
  header.
- `shared_ptr` is not used anywhere this sprint -- no genuine shared-ownership case exists yet.

`ptiff::Image` is the fully worked reference pattern for the PIMPL domain-type shape; `Scene`,
`Metadata`, `ScientificLayer`, `Camera`, and `CoordinateReferenceSystem` follow it exactly.

## ABI

Export macros are generated via CMake's `generate_export_header()` rather than hand-rolled --
it already gets the per-platform/per-compiler visibility rules right (MSVC dllexport/dllimport,
GCC/Clang visibility attributes). `CXX_VISIBILITY_PRESET hidden` and `VISIBILITY_INLINES_HIDDEN
ON` are set unconditionally on the `ptiff` target, even while `BUILD_SHARED_LIBS` defaults to
`OFF`, so nothing has to change when shared builds are enabled later.

## Build system

CMake (C++23) + Conan 2 as the primary dependency manager, with full vcpkg compatibility
(`conanfile.txt` / `vcpkg.json` describe the same dependency set). Sprint 1 dependencies: `fmt`,
`spdlog`, `catch2` -- `boost`, `eigen`, `libtiff`, `opencv4`, `gdal` are intentionally not listed
yet and get added when the code that needs them lands.

- `libptiff/cmake/CompilerWarnings.cmake` -- `INTERFACE` target `ptiff_warnings`, warnings-as-error
  posture across GCC/Clang/MSVC.
- `libptiff/cmake/Sanitizers.cmake` -- `PTIFF_ENABLE_ASAN` / `PTIFF_ENABLE_UBSAN` /
  `PTIFF_ENABLE_TSAN` options, off by default, TSAN mutually exclusive with ASAN.
- `libptiff/cmake/GitVersion.cmake` -- git describe resolution feeding `version.hpp`.

## Storage & I/O architecture (Sprint 3)

`libptiff` never depends on a concrete file format directly. The public domain model (`Scene`,
`Image`, `Camera`, ...) talks only to `ptiff::io::Serializer`/`Deserializer`, which convert to/from
a format-neutral `ptiff::io::StorageModel`. Everything below that is a `ptiff::io::StorageBackend`'s
problem, selected through `ptiff::io::BackendFactory`'s name-based registry. Concrete formats (TIFF/
BigTIFF, PDS4, ISIS3 CUB, Zarr, OpenEXR, in-memory, cloud object storage) live in
`ptiff::io::backend` as `StorageBackend` subclasses -- `TiffBackend`, `MemoryBackend`,
`Pds4Backend`, `IsisBackend`, `ZarrBackend`, `CloudBackend`, `OpenExrBackend`. Pixel data moves
through `ptiff::io::ImageSource`/`ImageSink` in `ptiff::io::tile::Tile` units (never a whole-image
buffer), described by a `ptiff::io::tile::TileLayout`, over `ptiff::io::BinaryReader`/`BinaryWriter`
byte-level transports. Layering, thread-safety and memory/ownership decisions follow the same
architecture established for the core model and the TIFF backend.

This sprint is architecture only: every `io`/`io::tile`/`io::backend` method returns
`Error::NotImplemented` except `BackendFactory`'s registry (pure bookkeeping) and
`TileLayout`'s grid arithmetic (pure math, touches no bytes). No TIFF/BigTIFF/PDS4/ISIS parsing, no
libtiff calls, and no actual file reads/writes exist yet -- `Reader::open`/`Writer::create` are
still `NotImplemented` stubs and are not yet wired to this layer; that wiring is the next sprint.

## Testing

Catch2 v3 via CTest (`catch_discover_tests`). Real test content across Sprints 1-3: `Result`/
`Error` semantics, `Version` compile-time vs runtime, `Logger` level filtering, static-assert-based
API-surface tests (move-only traits, PIMPL completeness) for the domain types, the full
`ptiff::io::tile` value-type/layout/interface model, `BinaryReader`/`BinaryWriter`,
`StorageModel`, `Serializer`/`Deserializer`, `StorageBackend`/`BackendCapabilities`,
`BackendFactory`, and the seven backend stubs' self-registration.
`integration/`, `golden/`, and `conformance/` are wired but empty -- they need real files that
don't exist yet.

## Non-goals this sprint

No TIFF/BigTIFF/PDS4/ISIS reading or writing, no parsers, no compression, no SPICE, no stereo
algorithms. The storage/I/O layer (`BinaryReader`/`BinaryWriter`, `ImageSource`/`ImageSink`, the
tile model, `StorageModel`, `Serializer`/`Deserializer`, `StorageBackend`, `BackendFactory`, and
the seven backend stubs) exists and compiles but every I/O method returns
`Error::NotImplemented`. `Reader`/`Writer` remain unconnected to this layer by design.
