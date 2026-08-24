# Changelog

Alle bemerkenswerten Änderungen an der PTIFF-Referenzimplementierung (`libptiff`)
und an der PTIFF-Spezifikation werden in dieser Datei dokumentiert.

Das Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/)
und dieses Projekt hält sich an [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
Sofern nicht anders vermerkt, gelten alle Einträge mit einem `Unreleased`-Abschnitt
als noch nicht veröffentlicht.

## [Unreleased]

### Rust-C-ABI-Migration (2026-08-24)

- **`bindings/c/` gelöscht.** Die handgeschriebene `extern "C"`-Veneer
  (`libptiff_c`) ist entfernt. Die C-ABI wird jetzt vollständig von der
  Rust-Crate `crates/ptiff-c` erzeugt: cbindgen autorisiert den Header
  `target/ptiff_c.h` aus den Rust-Signaturen (source of truth), `cargo build
  -p ptiff-c --release` baut `libptiff_c` in `target/release/`.
- **SWIG-Bindings auf die Rust-ABI umgestellt** (`bindings/swig`): `ptiff.i`
  inkludiert den cbindgen-Header statt der 8 Einzel-Header aus `bindings/c`;
  das Makefile liest aus `target/`/`target/release` (sed-gefilterte
  `real_inc/ptiff_c.h`-Kopie für SWIG, direkte `-I`/`-L`/`-Wl,-rpath`-Flags in
  `cgo_flags.go`, kein pkg-config); `bindings/swig/swig_include/`
  (Export-Header-Shim) entfernt.
- **Typemaps auf die Rust-ABI-Typen angepasst**: `size_t`→`uintptr_t`,
  `int`→`int32_t` (Rust `usize`/`i32`), und `struct ptiff_*`-Zeiger in den
  `numinputs=0`-/out-Param-Typemaps (cbindgen emittiert das `struct`-Schlüsselwort).
- **Camera-Timestamp als `[c_char; 64]`** (`crates/ptiff-c`), damit cbindgen
  `char[64]` emittiert — die ABI-Form, die alle vier Sprachen (Go, Python,
  Ruby, Octave) als String zuweisen. Camera-Timestamp-Roundtrip grün in allen
  Suiten.
- **Go-`image.go`/`metadata.go`**: `TileByteSize`→`int64`,
  `bytesRead` als `uint`, `SetTimestamp(string)` (via `char[64]`).
- **Ruby-Versions-Test** auf Rust-1.0.0-Workspace-Version aktualisiert
  (vorher C++-0.3.0).
- **CMake-Build-Systeme bereinigt**: `libptiff/CMakeLists.txt` delegiert
  `PTIFF_BUILD_C_BINDINGS` an `cargo build -p ptiff-c --release` statt an
  `add_subdirectory(bindings/c)`; `bindings/go/CMakeLists.txt` verweist auf
  `target/release` statt auf das alt `ptiff_c`-CMake-Target.
- **Verifiziert**: `make -C bindings/swig test` (Python 14, Go 25, Ruby 14,
  Octave 8 grün), C-ABI-Test `crates/ptiff-c/tests/c/ptiff_c_abiltest.c`
  (ALL OK gegen `target/ptiff_c.h`), `cargo test --workspace` (>500), clippy
  + fmt clean.

## [0.4.2] — 2026-08-22

### Paketierung & CI

- GitHub Actions erzeugen native Installationspakete und veröffentlichen sie als
  GitHub-Release:
  - **Linux** (clang + Ninja): CPack DEB und RPM mit `libptiff`, `libptiff_c`,
    Headern, CMake-/pkg-config-Metadaten und dem `ptiff`-CLI.
  - **macOS** (AppleClang + Ninja): ein `.pkg`-Installer (pkgbuild) für
    `/usr/local` mit den gleichen Bestandteilen.
- **Nix**-Paketbeschreibung ergänzt (`flake.nix` / `default.nix`), gebaut mit
  `clangStdenv` und Ninja.
- Das Rust-CLI (`ptiff-cli`) wird gegen den installierten `libptiff_c`-Prefix
  kompiliert und in die Pakete aufgenommen; der installierte `ptiff`-Binary
  verwendet einen relozierbaren `@loader_path`/`$ORIGIN`-RPATH.
- `CMakeLists.txt`: `LANGUAGES CXX C`, Option `PTIFF_BUILD_CLI`, CPack-Konfiguration
  für DEB/RPM sowie ein `install(SCRIPT)`-Hook, der das CLI nach dessen Cargo-Build
  in das Installationsverzeichnis legt.

## [0.4.0] — 2026-08-22

### Multispektrale Bilder: beliebige `samplesPerPixel`-Bandzahl

Der TIFF-Backend-Schreib-/Lesepfad beschränkte `samplesPerPixel` bisher strikt auf
1 (Graustufen) oder 3 (RGB). Ab 0.4.0 sind 1–512 Bänder zulässig (obere Grenze als
Härtung gegen fehlerhafte/böswillige Eingaben, weit über realer
Multispektral-/Hyperspektral-Sensorik). Motiviert durch bevorstehende
Merkur-Spektrenbilddaten (N Spektralbänder, kein Stereo) als Eingabe für die
Hapke-Shape-from-Shading-Pipeline in `src/sfs`.

- Photometric-Interpretation-Bug behoben: Bandzahlen ungleich 1 wurden zuvor
  fälschlich als RGB (2) getaggt (hätte z.\,B. 5-Band-Daten falsch etikettiert). Nur
  `samplesPerPixel == 3` erhält jetzt RGB; alles andere (inkl. N-Band) BlackIsZero (1).
- Neuer `ExtraSamples`-Tag (338, TIFF 6.0) wird emittiert, sobald die Bandzahl über
  die von der Photometric-Interpretation implizierte Baseline hinausgeht (3 für RGB,
  1 sonst); jedes zusätzliche Band als `0` ("unspecified data", nicht Alpha) markiert.
- Bestehende Golden-SHA-256-Digest-Tests bestätigen byte-identischen Output für
  bestehende 1-/3-Band-Dateien -- kein Format-Bruch für Grau/RGB.
- Neue Tests: 5-Band-Akzeptanz, `ExtraSamples`-Emission (vorhanden bei N-Band,
  abwesend bei Grau/RGB), Ablehnung bei 0 und oberhalb der Obergrenze, 6-Band
  Byte-für-Byte-Write→Read-Roundtrip. 373/373 Tests grün (vorher 369).

### Build-Fixes

- `CMakeUserPresets.json` war Release-only committed, wodurch der dokumentierte
  `cmake --preset conan-debug`-Workflow für einen frischen Checkout kaputt war;
  neu generiert (beide Presets vorhanden).
- `bindings/swig/Makefile`: das `ruby`-Target hatte kein eigenes Rezept, wodurch Make
  die Tab-eingerückten `ifeq`/`else`-Variablenzuweisungen des unabhängigen
  Octave-Blocks fälschlich als Ruby-Rezept übernahm (`LIB_DIR := ...` wurde als
  Shell-Kommando ausgeführt, Build schlug fehl). Tabs vor den Conditional-Zuweisungen
  entfernt -- `make ruby` baut jetzt sauber, Gem-Testsuite grün (20/20).

## [0.3.0] — 2026-08-17

### PTIFF Extension Domänen — Private Tags 65001–65005 (RFC-7002)

Die fünf PTIFF-spezifischen Private-Tags sind jetzt als konkreter TIFF-Tag-Output
implementiert und werden durch den Container-Roundtrip gelesen und geschrieben.
Bis einschließlich 0.2.x existierten diese Domänen nur als In-Memory-Spezifikation und
die Domain-Klassen kehrten `Error::NotImplemented` zurück; ab 0.3.0 trägt der TIFF-IFD
dieselben Metadaten als echte Tags.

- **65001 `PtiffSpice`** — SPICE-abgeleitete Geometrie/Pointing (Frame, Instrument, Zeitsystem, Position).
- **65002 `PtiffCameraGeometry`** — Kameramodell + Intrinsik (Modellbezeichnung, Brennweite, Hauptpunkt).
- **65003 `PtiffCrs`** — Planetares Koordinatenreferenzsystem (Körper-ID, Projektion).
- **65004 `PtiffScientificLayers`** — Abgeleitete Raster-Layer (DEM, Albedo, ...).
- **65005 `PtiffProvenance`** — Verarbeitungshistorie/Provenienz (Software, Operator).

### Neuer, versionierter Metadaten-Payload-Codec

- [`ptiff_metadata.hpp`](libptiff/include/ptiff/io/backend/tiff/ptiff_metadata.hpp): jedes
  Private-Tag trägt ein selbstbeschreibendes, versioniertes Byte-Payload (Magic `PTIFF`,
  Versionsfeld), das eine geordnete Menge von (Schlüssel, Wert)-Rekorden kodiert.
  Deterministisch/kontonisch (Schlüssel sortiert) — gleiche logische Metadaten ergeben
  immer dieselben Bytes, was die Golden-Digest-Eigenschaft erhält.
- Schreibpfad (`planTiffWrite`/`serializeModel`): konvertiert `ptiff.<domäne>.<schlüssel>`
  Felder aus dem `StorageModel` in `FieldType::Byte`-Tags 65001–65005. Modelle ohne
  `ptiff.*`-Felder sind weiterhin byte-identisch zum 0.2.x-Container (bestehender
  Golden-Roundtrip unverändert grün).
- Lesepfad (`interpretTiffIfd`/`deserializeModel`): dekodiert private Tags defensiv in
  `ptiff.<domäne>.<schlüssel>`-Felder; unbekannte/fremde Byte-Blobs oder Vorwärts-Versionen
  scheitern nicht am gesamten Parse (Domäne gilt dann als abwesend). Andere Tipper ignoriert
  private Tags weiterhin (TIFF 6.0-Interop).

### CLI & C-ABI: JSON-Ausgabe der PTIFF-Felder

- C-ABI (`ptiff_image_bridge`): neues `ptiff_open_path_fields`/`ptiff_fields_free` —
  liest die aus den Private-Tags 65001–65005 dekodierten, abgeflachten
  `ptiff.<domäne>.<schlüssel>`-Felder einer Datei zurück (C-Array von Schlüssel/Wert-Paaren).
- Rust-Binding: idiomatisches `open_path_fields()` über die neue C-ABI.
- `ptiff info --json <datei>`: gibt Bild-Metadaten **und** die PTIFF-Domänen-Felder als
  JSON aus, gruppiert unter `"ptiff"` (z. B. `"spice": {"frame": "IAU_MOON"}`) — davon
  bekommt man die in 0.3.0 implementierten Private-Tags direkt auf der Kommandozeile.

### Tests

- Codec-Unit-Tests (`ptiff_metadata_test`): verlustfreier Roundtrip, kanonische Kodierung,
  Ablehnung fremder Byte-Blobs, `ptiff.<domäne>`-Feldabbildung.
- Backend-Roundtrip (`tiff_ptiff_metadata_roundtrip_test`): alle fünf Tags 65001–65005 mit
  vollständigem Feldinhalt durch den TIFF-Container; plain-Dokument ohne `ptiff.*`-Felder
  erzeugt keine Private-Tags.
- Testsuite: 364 → 371 Testfälle, alle grün (inkl. unveränderter 0.2.0-Golden-Roundtrip).

## [0.2.0] — 2026-08-11

### TIFF/BigTIFF Lese-Pfad (real implementiert)

- `TiffBackend` **Lese-Pfad** Ende-zu-Ende: TIFF/BigTIFF-Header-Erkennung, IFD-Parsing,
  Pixel-Format-Auflösung, Directory-Interpretation und Tile-/Strip-Lesen unter
  `libptiff/src/io/backend/tiff/`. Gemeinsamer Einstieg `interpretTiffIfd()` — Header/IFD
  wird nur einmal pro Öffnen geparst.
- `Reader::open`/`Writer::create` sind jetzt echte Fassaden über konkrete Backends
  (nicht mehr `Error::NotImplemented`-Stubs), inkl. `ImageSource`/`ImageSink`-Zugriff auf
  Pixel-Daten über das format-neutrale Tile-Modell.
- **Mehrbild-Dokumente** unterstützt (IFD-Kette in einer Datei): `openImageSourceAt`/
  `openImageSinkAt`, `planTiffWriteMulti`/`serializeModelList`.

### Kompression (TIFF/BigTIFF)

- **LZW** (`Compression=5`) und **PackBits** (`Compression=32773`) mit
  `Predictor = 2` (horizontale Differenzierung) implementiert.
- **Deflate** (`Compression=8`) über zlib implementiert.
- **JPEG** (`Compression=7`, "new-style JPEG") über libjpeg-turbo implementiert —
  Graustufen + RGB/YCbCr, `UInt8`-only, ohne Tiled-Write/Predictor.
- Kompressionen im **Schreib-Pfad** (`SceneSerializer`, `planTiffWrite`) verdrahtet:
  `None`, `LZW`, `PackBits`, `Deflate`, `JPEG`.

### Weitere reale Backends

- Die bisherigen Stub-Backends wurden als echte, nicht-"NotImplemented"-Formate
  umgesetzt: `Pds4Backend` (pugixml-XML-Label + seekbare Roh-Pixel),
  `IsisBackend` (PDS3-artiges Textlabel), `ZarrBackend` (JSON-Header + zstd/zlib-Chunks,
  chunk == tile), `OpenExrBackend` (echte `.exr` über die OpenEXR-C++-API,
  `Float32`/`UInt32`, samplesPerPixel 1/3/4). Alle vier deferieren Multi-Image.
- `MemoryBackend` mit selbstbeschreibendem In-Memory-Format "PMEM" und öffentlichen
  `MemoryBinaryReader`/`MemoryBinaryWriter`-Transporten.
- Erstes Ende-zu-Ende-Read-Beispiel (`examples`/Docs) für den TIFF-Read-Pfad ergänzt.

### Cloud-Object-Storage als Transport (lesend)

- `ptiff::io::HttpRangeBinaryReader`: `BinaryReader`-Transport über HTTP(S) `Range`-Requests
  (libcurl), so dass `TiffBackend` einen Cloud-Optimized TIFF/BigTIFF wie eine lokale Datei
  liest. `Reader::open` wählt per `http://`/`https://`-Prefix automatisch den HTTP-Transport.
  Read-only, read-ahead-Buffer (64 KiB), Auth nur via Presigned-URL/Bearer-Token.

### Robustheit (RFC-0001 §13)

- Gemeinsame Overflow-geschützte Arithmetik-Helfer (`checkedAdd`/`checkedMul`).
- `readTiffIfd` validiert Entry-Tabellen-Bounds und verhindert Integer-Overflow in
  IFD-Offset-/Count-Rechnung; `readDirectoryChain` prüft jeden IFD-Offset gegen die Dateigröße.
- Allokations-Guards (`kMaxTagCount`-Schranke), Strip-/Tile-Tabellen vor Allokation abgewiesen.
- Neues selbstbeschreibendes In-Memory-Format "PMEM" mit Magic-/Version-Prüfung und
  Bounds-/Truncation-/Overflow-Schutz.
- Deterministische Malformed-Input-Test-Suiten (`tiff_parser_hardening_test.cpp`,
  `memory_*_test.cpp`).

### Sprachgebundene Anbindung (Basis)

- `bindings/c` (`libptiff_c`): sprachunabhängige "extern C"-Veneer über die real
  implementierte Kern-API (`ptiff_c::ptiff_c`-Target), die Grundlage für Go/Python/Rust.
- CI-Workflows für C++-Build/Lint sowie Go- und Python-Bindings und `ptiff-cli`.

### Lizenz & Werkzeuge

- `LICENSE`, `LICENSE-SPEC`, `NOTICE`: Apache License 2.0 (Spezifikation + Referenz-
  implementierung); freie Nutzung auch in kommerziellen/Closed-Source-Produkten.
- `CITATION.cff` für Zitier-Metadaten ergänzt.

## [0.1.0] — Foundation, Domain-Modell & Speicher-/I/O-Architektur

### Neu (Sprint 1 — Foundation)

- Von Grund auf neu aufgesetzter C++-Referenz-Stack für die PTIFF-Referenzimplementierung:
  CMake (C++23) + Conan 2 als Paketmanager, mit vollständiger vcpkg-Kompatibilität
  (gleiche Abhängigkeitsmenge in `vcpkg.json` und `conanfile.txt`).
- Öffentliche API-Oberfläche: domain-neutrale Kern-Typen `Result<T>`,
  `Error`/`ErrorCode`, `Version`, `Logger`/`LogLevel`, Preconditions
  (`PTIFF_PRECONDITION`).
- Export-Makros via `generate_export_header()`; versteckte Symbol-Sichtbarkeit für
  spätere Shared Builds vorbereitet.
- Build-Infrastruktur: Compiler-Warnishärtung (warnings-as-error) und Sanitizer-
  Optionen (`PTIFF_ENABLE_ASAN`/`UBSAN`/`TSAN`).

### Neu (Sprint 2 — In-Memory-Domain-Modell)

- Domain-Typen vollständig im Speicher implementiert und in den Build verdrahtet:
  `Scene`, `Image`, `Camera`, `CoordinateReferenceSystem`, `ScientificLayer`,
  `Geometry`, `Annotation`, `Metadata`, `Mission`, `History`.
- `Scene` ist ein echter Objektgraph geworden: Container für `Image`, `Camera`,
  `ScientificLayer`, `Annotation`, `Geometry` sowie Zugriff auf `Metadata`,
  `Mission`, `CoordinateReferenceSystem` und `History` (jeweils mit dichten,
  stabilen `Id<T>`-Zugriffen).
- `Metadata::get`/`set` real implementiert (zuvor `NotImplemented`-Stub).
- `Spice` als eigenständiges Platzhalter-Datenmodell für SPICE-Kernel-Provenienz
  ergänzt (noch nicht in `Scene` eingebunden — bewusst).
- Default-Konstruktoren für `Planet`, `Mission` und `CoordinateReferenceSystem`
  (explizite "unset"-Sentinals für die `Scene`-Voreinstellung).
- Umbrella-Header `ptiff/ptiff.hpp` vervollständigt (selbstständig kompilierbar).
- Umfassende Unit-Test-Abdeckung (~45 Testfälle) für alle Domain- und
  Wert-Typen (inkl. `Vec3`, `Quaternion`, `Extrinsics`, `Intrinsics`,
  `LensModel`, `Ellipsoid`, `Projection`).

### Neu (Sprint 3 — Speicher- & I/O-Architektur)

- Format-neutrale Speicher-/I/O-Architektur: Das Domain-Modell spricht nur über
  `Serializer`/`Deserializer` mit einem `StorageModel`; darunter liegt eine per
  `BackendFactory` wählbare Ebene aus `StorageBackend`s.
- `BinaryReader`/`BinaryWriter` als Byte-Ebene-Transporte.
- Tile-Modell (`Tile`, `TileLayout`, `TileIndex`, `TileExtent`, ...) — Pixel-Daten
  bewegen sich in Tile-Einheiten statt als Ganzbild-Puffer. `TileLayout`-Gitter-
  Arithmetik ist vollständig implementiert.
- Sieben Backend-Stubs, die sich selbst im `BackendFactory`-Registry registrieren:
  TIFF/BigTIFF, PDS4, ISIS3 CUB, Zarr, OpenEXR, in-memory und Cloud-Object-Storage.
  (Alle I/O-Methoden liefern — architektur-bewusst — `Error::NotImplemented`.)
- `Reader`/`Writer` bleiben absichtlich `Error::NotImplemented`-Stubs, solange noch
  kein Dateiformat angebunden ist.

### Geändert

- `ARCHITECTURE.md` und `libptiff/README.md` an den aktuellen Stand
  der Sprints 1–3 angeglichen.

### Neu (nach Sprint 3 — TIFF-Read-Pfad)

- `FileBinaryReader` ergänzt: erster konkreter `BinaryReader` für echte Dateien.
- `TiffBackend` **Lese-Pfad** real implementiert (Stand 23.07.2026): liest echte
  TIFF/BigTIFF-Dateien Ende-zu-Ende über `deserializeModel` und `openImageSource`.
  Interne Parser-Kette (TIFF-Header-Erkennung, IFD-Parsing, Pixel-Format-
  Auflösung, Verzeichnis-Interpretation, Tile-/Strip-Lesen) unter
  `libptiff/src/io/backend/tiff/`, gemeinsamer Einstieg `interpretTiffIfd()`
  (Header/IFD wird nur einmal pro Öffnen geparst).
- Akzeptiert initial Baseline-Kompression `None (=1)`; die Kompressions-Verdrahtung
  (LZW/PackBits/Deflate/JPEG, Predictor) folgte im Release `0.2.0`.
- Umfangreiche Tests für den TIFF-Read-Pfad ergänzt (Header, IFD, Pixel-Format,
  Directory, Image-Source sowie ein End-to-End-Backend-Test).
