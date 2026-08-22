# PTIFF

Planetary TIFF

An open TIFF-compatible scientific image standard for planetary imagery,
stereo vision, GIS, AI and 3D reconstruction.

**Status: `0.4.0`** — die Referenzimplementierung `libptiff` liest und schreibt
TIFF/BigTIFF inklusive Kompression und Mehrbild-Dokumente, bietet mehrere
weitere echte Backends sowie einen Lese-Transport für Cloud-Object-Storage.
Neu in 0.4.0: `samplesPerPixel` unterstützt jetzt beliebige Bandzahlen (1–512,
nicht mehr nur Grau/RGB) für multispektrale Daten, inkl. korrektem
`ExtraSamples`-Tag. Neu in 0.3.0: die PTIFF-spezifischen Private-Tags
**65001–65005** (SPICE, Kamera-Geometrie, CRS, Scientific-Layer, Provenienz)
sind als konkreter TIFF-Tag-Output implementiert und roundtrip-fest getestet.

## Features (Stand 0.4.0)

- **TIFF/BigTIFF lesen und schreiben** (`TiffBackend`): Header/IFD-Parsing,
  Tile-/Strip-Lesen, Mehrbild-Dokumente (IFD-Kette), format-neutrales
  `ImageSink`/`ImageSource`-Tile-Modell.
- **PTIFF Private-Tags 65001–65005** (`RFC-7002`): versionierter Payload-Codec
  für SPICE/Kamera/CRS/Scientific-Layer/Provenienz; `ptiff.<domäne>.*`-Felder am
  `StorageModel` werden in die IFD geschrieben und beim Lesen zurückgewonnen.
  Dokumente ohne diese Felder bleiben mit 0.2.x byte-identisch.
- **Kompression**: LZW, PackBits, Deflate und JPEG (via
  libjpeg-turbo), jeweils mit `Predictor = 2`-Unterstützung wo zutreffend.
- **Weitere echte Backends**: PDS4, ISIS3 CUB, Zarr, OpenEXR und ein
  In-Memory-Backend ("PMEM") — alle als echte Formate, nicht als Stubs.
- **Cloud-Object-Storage (lesend)**: `HttpRangeBinaryReader` liest ein
  Cloud-Optimized TIFF/BigTIFF über HTTP(S) `Range`-Requests (libcurl);
  `Reader::open` wählt per `http://`/`https://` automatisch den Transport.
- **Robustheit**: Parser-Härtung gegen missgebildete Dateien (RFC-0001 §13),
  Overflow-geschützte Arithmetik und Allokations-Guards.
- **Anbindung**: `libptiff_c` (sprachunabhängige "extern C"-Veneer) als
  Grundlage für die Go-/Python-Bindings und die Rust-CLI. Darauf baut ein
  **MCP-Server** ([`bindings/mcp`](./bindings/mcp)) auf, der PTIFF als
  LLM-Werkzeuge (Model Context Protocol) über stdio bereitstellt — z. B. für
  Claude Code.

Architektur und Design-Rationale: `ARCHITECTURE.md` und `CHANGELOG.md`.

## Build & Test

C++23-Compiler + CMake >= 3.26. Conan 2 ist der primäre
Dependency-Manager; vcpkg ist eine vollwertige Alternative
(`conanfile.txt` / `vcpkg.json` beschreiben dieselbe Abhängigkeitsmenge).

```bash
# Conan (primärer Weg)
conan profile detect --force
conan install . --build=missing -s build_type=Debug -s compiler.cppstd=23
cmake --preset conan-debug
cmake --build --preset conan-debug
ctest --preset conan-debug --output-on-failure

# vcpkg-Alternative
cmake -B build -S . -DCMAKE_TOOLCHAIN_FILE=$VCPKG_ROOT/scripts/buildsystems/vcpkg.cmake
cmake --build build
ctest --test-dir build --output-on-failure
```

Ein einzelnen Test ausführen (Catch2 v3 via CTest):

```bash
ctest --preset conan-debug -R <test-name-regex> --output-on-failure
# oder direkt über die Catch2-Binary für Tag-/Name-Filter:
./build/Debug/libptiff/tests/unit/ptiff_unit_tests "<test name or [tag]>"
```

## License

PTIFF is **free to use**: the specification (`LICENSE-SPEC`) and the
reference implementation `libptiff` (`LICENSE`) are both released under the
[Apache License, Version 2.0](https://www.apache.org/licenses/LICENSE-2.0).
You may use, modify, extend, and redistribute them, including in commercial
and closed-source products. See [`LICENSE`](LICENSE) and
[`LICENSE-SPEC`](LICENSE-SPEC) for the full terms.

## Documentation

API documentation is generated with [Doxygen](https://www.doxygen.nl/)
(`doxygen >= 1.9`). The config lives in the repo-root `Doxyfile`; the
hand-written overview pages are under `docs/doxygen/`.

Build it one of two ways:

```bash
# standalone (primary way)
doxygen Doxyfile
# output: build/docs/html, build/docs/xml

# or as a CMake target (requires CMake configure)
cmake --build build --target docs
```

The reference pages are generated **from the `///` doc comments** in
`libptiff/include/`, so keep those in sync when you change the public API.
