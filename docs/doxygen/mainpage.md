# PTIFF -- API Documentation {#mainpage}

<center>
**PTIFF** — *Planetary Terrain-sensor Image File Format — reference implementation*
</center>

`libptiff` is a from-scratch C++23 reference implementation of the **PTIFF**
standard (see RFC-0001 in `rfcs/RFC-0001-Core.md`). It provides a
format-neutral, extensible image I/O library for planetary-science imagery,
with pluggable backends for TIFF/BigTIFF, PDS4, ISIS3 CUB, Zarr, OpenEXR,
in-memory and cloud object stores.

> **Status note.** The codebase is still architecture-first. The domain model
> (`Scene`, `Image`, `Metadata`, `Camera`, `CoordinateReferenceSystem`, ...)
> and the storage/I/O layer exist and compile, but most I/O verbs return
> `Error::NotImplemented` until the backends land. See
> @ref md_ARCHITECTURE "ARCHITECTURE.md" for the current sprint reality.

## Contents

@subpage core_model
@subpage io_architecture

## Getting started

```bash
conan profile detect --force              # first time only
conan install . --output-folder=build --build=missing
cmake -B build -S . -DCMAKE_TOOLCHAIN_FILE=build/conan_toolchain.cmake
cmake --build build
ctest --test-dir build --output-on-failure
```

The public API is pulled in through the umbrella header
`#include <ptiff/ptiff.hpp>`.

## How to use this documentation

- **Modules** — high-level walkthroughs under [Related Pages](pages.html) /
  in the navigation tree (Classes, Files, Namespaces).
- **Classes / Files** — the full reference for every public type and header,
  auto-generated from the Doxygen comments in the code.
- **Related pages** — prose documentation, coding guidelines and design specs.

The reference is generated **from the code comments**, so when you change the
public API, keep the `///` doc comments in `libptiff/include/` up to date and
rebuild with:

```bash
doxygen Doxyfile            # or: cmake --build build --target docs
```

## Project layout

| Path                      | Contents                                      |
| ------------------------- | --------------------------------------------- |
| `libptiff/include/ptiff/` | Public headers (the documented API)           |
| `libptiff/src/`           | Private implementation, incl. `ptiff::detail` |
| `libptiff/tests/`         | Catch2 v3 unit/integration tests              |
| `rfcs/`                   | PTIFF standard RFCs                           |
| `specification/`          | Normative format specification                |
| `docs/`                   | Design specs, plans and this documentation    |

## Namespaces

The public API lives in the root `ptiff` namespace:

- `ptiff` — domain model (`Scene`, `Image`, `Metadata`, `Camera`,
  `CoordinateReferenceSystem`, `ScientificLayer`, ...), `Result`/`Error`, `Logger`.
- `ptiff::io` — serializers, storage model, `ImageSource`/`ImageSink`,
  binary transports, backends.
- `ptiff::io::tile` — the tile model (`Tile`, `TileLayout`, tile storage).
- `ptiff::io::backend` — concrete format backends (`TiffBackend`,
  `MemoryBackend`, `Pds4Backend`, `IsisBackend`, `ZarrBackend`,
  `OpenExrBackend`). Remote objects are read via the
  `HttpRangeBinaryReader` *transport*, not a dedicated backend.
- `ptiff::detail` — _private_, implementation details. **Not** part of the
  public API and excluded from this documentation.

## Feedback

Found a bug in the docs? The API docs are generated from `libptiff/include/`;
file an issue with the relevant header and symbol.
