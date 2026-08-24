# PTIFF 1.0 – Rust Core Architecture & Migration Planning

**Status:** Entwurf / Planungsdokument (Rev. 2)
**Stand:** 2026-08-22
**Geltungsbereich:** Die Analyse, Zielarchitektur, Rust-Crate-Struktur, Dependency-Evaluation,
Parallelisierung, C-ABI, C++/Python/Octave-Strategie, Test-/Benchmark-/Build-Strategie,
Migrationsphasen, Risiken und die Definition von PTIFF 1.0.
**Rev. 2 (2026-08-22):** Konsolidierung des architectural-response-Inputs als §3.1
(Leitprinzipien: Rust als einziger maintained Core, Plattform-Unabhängigkeit, Dependency-
Triage, Rust-ABI-intern, C++ als Consumer, Zero-copy-Ownership, Portability/Performance/
Interoperability) sowie Abgleich mit `ROADMAP.md` als §17.0 (M0–M4 ↔ Migrationsphasen,
zwei-Sichten-Modell) und Plattform-/Packaging-Ziele in §16 (16.2a, 16.5).

> **Dies ist ein reines Planungsdokument.** Es verändert weder bestehende Implementierung,
> APIs, Bindings noch Build-Konfiguration. Ein Entwicklerteam soll auf Basis dieses Dokuments
> mit der Implementierung beginnen können, ohne die grundlegenden Architekturentscheidungen
> noch einmal treffen zu müssen.

> **Update 2026-08-24 (Stufen 2+3 der C-ABI-Migration umgesetzt):** Die früheren Abschnitte
> zur C-ABI lesen sich teils so, als sei `bindings/c/` die Single Source of Truth und cbindgen
> dürfe nur ergänzend laufen. **Das ist nicht mehr der Fall.** `bindings/c/` wurde **gelöscht**
> (ebenso der `bindings/swig/swig_include/`-Export-Shim). Die C-ABI ist jetzt **rein Rust**:
> `crates/ptiff-c` implementiert die `extern "C"`-Signaturen, cbindgen autorisiert den Header
> `target/ptiff_c.h` daraus (die neue Single Source of Truth), und alle SWIG-Bindings
> (Python/Go/Ruby/Octave) bauen gegen diesen generierten Header + das Cargo-gebaute
> `libptiff_c` in `target/release/`. Historische Erwähnungen von `bindings/c/*.h` im Rest des
> Dokuments beschreiben den Zwischenstand und sind als solche zu lesen.

---

## Inhaltsverzeichnis

1. [Current Architecture](#1-current-architecture)
2. [Problems in Current Architecture](#2-problems-in-current-architecture)
3. [Target Architecture](#3-target-architecture)
4. [Rust Crate Architecture](#4-rust-crate-architecture)
5. [Dependency Evaluation](#5-dependency-evaluation)
6. [Parallelization Strategy](#6-parallelization-strategy)
7. [C ABI Design](#7-c-abi-design)
8. [C++ Strategy](#8-c-strategy)
9. [Python Strategy](#9-python-strategy)
10. [Octave Strategy](#10-octave-strategy)
11. [Testing Strategy](#11-testing-strategy)
12. [Benchmark Strategy](#12-benchmark-strategy)
13. [Build Strategy](#13-build-strategy)
14. [Scientific / Planetary Data Requirements](#14-scientific--planetary-data-requirements)
15. [Build / Performance-Hinweise](#15-build--performance-hinweise-zu-ptiff-10)
16. [PTIFF 1.0 Definition](#16-ptiff-10-definition)
17. [Migration Phases](#17-migration-phases)
18. [Weitere technische Entscheidungen](#18-weitere-technische-entscheidungen-konsolidiert)
19. [Risiken](#19-risiken)
20. [Open Questions](#20-open-questions)
21. [Zusammenfassung / Fazit](#21-zusammenfassung--fazit)

---

# 1. Current Architecture

## 1.1 Repository-Gesamtbild

Das Repository ist deutlich weiter fortgeschritten, als `ARCHITECTURE.md` (Stand Sprint 3)
vermuten lässt. Der tatsächliche Stand umfasst eine vollständige C++23-Referenzimplementierung
`libptiff` mit **funktionierenden** TIFF/BigTIFF-Read/Write-Pfaden, mehreren realen Backends,
Kompression, HTTP-Range-Transport, einem C-ABI-Layer (`libptiff_c`) und Bindings für **Python,
Go, Ruby, Octave, Rust** sowie einem Rust-CLI (`ptiff-cli/`).

**Wichtiger Befund:** Das Kern-Entscheidungsmuster ist bereits etabliert:

```
C++-Core (libptiff)  →  C-ABI (libptiff_c, bindings/c)  →  Sprach-Bindings
                                                              ├── Rust (bindgen)
                                                              ├── Python (SWIG)
                                                              ├── Go (SWIG / SWIGcgo)
                                                              ├── Ruby (SWIG)
                                                              └── Octave (SWIG/MEX)
```

Die C-ABI-First-Architektur, die für PTIFF 1.0 angestrebt wird, existiert also **bereits** –
nur mit C++ als Implementierung des Cores. Die Migration ersetzt den Core-Teil, ohne das
ABI-Versprechen der Sprachen anzutasten.

## 1.2 Schichten im Detail

| Ebene | Pfad | Technologie | Status |
|-------|------|-------------|--------|
| C++-Core-Bibliothek | `libptiff/` | C++23, CMake, PIMPL | Voll funktionsfähig (v0.4.2) |
| C-ABI | `bindings/c/` | handgeschriebene `extern "C"`-Bridge (`ptiff_*` Funktionen) | Funktionsfähig |
| Rust-Sprachbindung | `bindings/rust/` | rust-bindgen über C-ABI | Promoted, offiziell |
| Python-Binding | `bindings/python/` | SWIG `-python` über C-ABI | Promoted, offiziell |
| Go-Binding | `bindings/go/` | SWIG `-go -cgo` über C-ABI | Promoted, offiziell |
| Ruby-Binding | `bindings/ruby/` | SWIG `-ruby` über C-ABI | Promoted, offiziell |
| Octave-Binding | `bindings/octave/` | SWIG `-octave` (C++-MEX) über C-ABI | Promoted, offiziell |
| CLI | `ptiff-cli/` | Rust, nutzt `bindings/rust` | Funktionsfähig |
| Benchmarks | `benchmarks/` | pro-Sprache + Rust-Microbench | Funktionsfähig |
| SWIG-Quellen | `bindings/swig/` | `ptiff.i` + `typemaps.i` | Geteilt, treibt 4 Sprachen |

## 1.3 C++-Core `libptiff` – Modulaufbau

```
libptiff/include/ptiff/
  core/          result.hpp, error.hpp, id.hpp, version.hpp, precondition.hpp
  logging/       logger.hpp (spdlog hinter PIMPL)
  io/            reader.hpp, writer.hpp, binary_reader/writer.hpp
                 serializer/deserializer.hpp, storage_model.hpp
                 storage_backend.hpp, backend_factory.hpp
                 image_source.hpp, image_sink.hpp, tile_provider.hpp
                 tile/ tile_index, tile_layout, tile_region, tile_extent, tile,
                       tile_cache, tile_storage, tile_iterator
  io/backend/    tiff/, pds4/, isis/, zarr/, openexr/, memory/
                 tiff_backend, pds4_backend, isis_backend, zarr_backend,
                 openexr_backend, memory_backend
  image/         image_descriptor.hpp, compression_kind.hpp, pixel_type.hpp,
                 tile_info.hpp
  geometry/      camera.hpp, coordinate_reference_system.hpp, lens_model.hpp,
                 projection.hpp, ...
  metadata/      metadata.hpp, scientific_layer.hpp, history.hpp, mission.hpp
  scene.hpp, annotation.hpp
```

**Kern-Architekturmerkmale:**

1. **Format-neutraler StorageModel-Layer.** Das öffentliche Domain-Modell (`Scene`, `Image`,
   `Camera`, …) spricht nur mit `Serializer`/`Deserializer`, die in ein formatneutrales
   `StorageModel` überführen. Darunter ist alles ein `StorageBackend`.
2. **`StorageBackend` als einzige Erweiterungslinie.** Neue Formate unter
   `ptiff::io::backend::*` implementieren `openImageSource`, `openImageSink`,
   `deserializeModel`, `serializeModel` und registrieren sich über die `BackendFactory`-Registry
   (name-based).
3. **`Result<T> = std::expected<T, Error>` als einziger Fehlerpfad.** Keine Exceptions über die
   öffentliche API für erwartbare Fehler; `PTIFF_PRECONDITION` nur für Programmierfehler.
   `ErrorCode` ist eine wachsende, stabile Enumeration.
4. **TIFF/BigTIFF-Implementierung als `TiffBackend`.** Eigene IFD-/Tag-Parser (klassisch
   12-Byte, BigTIFF 20-Byte), eigene Directory-/Header-Writer, Endianness-Handling. Die fünf
   PTIFF-Private-Tags 65001–65005 mit versioniertem `PTIFF`-Payload-Codec (RFC-7002).
5. **Kompression.** `packbits.cpp`, `lzw.cpp`, `predictor.cpp` (eigene, interne
   Implementierung), `deflate.cpp` (zlib), `jpeg.cpp` (libjpeg-turbo). ZSTD nur im Zarr-Backend.
6. **Transport.** `FileBinaryReader`/`Writer` (lokal), `MemoryBinaryReader/Writer`
   (in-memory „PMEM“), `HttpRangeBinaryReader` (libcurl, 64-KiB-Read-Ahead, „not thread-safe,
   single-cursor“).
7. **Parallelität.** **Derzeit de facto nicht vorhanden.** `TiffImageSource::readTile` ist
   synchron, sequentiell, pro-Handle ein Cursor. `StorageBackend` ist „thread-compatible“ (eine
   Instanz pro Datei, kein Sharing). `BinaryReader` hält einen mutablen Cursor. Das ist der
   größte architektonische Engpass für skalierte große TIFFs.

## 1.4 Modul-Kategorien (Bestandsaufnahme)

Ordnet jedes wichtige Modul einer der 10 Zielkategorien zu.

| Modul | Kategorie |
|-------|-----------|
| `ptiff::core` (Result, Error, Version) | 1. Rust Core – direkt portierbar (`Result<T>`, `ErrorCode`) |
| `ptiff::logging::Logger` | 1. Rust Core – portierbar |
| `ptiff::io::tile::*` (Index/Layout/Region/Extent) | 1. Rust Core – reine Mathematik, ideal portierbar |
| `ptiff::io::StorageModel` / `Serializer`/`Deserializer` | 1. Rust Core |
| `ptiff::io::StorageBackend` / `BackendFactory` | 1. Rust Core (als Trait + Registry) |
| `ptiff::io::tiff::*` (Header/IFD/Tag/Directory) | 1. Rust Core – TIFF/BigTIFF-Logik |
| `ptiff::compression::{lzw, packbits, predictor}` | 1. Rust Core – interne Implementierungen |
| `Image`, `Scene`, `Camera`, `CRS`, `Metadata`, `ScientificLayer` | 1. Rust Core |
| `bindings/rust/` | 1. Rust Core (wird zum idiomatischen Rust-API des Cores) |
| LZW / PackBits | 1. Rust Core |
| Deflate | 2. Rust Dependency (`flate2`) |
| JPEG | 2. Rust Dependency über libjpeg-turbo-Bindings (siehe §5) |
| ZSTD | 2. Rust Dependency (`zstd` crate) |
| `zlib` | 2. Rust Dependency (über `flate2`-Backend) |
| `libjpeg-turbo` | 3. C/C++ Dependency (über Bindings; simd-optimiert) |
| `libcurl` (HTTP-Range) | 3. C/C++ Dependency → ersetzbar durch `ureq`/`reqwest` (2) |
| `pugixml` (PDS4-XML) | 3. C/C++ Dependency → ersetzbar durch `quick-xml` (2) |
| `nlohmann_json` (Zarr) | 3. C/C++ Dependency → ersetzbar durch `serde_json` (2) |
| `openexr` | 3. C/C++ Dependency → ersetzbar durch `exr`/`openexr-rs` |
| `spdlog` | 3. C/C++ → ersetzbar durch `tracing`/`log` (2) |
| `fmt` | 3. C/C++ → ersetzbar durch Rust `format!` |
| `bindings/c` (`ptiff_*` C-ABI) | 4. C ABI (bleibt, wird Ziel des Rust-Cores) |
| `bindings/rust` | 1. Rust Core / 4. C ABI (FFI) |
| `bindings/octave` | 7. Octave Binding (SWIG-MEX über C-ABI) |
| `bindings/python` | 6. Python Binding (SWIG über C-ABI, evtl. PyO3) |
| `bindings/go`, `bindings/ruby` | 6. andere Sprachbindings (über C-ABI) |
| `ptiff-cli/` | 8. CLI (Rust) |
| `benchmarks/` | 9. Test Infrastructure |
| `tests/{unit,integration,golden,conformance}` | 9. Test Infrastructure |
| PDS4 / ISIS3-Backends | 1. Rust Core (I/O-Logik) + 2. (XML) |
| RFCs / `specification/` | Dokumentation/Normativ, sprachunabhängig |
| `conformance/`, `scripts/interop*` | 9. Test Infrastructure (Cross-Language) |

---

# 2. Problems in Current Architecture

## 2.1 Architektur-Probleme (funktional)

1. **Keine Parallelisierung.** Tile-Decompression und Tile-Writes sind strikt sequentiell
   (single-cursor `BinaryReader`, thread-compatible nicht thread-safe). Für die
   planmäßig "große, unabhängige Tile-Mengen" (§6) fehlt jede Skalierung.
2. **C++-Core als Flaschenhals für Sprachen.** Obwohl der C-ABI-Pfad existiert, ist der Core
   selbst in C++ verfasst; jede C++-API-Änderung hat ABI-Konsequenzen für die Bindings, und der
   Core ist nicht `Send`-freundlich.
3. **Wissens-/Dependency-Last in C++.** LZW/PackBits/Predictor sind handgeschrieben in C++;
   JPEG/OpenEXR/PDS4/Zarr binden C-Bibliotheken (libjpeg-turbo, OpenEXR, pugixml, nlohmann_json,
   libcurl). Das erzeugt einen großen, langsamen Build- und Betriebsfußabdruck.
4. **Thread-Safety-/Ownership-Modell** ist im C++-Kern uneinheitlich (einige Typen move-only,
   `BinaryReader` single-cursor, `Sequence` nicht thread-safe). Das macht paralleles
   Arbeiten schwierig und fehleranfällig.
5. **Build-Geschwindigkeit.** Der C++-Build ist langsam – siehe §2.2.

## 2.2 Warum der aktuelle Build langsam ist (konkret – nicht „weil C++“)

Die Ursachen sind konkret identifizierbar:

1. **Header-schwere Template-Bibliotheken in nahezu jeder TU.**
   - `fmt`, `spdlog` (die hinter PIMPL reingehalten werden, aber beim Kompilieren der
     `SpdlogLogger`-TU trotzdem voll instanziiert werden),
   - `nlohmann_json` (header-only, Zarr-Backend),
   - je nach Einbindung `pugixml`, `jpeglib`, `zlib.h`, `zstd.h`.
   Jede `.cpp`-TU inkludiert einen großen Teil davon → hohe Ausgangs-Compile-Zeit.
2. **~60 C++-Übersetzungseinheiten** in `libptiff` (viele kleine `.cpp`-Dateien, jede mit
   eigener Precompiled-guard-Prüfung, Optimierung des gesamten DAGs).
3. **C++23-Optimierung.** `std::expected`/Stacks, PIMPL-Indirektion und strikte Generika in
   Debug+Release.
4. **Conan-Build von „missing“ Dependencies aus dem Quellcode** beim ersten Configure:
   `openexr` (sehr groß), `libcurl`, `libjpeg-turbo`, `zstd`, `libdeflate` – diese werden
   nicht selten neu gebaut, was Minuten dauert.
5. **Fehlende inkrementelle Cache-Politik** über die CI-Matrix (4 OS × 2-3 Compiler); jede
   Wiederausführung rekompiliert die deps oder große Teile.
6. **Keine Trennung von „Core“ und „optional“.**
   `catch2` (test-only), `cpp-httplib` (test-only) und Kern-Targets sind in einem CMake-DAG.

**Beseitigt eine Rust-Architektur diese Ursachen? Ja, teilweise, aber nicht automatisch:**

- **Rust kompiliert nicht per se schneller** als C++. Der Vorteil entsteht nur anderenfalls:
  - **Cargo baut nur die benötigten Features.** Optionale Crates (HTTP, OpenEXR, PDS4…) sind
    hinter Cargo-Features – sie werden nur kompiliert, wenn aktiviert. Das beseitigt Ursache 6
    und die Hälfte von Ursache 1 (nicht-aktivierte Backends werden gar nicht erst gebaut).
  - **Cargo's `--release`, `sccache`, `cargo-nextest`** verbessern die inkrementelle Dev-/CI-
    Schleife. Die Abhängigkeiten als **prebuilt
    Crates** (flate2, serde_json, quick-xml, ureq) sind meist reine Rust-Crates und kompilieren
    schnell; teure C-Bibliotheken (openexr, libcurl) entfallen oder werden durch kleinere
    Rust-Alternativen ersetzt (Ursache 4 teilweise).
  - **Trennung in `libptiff-core` vs. `ptiff-c` vs. bindings** in einem Cargo-Workspace: nur die
    gelinkten Crates werden gebaut. Wer nur TIFF+C++ braucht, baut nicht OpenEXR/HTTP mit.
  - **Inkrementelle Caches** (sccache, git-CI-Cache, `cargo` target caching) sind in Rust CI
    einfacher reproduzierbar.

**Dennoch:** Der größte zeitliche Posten im aktuellen Build ist die **Conan-Erstabnahme von
OpenEXR/libcurl**. Eine Rust-Migration ersetzt diese durch kleinere Crates (→ deutlich schneller),
aber ein reiner Rust-Core mit `image`/`tiff`-Dependencies hätte seinen eigenen (kleineren)
Compile-Fußabdruck. Das Ziel ist **nicht** „Rust > C++“, sondern „weniger zu kompilieren, weil
Cargo-Feature-Gating + kleinere reine-Rust-Crates“.

---

# 3. Target Architecture

Die Ausgangsskizze ist richtig und deckt sich mit dem bereits etablierten Muster. Ich behalte
sie mit einer wichtigen Präzisierung bei:

- **`ptiff-core` (Rust) wird der einzige Core.** Kein zusätzlicher Wrapper.
- **Die C-ABI ist die einzige Interoperabilitäts-Schicht** zwischen dem Rust-Core und allen
  Nicht-Rust-Clients.
- **Rust-Sprachbindung wird das idiomatische Frontend direkt auf `ptiff-core`** (kein Umweg
  über C-ABI).
- **C++ wird ein echter Wrapper auf der C-ABI**, NICHT mehr der Core.
- **Die bestehende C++-Implementierung wird nicht in der Zielarchitektur enthalten sein** –
  sie wird während der Migration als Referenz/Oracle beibehalten (siehe §11), ist aber in
  der 1.0-Zielarchitektur kein Bestandteil mehr.

```
                     ┌─────────────────────────────────────────────┐
                     │              PTIFF 1.0                       │
                     └─────────────────────────────────────────────┘
                                     │
                     ┌───────────────▼──────────────┐
                     │          ptiff-core           │   Rust, der einzige Core
                     │  (Format, IFD, Tags, Pixels,   │   (typsicher, safe, Send+Sync)
                     │   Kompression, Backends)        │
                     └───────────────┬──────────────┘
                                     │
                     ┌───────────────▼──────────────┐
                     │        stable C ABI           │   die einzige Sprach-Interop
                     │     (libptiff_c / ptiff-*)    │   (opaque handles, C89-kompatibel)
                     └───────────────┬──────────────┘
                                     │
        ┌─────────────┬──────────────┼──────────────┬───────────────┬───────────────┐
        ▼             ▼              ▼              ▼               ▼               ▼
       C            C++            Python        Octave          Go/Ruby      andere
     (roh)      (ptiff-cpp)      (PyO3)        (MEX über C-ABI) (SWIG)        (C-ABI)
                                     │
                                   NumPy
                    ┌─────────────────────────────┐
                    │       ptiff-core (Rust)      │ ← idiomatisches Rust-Frontend,
                    │     → ptiff (crate)          │   NUR Rust→Rust, kein C-ABI
                    └─────────────────────────────┘
```

**Begründung der Abweichungen gegenüber der Skizze:**

1. **PyO3 statt SWIG für Python** – siehe §9. Leichtgewichtiger, besserer NumPy-/zero-copy-Fluss,
   keine C++-Runtime in Python. Während der Migration bleibt der SWIG-Python-Pfad der Rückfall;
   das 1.0-Ziel ist PyO3.
2. **`ptiff-core` + `ptiff` (idiomatisch) getrennt.** Der Core ist NICHT an die C-ABI gebunden
   (kein `#[no_mangle]` im Core). Die C-ABI ist ein separater Crate, der Core-Objekte über
   opaque-Handles exponiert. Das hält den Core testbar, fuzzbar und rein.
3. **C++ wird komplett neu als Wrapper.** Die bestehende C++-Implementierung lebt während der
   Migration als Oracle weiter, ist aber nicht Teil der 1.0-Zielarchitektur.

## 3.1 Architektonische Leitprinzipien (konsolidiert)

Die folgende Position ist expliziter Bestandteil der Zielarchitektur von PTIFF 1.0 und
übernimmt die Kernaussagen des Architectural-Response-Inputs.

### 3.1.1 Rust ist die Referenz- und die einzige maintained Core-Implementierung

> **Rust ist die Referenzimplementierung und der einzige maintained PTIFF-Core.**

Die bestehende C++-Implementierung wird während der Migration als Referenz und
Kompatibilitäts-Oracle gepflegt (siehe §11), gehört aber **nicht** zur finalen
1.0-Core-Architektur. Nach 1.0 ist sie archiviert und wird nicht weiter entwickelt.
Damit verlagert sich die zentrale Änderung von einem Neuentwurf zu: **die Implementierung
des Cores hinter der bereits bestehenden C-ABI-Grenze von C++ nach Rust zu verschieben.**

### 3.1.2 Plattform-Unabhängigkeit als First-Class-Ziel

Die Rust-Migration wird **nicht primär als Sprachwechsel oder Performance-Wechsel** begründet.
Ihr stärkster Mehrwert ist die **Reduktion der plattformspezifischen Build-/Dependency-Fläche**:

```text
heute (C++)                                    Ziel (Rust)
C++23                                            ptiff-core
 ├── CMake                                        ├── plattform-neutraler Rust-Kern
 ├── Conan                                        └── isolierte FFI-Abhängigkeiten
 ├── zlib / libdeflate
 ├── libjpeg-turbo
 ├── libcurl
 ├── OpenEXR
 ├── pugixml
 └── weitere native Deps
```

> **Ziel:** plattformspezifische native Abhängigkeiten minimieren und die verbleibenden
> hinter klar definierten Schnittstellen isolieren (eigene, kleine FFI-Fläche).

### 3.1.3 Nicht „Rust überall“, sondern „Rust wo es sinnvoll ist“

**Dependency-Triage** – für jede Abhängigkeit entscheiden:

```text
Kann es sicher & effizient in Rust implementiert werden?
        │
        ├── Ja → Rust bevorzugen
        │
        └── Nein
             │
             ▼
     Braucht es eine reife native Bibliothek
     für Kompatibilität oder Performance?
             │
             ├── Ja → hinter FFI isolieren
             │
             └── Nein → Abhängigkeit überdenken
```

Konkret: TIFF/BigTIFF, LZW, PackBits, Predictor → Rust; Deflate → `flate2`; JSON →
`serde_json`; XML → Rust-XML-Crate; HTTP → Rust (`ureq`); parallel → Rayon; **JPEG →
`libjpeg-turbo` über eine sehr kleine FFI-Grenze** (Determinismus/Performance); CSPICE →
optional isoliertes FFI.

### 3.1.4 Die C-ABI ist die Plattform-Grenze; die Rust-ABI nie

PTIFF 1.0 unterscheidet explizit zwischen dem Rust-API (`ptiff-core`) und der Sprach-Grenze
(`ptiff-c`). **Die Rust-ABI selbst darf niemals zur öffentlichen Kompatibilitäts-Grenze werden.**
Nur die C-ABI ist stabil, dokumentiert, versioniert, opaque-handle-basiert, explizit über
Ownership & Memory-Lifetime, dort, wo versprochen, thread-safe, und unabhängig von der
C++- sowie der Rust-ABI.

### 3.1.5 C++ wird Consumer, nicht Implementierung

Eine C++-Applikation erhält weiterhin eine native-feelende C++-API (`ptiff-cpp`), aber die
PTIFF-Implementierung selbst ist unabhängig von C++. Damit werden der C++-Compiler und die
C++-Standardbibliothek **nicht mehr benötigt, um PTIFF selbst zu implementieren**. Die
bestehende C++-Implementierung bleibt nur als temporärer Oracle- und Migrations-Pfad.

### 3.1.6 Zero-copy & Memory-Ownership sind Architektur-Requirement

Für große planetare Daten ist Speicherbewegung so wichtig wie CPU-Performance. PTIFF 1.0
definiert explizit: Ownership, Buffer-Lifetime, geborgte vs. eigene Daten, Tile-Speicher,
FFI-Allokation/Free-Semantik, NumPy-Interop, Octave-Array-Interop. Der gewünschte Datenpfad
liegt so nah wie möglich an:

```text
          PTIFF tile
              │
              ▼
          Rust buffer
           /       \
          /         \
      Python       Octave
      NumPy         MEX
```

statt dieselben (potenziell hunderte von MB großen) Bilddaten mehrfach durch Sprach-Schichten
zu kopieren. Das ist Architektur-Requirement, keine spätere Optimierung.

### 3.1.7 Parallelität folgt dem natürlichen Datenmodell

Die Tile-Offsets des TIFF sind unveränderlich und bilden eine natürliche
Parallelisierungs-Grenze (siehe §6). Die erste Implementierung bleibt **synchron** im Kern;
Parallelität wird dort eingeführt, wo das Datenmodell es natürlich begünstigt (Rayon für
CPU-gebundene Tile-Verarbeitung), statt einen Async-Runtime nur um der Konkurrenz willen in
den Kern zu bringen.

### 3.1.8 Die drei primären Architektur-Ziele

```text
        PTIFF 1.0
            │
    ┌───────┼────────┐
    ▼       ▼        ▼
Portability Performance Interoperability
    │       │        │
  Rust    Rayon    C ABI
    │       │        │
    └───────┼────────┘
            ▼
      Scientific PTIFF
```


---

# 4. Rust Crate Architecture

## 4.1 Cargo-Workspace-Struktur

```
ptiff/                          (Workspace-Root; Cargo.toml [workspace])
├── crates/
│   ├── ptiff-core/             der Kern (kein C-ABI, keine externen Pflicht-System-Deps)
│   ├── ptiff-c/                cdylib + staticlib C-ABI (handgepflegte C-Header als SOT)
│   ├── ptiff-rust/             (Paketname: ptiff) ideomatische Rust-API auf ptiff-core
│   ├── ptiff-cli/              CLI (bestehendes ptiff-cli/ hierher, koppelt ptiff-core)
│   ├── ptiff-python/           PyO3-Modul (maturin), sagt ptiff-core
│   └── ptiff-cpp-sys/ (optional) C++-Wrapper baut gegen ptiff-c
├── bindings/
│   ├── octave/                 bestehend OK, gegen ptiff-c
│   ├── go/  ruby/              bestehende SWIG-Bindings gegen ptiff-c (unverändert)
│   └── c/                      hand-gepflegte stabile C-Header  (= source of truth der ABI)
├── tests/                      goldene + Cross-Language-Interop-Tests
├── conformance/                Konformitätsstufen
└── benchmarks/                 (einheitlicher Benchmark-Harness)
```

## 4.2 Was gehört in `ptiff-core`?

- `Error` / `ErrorCode` (stabile, wachsende Enum), `Result<T>` (Rust-native)
- Logging-Abstraktion (Trait, hinter der `tracing` liegt)
- Format-neutrales `StorageModel`, `Serializer`/`Deserializer`
- Tile-Modell (`TileIndex`, `TileLayout`, `Tile`, `TileProvider`) – numerisch identisch zu C++,
  so dass Golden-/Roundtrip-Tests exakt übernommen werden können
- `StorageBackend` (Trait) + `BackendFactory` (Registry, `Arc<Box<dyn StorageBackend>>`)
- `BinaryReader`/`BinaryWriter` Trait
- **TIFF/BigTIFF-Backend** (Header, IFD, Tag-Parser, Directory, PTIFF-Private-Tags
  65001–65005, RFC-7002-Payload-Codec)
- Kompressions-Codecs: LZW, PackBits, Predictor (intern), Deflate (flate2), JPEG
  (libjpeg-turbo), bei Zarr zusätzlich ZSTD
- Domain-Typen: `Image`, `Scene`, `Camera`, `CRS`, `Metadata`, `ScientificLayer`, `History`,
  `Mission`
- GeoTIFF-/Planeten-CRS-Basisfunktionalität (OGC-Identifikatoren, Projektions-Basis)
- Tests, Property-Tests, Fuzz-Targets

## 4.3 Was gehört NICHT in `ptiff-core`?

- **Nicht:** C-ABI (`#[no_mangle]`), PyO3, MEX, SWIG
- **Nicht:** eigenes HTTP/curl-Transport-Implementation im Kern (nur Trait; Implementierung als
  Feature, siehe §5.4/§6.4)
- **Nicht:** SPICE-Kernel-Bibliothek selbst (nur Referenz-/Metadaten-Modelle + optionales
  Feature über CSPICE-Bindings)
- **Nicht:** eigene XML/JSON/Common-Format-Parser (nutzt Crates)
- **Nicht:** OpenEXR-Implementierung (nutzt `exr`/`openexr-rs`), nur das Backend-Abstraktum
- **Nicht:** Stereo-/3D-/AI-Algorithmen (nur Metadaten-Schemas, RFC-Konformanz)

## 4.4 Optionale Cargo-Features

| Feature | Liefert |
|---------|---------|
| `tiff` (**default on**) | TIFF/BigTIFF-Backend |
| `lzw`, `packbits`, `deflate`, `jpeg`, `zstd` | je Codec (deflate/zstd als Cargo-Features) |
| `http` | Remote-Range-Transport (über `ureq`/`reqwest`-Backend) |
| `pds4`, `isis`, `zarr`, `openexr` | Format-Backends |
| `camera`, `crs`, `spice`, `provenance` | Domain-Erweiterungen (RFC-gated) |
| `cli` | CLI-Binary |
| `serde` | Serialisierung von Metadaten (Diagnose/Audit) |
| `simd` | NICHT default; wird für Determinismus-Alternativen geschaltet (siehe §14) |

## 4.5 Wo brauchen wir C/C++ (bewusst)?

- **libjpeg-turbo** – maßgeblich SIMD-/Rollback-Performance des JPEG-Codecs; entweder als
  externes System-Abl auf `libjpeg`-Bindings oder als `turbojpeg`-FFI. (Siehe §5.)
- **Optional SPICE** (CSPICE via `spice-rs`-FFI), wenn Kunden echte Kernel laden müssen. Kein
  Mandat für 1.0 – die PTIFF-RFCs speichern Referenzen/abgeleitete Werte, nicht die
  Kernel-Binärformate.

## 4.6 Was sollte bewusst NICHT in Rust portiert werden?

- **libtiff selbst.** Die bestehende Implementierung ist ein **eigener** TIFF-Reader/Writer,
  kein libtiff-Delegate. Für 1.0 nutzen wir weiterhin einen eigenen TIFF/Parser (in Rust), um
  volle Kontrolle über IFD-/Tag-Verhalten, Endianness und PTIFF-Private-Tags zu behalten und
  libtiff-Kompatibilitätsprobleme zu vermeiden. Ein `tiff`-Crate bleibt optionales
  Konformitäts-/Interop-Backup, nicht Runtime-Kern.
- **Die C++-`libptiff` selbst** wird nicht „in Rust nachgebaut, dann gelöscht“, sondern bleibt
  als Oracle erhalten. Der einzige maintained source of truth wird der Rust-Core.
- **Der SWIG-Generierungscode für Go/Ruby** bleibt auf der C-ABI; wir regenerieren nur, wenn
  sich die ABI ändert.
- **Eigenes HTTP-Parsing** – `ureq`/`reqwest` statt selbst.
- **Eigenes Float-Parsing für Metadaten** – Rust `f64::from_str`/serde.
- **Die Octave-Bindings** – bleiben MEX auf der C-ABI, kein Octave-interner Core.

---

# 5. Dependency Evaluation

> **Hinweis:** Die Angaben hier sind Stand „August 2026“ und müssen am Implementierungstag gegen
> crates.io abgeglichen werden (`cargo search`, aktuelle Version, MSRV, Lizenz-SPDX). Ich mache
> keine unbelegten Zusagen über „schneller“ oder „reifer“ – die empfohlenen Crates sind die
> etablierten De-facto-Standards in ihrem Bereich (flate2, serde_json, quick-xml, ureq, rayon,
> memmap2, zstd). Die konkreten Entscheidungen sind vor Fixierung zu verifizieren.

## 5.1 TIFF

Die **eigene TIFF/BigTIFF-Implementierung** bleibt im Rust-Core. Ein externes TIFF-Crate (z. B.
`tiff`) als direkter Parser ist **nicht** gewünscht, weil:

- PTIFF-Private-Tags 65001–65005 von einem generischen Crate nicht strukturell/versionsgetreu
  behandelt werden,
- Endianness-, Predictor- und IFD-Detailkontrolle (BigTIFF-20-Byte-Records) im eigenen Parser
  robuster absicherbar sind,
- wir die volle Buffer-/Streaming-Kontrolle behalten.

**Empfehlung:** Eigener TIFF/BigTIFF-Layer im Rust-Core; tragfähige, getestete Bausteine
(Tag-IDs, Feldtypen, Endian-Routing) aus der C++-Implementierung portieren. Optionales
Interop-/Konformitätsbackup: `tiff` (LibreGraphics).

## 5.2 Kompression – Rust-Crates

| Verfahren | Empfohlenes Crate | Reife | Lizenz | Performance | Empfehlung |
|-----------|--------------------|-------|--------|-------------|------------|
| LZW (TIFF-Variante) | **Eigen im Rust-Core** | exakt, in C++ implementiert | Apache-2.0 (eigene) | hoch | Portieren, kein externes Crate |
| Deflate | **`flate2`** (Backend wählbar) | sehr reif | MIT/Apache-2.0 | gut; Backend wählen (miniz_oxide vs zlib vs libdeflater) | `flate2` |
| PackBits | **Eigen** | trivial | — | — | Portieren |
| ZSTD | **`zstd`** | reif, bindet `libzstd` | BSD | sehr hoch | `zstd` für Zarr/Cloud |
| JPEG | siehe §5.3 | — | — | — | — |

**Wichtig:** ZSTD ist derzeit **nur** im Zarr-Backend ein Feature; `CompressionKind` kennt nur
None/Lzw/Deflate/Jpeg. Soll PTIFF 1.0 ZSTD als TIFF-Kompression ausweisen, ist zuerst eine
RFC-0011-Entscheidung nötig – nicht einfach „hinzufügen“.

## 5.3 JPEG – detaillierter Vergleich

Anforderung: Grayscale+RGB/YCbCr, UInt8-only (bestehende Restriktion), 4:4:4 (kein Chroma-
Subsampling) erzwingbar, SIMD-Performance, wissenschaftliche Reproduzierbarkeit.

| Option | Maturity | Performance | Plattform | Lizenz | Risiko | Empfehlung |
|--------|----------|-------------|-----------|--------|--------|------------|
| `jpeg-decoder`/`jpeg-encoder` (pure Rust, image-rs) | reif (8-bit) | gut; SIMD meist schwächer als libjpeg-turbo | alle | MIT/Apache | 8-bit-only; DCT-Präzision weicht ab | Rückfall, kein Primärziel |
| **libjpeg-turbo via `turbojpeg`-FFI / `jpeg`-Crate** | sehr reif | **höchste**, voll parallelisierbar | alle | **IZL / BSD-3-Clause** | C-Dependency; ABI an libjpeg-Version gekoppelt | **Erste Wahl** |
| `mozjpeg`/`mozjpeg-sys` | reif | Gipfel der Encoder-Qualität | alle | BSD | **Output-NICHT byte-identisch mit libjpeg** – bricht Golden-Digests | **Nicht** für 1.0-Identität |

**Entscheidung:** Für verlustfreie/identische Kodierung mit der C++-Referenz wird **libjpeg-
turbo** verwendet. Kritisch ist **Determinismus**: libjpeg-turbo liefert bei fester Eingabe und
fester Konfiguration deterministische Bytes (kein Zufall), was die Golden-Files sichert. Ein
reiner Rust-Encoder erzeugt für denselben `quality`-Wert **andere Bytes** als libjpeg. Daher:

- Roundtrip-/Pixel-Vergleiche vergleichen **dekodierte Pixel** (mit Toleranzband), nicht die
  komprimierten Bytes, ODER hinterlegen pro-Encoder-Golden.
- Byte-exakte Golden-Tests nur für verlustfreie Codecs (LZW/Deflate/PackBits), für JPEG eine
  klar definierte Pixel-Toleranz.

## 5.4 Weitere Komponenten

| Komponente | Library | Status | Purpose | Maturity | Lizenz | Performance | Plattform | Risiko | Empfehlung |
|------------|---------|--------|---------|----------|--------|-------------|-----------|--------|------------|
| HTTP / Range | `ureq` (sync) oder `reqwest` (async) | reif, aktiv | HTTP(S) `Range`-Reader | hoch | MIT/Apache | sehr hoch | alle | reqwest braucht Tokio; ureq sync einfacher | `ureq` für Kern (sync CLI/Benchmark), `reqwest` optional für async |
| XML (PDS4) | `quick-xml` | reif | PDS4-XML-Labels | hoch | MIT | sehr hoch | alle | niedrig | `quick-xml`; oder `roxmltree` (read-only) |
| JSON (Zarr) | `serde_json` | de-facto-Standard | `.zarray`/`.zattrs` | sehr hoch | MIT/Apache | sehr hoch | alle | niedrig | `serde_json` |
| Checksums | `crc32fast` / std `md5`/`sha2` | sehr reif | Integrität (Zarr & Provenanz) | hoch | MIT/Apache | sehr hoch | alle | niedrig | `crc32fast` für Zarr; SHA-256 für Provenanz |
| Memory mapping | `memmap2` | sehr reif | optional mmap-Reader | hoch | MIT/Apache | sehr hoch | alle | niedrig | optional Feature |
| SIMD | std `core::simd` / `portable-simd` | reif (nightly für std) | LZ/Predictor-Optimierung | mittel | Rust | hoch | alle (portable) | Feature-Gating für Determinismus | optional (§14) |
| Parallelität | `rayon` | sehr reif | parallele Tile-/Decode-Zyklen | sehr hoch | MIT/Apache | sehr hoch | alle | niedrig | Haupt-Parallelismus (§6) |

---

# 6. Parallelization Strategy

## 6.1 Ist-Zustand

Die C++-Implementierung ist in der Tile-Verarbeitung **vollständig sequentiell**:
- `TiffImageSource::readTile` → ein Cursor + ein `BinaryReader` (single-cursor).
- `BinaryReader` hält einen mutablen Cursor (nicht thread-safe).
- `StorageBackend` ist thread-compatible, nicht thread-safe.
- Kein Thread-Pool, keine async-Runtime.

## 6.2 Design-Grundsätze

**I/O vs. CPU klar trennen:**

```
       sequential I/O (lokal/HTTP-Range)
          │  (Header/IFD, Tile-Offsets, je Tile ein Buffer → 1 Disk-/Range-Request)
          ▼
   ┌────────────────────────────────────┐
   │  Paralleler CPU-Work-Steal-Pool     │ ← Rayon (data-parallel), deterministischer Plan
   │    Tile 0..N: read → decompress →   │     pro Tile owned Buffer, kein geteilter &mut
   │               YCbCr/Predictor       │
   └────────────────────────────────────┘
          │
          ▼
      Consumer (Speicher, Netz, weitere Verarbeitung)  → bounded channel (backpressure)
```

1. **Sequentielles I/O** bleibt zuerst sequentiell: Header/IFD lesen, Tile-Tabelle lesen. Das ist
   billig und vermeidet Races auf dem Byte-Cursor.
2. **Paralleles Lesen von Tiles:** Nach dem Parsen kennt man die Offsets. Jeder Tile kann
   **unabhängig** gelesen werden – bei lokalen Dateien via `memmap2` oder `FileExt::read_at`,
   bei HTTP via begrenzt parallelisierte Range-Requests.
3. **CPU-Teil (Decompression):** Rayon `par_iter` über `(tile, buffer)`. Jeder compressed
   Tile-Buffer ist per `Vec<u8>` owned und `Send + Sync`, daher problemlos parallel.
4. **Schreiben:** Sequentielles Schreiben von Header/IFD + paralleles **Komprimieren** der Tiles;
   die komprimierten Bytes werden sequentiell in Offset-Reihenfolge geschrieben (Tile-Offsets
   sind vorab deterministisch berechenbar).

## 6.3 Race-Conditions / Thread-Safety-Anforderungen

- **`BinaryReader`-Cursor:** Nicht teilen. Für parallele Reads nicht den mutable Cursor teilen;
   stattdessen pro Tile `read_at`/Range-Request (positiv, da Offset bekannt). Konzeptionell
   getrennt: **eine read-Position pro Thread**.
- **`StorageBackend`-Instanz:** Nach `deserializeModel`/`open` den Tile-Lookup-Index
   (Offsets/Längen) in einen eigenen, immutable Snapshot (`Sync`) überführen. Als immutable
   Snapshot ist er `Sync` (teilsicher in Read-Only-Nutzung).
- **Kompressions-Rohdaten:** pro Tile owned & moved. Kein gemeinsamer `&mut`.
- **JPEG-Worker:** libjpeg-turbo (C-API) ist pro Instanz thread-safe; pro Tile eine eigene
   Handler-Struktur erzeugen → voll parallel.
- **Async vs. sync:** Der **Core bleibt synchron** (kein Tokio im Kern). Für eine zukünftige
   async-Fassade wird der sync-Kern in `spawn_blocking` ausgeführt. HTTP-Range wird per
   Threadpool parallelisiert; die **C-ABI ist definitionsgemäß synchron** (kein Future über die
   ABI reichen).

## 6.4 Grenzen klar definieren

| Operation | Strategie |
|-----------|-----------|
| Header/IFD parse | sequentiell |
| Tile-Offset-Tabelle lesen | sequentiell (ein Request/read) |
| Tile-Bytes lesen (lokal) | parallel via `read_at` (Offset bekannt) |
| Tile-Bytes lesen (HTTP-Range) | begrenzt parallele Range-Requests (z. B. ≤ 8 in-flight) |
| Decompression (CPU) | Rayon-Pool, eine Job/Tile |
| Compression (CPU) | Rayon-Pool, eine Job/Tile |
| Tile schreiben (Datei) | sequentiell in Offset-Reihenfolge (komprimierte Bytes gepuffert) |
| Memory-Verwaltung | Buffer-Pool pro Thread (kein teurer Alloc pro Tile), bounded channel |

## 6.5 NUMA / Memory / Backpressure

- **NUMA:** Für First-Class-Unterstützung ist ein `rayon` Work-Standard gut; echte NUMA-Optimierung
   (Erstzugriff auf die richtige Domain) ist kein Ziel für 1.0 – Berücksichtigung als offen.
- **Memory-Druck:** Tile-Puffer einzeln owned; pro Worker ein wiederverwendbarer Buffer (`par_iter`
   mit `for_each_init` auf einem je-Thread-Buffer). Begrenzte Parallelität, keine OOM durch
   unbegrenzte Warteschlangen.
- **Backpressure:** Bei Streaming/Netz ein bounded channel (`std::sync::mpsc` begrenzt oder
   `crossbeam-channel` mit Kapa), der den Produzenten blockiert, wenn der Konsument nicht mitkommt.
- **Bestimmtheit:** Der parallele Plan ist deterministisch bzgl. der Reihenfolge der Ergebnisse,
   auch wenn die Ausführungsreihenfolge pro Lauf variieren darf. Für verlustfreie Codecs ist das
   Ergebnis immer byte-identisch (Decompression ist funktionstüchtig); für JPEG gilt, dass die
   dekodierten Pixel deterministisch sind, solange dieselbe libjpeg-Konfiguration verwendet wird.

---

# 7. C ABI Design

## 7.1 Rolle der C-ABI

Die C-ABI ist die **einzige sprachübergreifende Interoperabilitätsschicht** von PTIFF 1.0. Sie
muss ermöglichen: **C, C++, Python (FFI/PyO3), GNU Octave (MEX), Go, Ruby, Rust (FFI)**.

## 7.2 Konzeptionelle Merkmale (Dokumentation, kein Header-Schreiben)

### Opaque handles
- Alle Ressourcen (Dokument, Image, Source, Sink, Metadata, Tile) werden als **opaque,
  undurchsichtige Pointer** exponiert (z. B. `ptiff_document_t*`, `ptiff_image_t*`,
  `ptiff_source_t*`). Niemals der Inhalt dereferenziert.
- Jeder Constructor liefert ein Handle; jeder Destructor (`_close`/`_destroy`) ist idempotent
  (NULL = no-op).

### Error handling
- Kein Exceptions über ABI. Jede fallible Funktion gibt einen **int/Fehlercode** zurück
  (negativ für Fehler, 0 für Erfolg), optional mit ausführlicher Fehlermeldung über Out-Parameter
  oder eine `ptiff_error_last()`/`ptiff_error_message()`-Abfrage.
- `ErrorCode`-Werte sind **stabil und wachsen nur** (nicht umsortieren, nicht entfernen).

### Ownership & Memory
- **Grundregel:** „wer öffnet, schließt“. Pointer, die der Caller erhält, muss der Caller
  freigeben (symmetrische `_free`/`_close`).
- Out-Parameter sind explizit dokumentiert: wer besitzt die Daten (caller vs. callee), wie werden
  sie freigegeben.
- C-Strings: `ptiff_free_string()` als einheitlicher Freigaber für vom Callee allozierte Strings.

### Lifetime
- Ein ImageSource ist nur gültig, solange das Dokument, das es erzeugte, lebt.
- Cursor/Handle-Lifetimes werden konzeptionell als „Handle-Tree“ modelliert: Kind-Handles
  referenzieren das Eltern-Handle und werden in der Doku als „valid only while parent lives“
  markiert.

### Thread safety
- **Thread-sichere API** als PTIFF-1.0-Ziel (nicht nur thread-compatible): ein Handle darf in
  mehreren Threads gleichzeitig **gelesen** werden; schreibende Operationen (Sink) sind
  serialisiert oder pro-Thread. Konzeptionell: Lesehandles sind `Sync`-ähnlich, Schreibhandles
  werden synchronisiert. Detaillierte Grenzen im §6-Kontext.

### Versioning / ABI compatibility
- **Semantische Versionierung (SemVer)** der ABI: Maj zum Brechen der C-ABI, Min für
  hinzugefügte, rückkompatible Funktionen, Patch für Fixes.
- **Symbol-Versionierung** dort, wo ein OS sie unterstützt (ELF `.symver`, macOS `-compatibility_version`)
  – optional.
- `compile_time_version()` vs. `runtime_version()` (bereits in C++ vorhanden) wird übernommen,
  um ABI-Mismatch früh zu erkennen.

## 7.3 Beispielsignaturen (konzeptionell – nur Muster, kein finaler Header)

```c
// Öffnen/Schließen
ptiff_document_t*  ptiff_document_open_path(const char* path, ptiff_error_t* err_out);
void               ptiff_document_close(ptiff_document_t* doc);

// Image Metadaten (ohne Pixel)
ptiff_image_t*     ptiff_document_image(ptiff_document_t* doc, uint32_t index,
                                        ptiff_error_t* err_out);
uint32_t           ptiff_image_width(const ptiff_image_t* img);
int                ptiff_image_pixel_type(const ptiff_image_t* img, ptiff_pixel_type_t* out);

// Pixel-Tile-Zugriff
ptiff_source_t*    ptiff_source_open(ptiff_document_t* doc, uint32_t image_index,
                                     ptiff_error_t* err_out);
int                ptiff_source_read_tile(ptiff_source_t* src,
                                          uint32_t column, uint32_t row,
                                          uint8_t* buffer, size_t buffer_size,
                                          size_t* bytes_read);
```

Diese Signaturen orientieren sich an der bereits bestehenden `bindings/c/*.h` und erweitern sie
konsistent. **Kein Header wird in diesem Plan geschrieben.**

## 7.4 Header-Strategie: cbindgen vs. manuell

| Option | Bewertung |
|--------|-----------|
| **cbindgen** | Erzeugt Header aus Rust `#[no_mangle] extern "C"`-Code. Gut für Konsistenz; erzeugt aber automatisch, was die **manuell gepflegten, stabilen Header** als „source of truth“ ersetzen würde. Risiko: automatische Header können semantisch unschön sein und ABI-Versprechen nicht dokumentieren. |
| **Manuell gepflegte C-Header** (Empfehlung) | Header sind die Menschen-lesbare, dokumentierte Spezifikation der ABI. Rust-Seite parst sie via Bindings (bindgen) oder deklariert `extern "C"` manuell. Vorteil: volle Kontrolle über Doku, out-params, Ownership-Hinweise. |

**Empfehlung:** Manuell gepflegte C-Header in `bindings/c/` bleiben die Single Source of Truth
(so wie heute). cbindgen kann *ergänzend* zur Konsistenzprüfung (CI-Abgleich Header ↔ Rust-FFI)
laufen, ist aber nicht der Autor der ABI.

## 7.5 ABI-Versionierung

- Die bestehende `libptiff_c` heißt künftig weiter `libptiff_c`; ABI-Version (Major) wird durch
  SemVer der C-ABI getragen.
- Symbol-Versionierung (ELF) ist optional und wird nur bei nachgewiesenem Bedarf eingeführt.
- Ein ABI-Header `ptiff_version.h` mit `PTIFF_ABI_VERSION` (hochgezählt bei Breaking Changes)
  wird gepflegt.

---

# 8. C++ Strategy

## 8.1 Ziel

Die bestehende C++-API darf nicht unnötig verloren gehen. Für PTIFF 1.0 gibt es eine
**C++-Wrapper-Schicht**:

```
C++ (Consumer)
   │
   ▼
ptiff-cpp (neuer, handgeschriebener, moderner C++-Wrapper)
   │
   ▼
C ABI (ptiff-c)
   │
   ▼
Rust Core (ptiff-core)
```

## 8.2 Wrapper-Optionen bewertet

| Option | Bewertung |
|--------|-----------|
| **C-ABI direkt verwenden** | Einfach, aber roh: kein RAII, keine STL, kein Exception-Mapping. Als *interner* Unterbau der Wrapper-Schicht sinnvoll. |
| **`cxx`** | Baut Brücke Rust↔C++ über generierte Header, **kein** Call aus einer anderen Sprache/PyO3. Unterbricht den „C-ABI-zentral“-Gedanken, weil C++ sonst direkt an Rust andocken würde. **Nicht gewünscht** für die zentrale Architektur. |
| **`ptiff-cpp` (eigener moderner C++-Wrapper)** | **Empfehlung.** Kann `std::expected`, RAII, STL-Container, `std::span`, `std::variant` nutzen und als komfortables, exceptions-freies (oder beim MUSS exceptions-) Mapping über der C-ABI liegen. |

## 8.3 Ownership-/Exception-Mapping

- **Exceptions:** Für 1.0 als opt-in wrapper, der C-ABI-Fehler in `ptiff::result<T>`-Werte
  (`std::expected`) übersetzt; kein Zwang auf Exceptions. Die C++-API soll sich für bestehende
  C++-Anwender natürlich anfühlen (gleiche Typnamen wie heute: `Image`, `Scene`, `Reader`,
  `Writer`, `Camera`, `Result<T>`, `Error`, `ErrorCode`).
- **RAII:** Jedes Handle der C-ABI wird von einem RAII-Objekt (unique_ptr-artig) geschützt
  (`~` ruft `_close`/`_destroy`), move-only wo sinnvoll.
- **STL-Container:** `std::span<uint8_t>` für Tile-Buffer, `std::vector`, `std::optional` für
  optionale Metadaten.
- **`std::expected`:** `Result<T>` = `std::expected<T, Error>` – bereits bestehender Stil.
- **Ownership-Mapping:** Verweis von C-Handles auf C++-Objekte; der C++-Wrapper besitzt das
  Handle und garantiert Lifetime, solange das Eltern-Objekt lebt.

## 8.4 Abwärtskompatibilität

- Während der Migration bleibt die **bestehende** C++-`libptiff` parallel verfügbar (Oracle).
  Die neue `ptiff-cpp` wird zunächst unter neuem Namen (z. B. `ptiff-cpp`) bereitgestellt, um
  Kollisionen zu vermeiden.
- Die Datei-/Kopfzeilennamen (`ptiff/*.hpp`) und die zentrale `Reader`/`Writer`/`Image`/`Scene`/
  `Result`/`Error`/`ErrorCode`-Semantik bleiben der öffentliche Vertrag.

---

# 9. Python Strategy

## 9.1 Bestehende Lösung: SWIG

Die heutige Python-Bindung (`bindings/python/`) ist **SWIG-`-python`** über das C-ABI
(`bindings/swig/ptiff.i`). Der `_ptiff`-Wrapper bindet nur die `extern "C"`-Oberfläche
(`libptiff_c`), trägt keine C++-Runtime. Die `typemaps.i` mappen counted `(ptr,size)`-Buffer auf
`bytes`/`bytearray`/`memoryview`; Out-Parameter werden als Extra-Rückgabewerte ergänzt.
Ein bekannter Punkt: SWIG 4.5.0 segfaulted beim Python-Shadow-Class-`__init__` auf CPython 3.14 –
Build gegen die uv-gepinnte 3.13.

## 9.2 Optionen verglichen

### Option A
```
Rust → C ABI → SWIG → Python
```
- **Pro:** Wiederverwendung des bestehenden SWIG-Pfads (und Go/Ruby teilen ihn).
- **Contra:** SWIG-Python ist weniger developer-freundlich, hat die oben genannte SWIG-CPython-
  Empfindlichkeit, erzeugt generische Wrapper ohne NumPy-Integration.

### Option B (Empfehlung für 1.0)
```
Rust → (ptiff-core) → PyO3 → Python
```
- **Pro:** Native Rust-Achse; PyO3 bietet **NumPy-Integration** (zero-copy über `numpy::PyArray`),
  moderne Typen, sauberes Packaging via **maturin**, direkte Fehler-/Ownership-Mapping, keine
  C++-Runtime.
- **Contra:** Zusätzlicher Maintenance-Aufwand gegenüber SWIG (ein neuer Wrapper). Ersetzt die
  bisherige SWIG-Python-Bindung, was die gemeinsame SWIG-`ptiff.i` für Python aufgibt.

### Option C
```
Rust → C ABI → cffi/ctypes → Python
```
- **Pro:** Minimal, kein Build-Wrapper.
- **Contra:** Umständlich für Typsicherheit, out-params umständlich, keine native NumPy-Integration,
  viel Boilerplate.

## 9.3 Empfehlung für PTIFF 1.0: **Option B (PyO3)**

Begründung:
- **NumPy / zero-copy:** PyO3 kann direkt `numpy::PyArray`-Views über Tile-Buffer legen; für
  wissenschaftliche Nutzer (NumPy) ist das essenziell. SWIG/ctypes haben keine native
  Array-Integration.
- **Packaging:** `maturin` baut Räder pro Plattform; einfacher als SWIG-Build-Matrix.
- **Maintenance / DX:** PyO3 ist in Rust aktiv gepflegt, liefert typsichere Bindings, generiert
  einen sauberen Python-API-`__init__.py`. Keine SWIG-CPython-3.14-Problematik.
- **Memory-Ownership:** PyO3 verwaltet Python-Objekte korrekt (Refcounting), kein String-Leak-
  Muster wie bei `ptiff_backend_names()`+`ptiff_free_string()` unter SWIG.

**Übergang:** Während der Migration (Phase 9) bleibt die bestehende SWIG-Python-Bindung als
Rückfall für die Feature-Parität erhalten; die PyO3-Bindung wird schrittweise parallel
bereitgestellt (`ptiff-py`), bis sie die komplette C-ABI-Surface (Version, Logger, Image,
Source, Sink, Metadata/Camera/PDS-Layer) abdeckt. Das C-ABI bleibt für Option-A-Rückfall
bestehen (insbesondere für Go/Ruby).

## 9.4 Ideale Python-API (konzeptionell)

```python
import ptiff, numpy as np

with ptiff.open("scene.ptiff") as doc:
    img = doc.image(0)                 # ptiff.Image
    print(img.width, img.height, img.pixel_type)
    tile = img.read_tile(column=1, row=0)   # np.ndarray (zero-copy-Liste)
```
Es wird ein `numpy.ndarray` (kein `list`) für Pixel verwendet, mit optionalem `dtype`-Mapping
auf `uint8/uint16/uint32/float32/float64`.

---

# 10. Octave Strategy

## 10.1 Bestehende Lösung

Die Octave-Bindung (`bindings/octave/`) ist ein **SWIG-`-octave`-Modul** über das C-ABI. Der
SWIG-Octave-Runtime (`octave_swig_ref`, `octave_value`-Typsystem, `mkoctfile`) ist C++, daher
wird der Octave-Wrapper als C++ erzeugt, bindet aber nur die `extern "C"`-Oberfläche von
`libptiff_c`. Die `typemaps.i`-SWIGOCTAVE-Regeln akzeptieren `uint8`-Arrays/Strings für
Buffer und geben den gelesenen Puffer als zusätzlichen `uint8`-Output-Wert zurück (Octave
kopiert bei Zuweisung, kein In-place-Mutate).

Der vorgesehene Pfad `Octave → MEX → C ABI → Rust Core` ist also bereits aktiv: SWIG-Octave
erzeugt einen MEX-Wrapper über `ptiff_*`.

## 10.2 Analyse der MEX-Details

- **C vs. C++ MEX:** SWIG-Octave nutzt C++ (`mkoctfile`), was die Octave-Primitive
  (`octave_value_list`, `octave_idx_type`) sauber indexiert. Die C-ABI bleibt rein C, die MEX
  ist die C++-Hülle.
- **Array-Conversion:** `uint8`-Row-Arrays werden hin- und zurückkopiert; für größere
  Tile-Buffer ist das akzeptabel, aber **kein zero-copy** (Octave kopiert bei Zuweisung).
- **Fehlerbehandlung:** C-ABI-Fehlercodes werden als `err_out`-Out-Parameter gemappt; Octave
  erhält `[res, err]`-Rückgabewerte.
- **Packaging:** Octave-Packages via `pkg` / `mkoctfile`; ein Octave-Package (`.m` +
  `_ptiff.oct`) ist sinnvoll.
- **Plattform:** Linux/macOS/Windows über mkoctfile; Status variiert (Windows-Octave weniger
  getestet als Linux).

## 10.3 Entscheidung: eigener `ptiff-octave`-Adapter?

**Ja, ein eigener dünner `ptiff-octave`-Adapter zusätzlich zur SWIG-Bindung ist sinnvoll**, wenn
man eine idiomatische Octave-API will (z. B. `ptiff.open`, `img.read_tile`, Matrizen-I/O auf
`uint8`-Arrays). Der Adapter ruft die C-ABI (nicht den Rust-Kern direkt), so dass Octave die
gleiche stabile ABI wie alle Sprachen nutzt.

- **Empfohlen:** `bindings/octave/` erweitert um einen schlanken, handgeschriebenen C++-MEX-
  Adapter `ptiff_octave.cpp` (der die C-ABI aufruft) plus eine `.m`-API-Schicht
  (`ptiff_open.m`, `ptiff_image.m`, `ptiff_read_tile.m` …). Das SWIG-Modul bleibt als
  Low-Level-Rückfall.
- **Zero-copy:** Octave erlaubt kein echtes zero-copy über die MEX-Grenze; die Puffer kopieren
  wir, dokumentieren dies aber. Für typische Tile-Größen (256×256×…) ist das ok.
- **Fehlerbehandlung:** MEX wirft bei Fehlercodes ein Octave-`error()`-Objekt mit
  `ptiff.*`-Kontext.

## 10.4 Empfehlung

`Octave → MEX (C++-Adapter) → C ABI → Rust Core` ist die saubere Zielarchitektur. Der
bestehende SWIG-Octave-Pfad bleibt, bis der eigene `ptiff-octave`-Adapter Feature-Parität
erreicht. Beide nutzen dieselbe C-ABI → kein doppelter Kern.

---

# 11. Testing Strategy

## 11.1 Die C++-Implementierung als Referenz/Oracle

Während der Migration laufen **beide** Implementierungen parallel:

```
             Test Suite
              /      \
             /        \
      C++ PTIFF      Rust PTIFF
          │              │
          └──── gleiche ─┘
                Tests
```

Die Rust-Implementierung muss nachweisen, dass sie **semantisch identisch** zur C++-Referenz ist.

## 11.2 Testarten

| Testart | Beschreibung | Anwender |
|---------|--------------|----------|
| **Golden-Files** | Binär/Byte-erwartete Ausgabe von `serializeModel`/`write`; die C++-Referenz erzeugt das Golden, Rust muss es reproduzieren. Für verlustfreie Codecs byte-exakt; für JPEG nur Pixel-Toleranz. | Beide |
| **Binary-Kompatibilität** | C++-Party schreibt eine Datei, Rust liest sie (und umgekehrt). MUST-match (Rust-Read == C++-Write) für alle nicht-komprimierten Formate; für Deflate/LZW byte-identisch; JPEG Pixel-Match. | Beide |
| **Metadaten-Vergleich** | `read_metadata`/Extension-Felder (Tags 65001–65005) aus C++-Datei via Rust ergeben exakt dieselben (key,value)-Paare. | Beide |
| **Pixel/Tile-Vergleich** | `read_tile`-Ergebnisse aus C++ vs. Rust für identische Datei: `assert_eq!`-Buffer. | Beide |
| **Round-Trip** | Schreiben mit Rust, Lesen mit C++ und umgekehrt; Ergebnis muss Pixel-/Metadaten-identisch sein. | Beide |
| **Kompressionstests** | Je Codec (LZW, Deflate, PackBits, JPEG) cross-implementiert Round-Trip. | Beide |
| **Corrupted-File-Tests** | Missgebildete TIFFs (fehlerhafte IFD-Offsets, überbreite Tag-Counts, truncierte Tiles) müssen in beiden gleich abgefangen werden (`ErrorCode::InvalidArgument/OutOfRange/NotFound`). | Beide |
| **Fuzzing** | Rust-`cargo fuzz` (mit `arbitrary`) und C++-Clang-Fuzzer gegen denselben Corpus; Eigenschaften: kein Panic, keine OOM, keine Fehlinterpreation von Benachbarten. | Beide |
| **Cross-Language-Tests** | Ein und dieselbe Datei wird über Rust-, Python-, Go-, Octave-Binding gelesen; alle liefern identische Metadaten/Pixel. Nutzt `scripts/interop*`-Ansätze. | Alle |
| **Conformance** | `conformance/levels.md` / `matrix.md` definiert Stufen; Rust-Implementierung & C++ bestehen beide die Stufen. | Beide |
| **GitHub-Actions/CI** | pro Phase ein CI-Job, der beide Implementierungen baut und die Cross-Vergleiche laufen lässt. | CI |

## 11.3 Golden-Files-Regelung

- **Verlustfreie Codecs + unkomprimiert:** Golden = exakte Bytes (SHA-256-Hash im Testcode
  hinterlegt, Datei unter `tests/golden/`).
- **JPEG:** Golden = Pixel-Hashes der **dekodierten** Pixel (nicht der komprimierten Bytes),
  mit dokumentierter Toleranz (z. B. `max_abs_diff <= 1` und identische Dimensionen), weil
  Encoder-Konfigurationen zwischen libjpeg und pure-Rust abweichen können. Für 1.0 ist der
  Ziel-Encoder libjpeg-turbo (deterministischer Encoder), also kann der Golden auch byte-exakt
  pro libjpeg-Version sein – wird als offen gekennzeichnet.

## 11.4 Werkzeuge in Rust

- `cargo test` (Unit/Integration), `proptest` für Property-Tests der Tile-Arithmetik,
- `cargo fuzz` (via `cargo-fuzz`) für die Parser/Decoder,
- `criterion` für Benchmarks (§12),
- `crossbeam`/`rayon`-Paralleltests mit `#[cfg(test)]`-Thread-Zählern.

---

# 12. Benchmark Strategy

## 12.1 Grundsatz

**Nicht behaupten, dass Rust automatisch schneller ist.** Wir formulieren messbare Hypothesen
und vergleichen C++-PTIFF vs. Rust-PTIFF auf identischen Fixtures/Hardware.

## 12.2 Hypothesen (messbar)

| Nr | Hypothese |
|----|-----------|
| H1 | `open` + Metadata-Read: Rust ≤ C++ (beide parse einmal). |
| H2 | `read_tile` unkomprimiert: Rust ≤ C++ (Byte-copy + overhead vergleichbar). |
| H3 | `read_tile` LZW/Deflate/PackBits: Rust ≈ C++ (Single-threaded), `≥2×` bei paralleler Tile-Kompression. |
| H4 | `read_tile` JPEG: Rust ≈ C++ bei identischem libjpeg-turbo; DCT-Determinismus. |
| H5 | Random-Access (zufällige Tile-Offsets): Rust ≤ C++ bei `memmap2`-basierte Reads. |
| H6 | Sequentielles Lesen: ≥ C++ (keine Regression). |
| H7 | parallele Tile-Decompression (Rayon): skaliert sublinear bis linear über Cores gegenüber C++ sequential. |
| H8 | parallele Kompression: ≥ C++ sequential bei ≥2 Cores. |
| H9 | Großer BigTIFF (>4 GiB): Rust-Read kein Speicherüberlauf, Streaming wie C++. |
| H10 | HTTP-Range-Reads: Rust (ureq/reqwest) ≤ C++ (libcurl) bei gleicher Bandbreite; Overhead vergleichbar. |

## 12.3 Vergleichsgrößen

- open (ms), metadata read (ms), tile read/write (ms & MB/s), sequential read, random access,
- LZW / Deflate / JPEG / ZSTD (nur falls als TIFF-Codec aktiviert) encode+decode,
- parallele Decompression/Compression (Speedup bei 1/2/4/8 Cores),
- großer BigTIFF (streaming memory footprint, keine OOM),
- remote HTTP-Range (Latenz + Durchsatz).

## 12.4 Harness

- Nutzt das bestehende `benchmarks/`-Gerüst, erweitert um einen einheitlichen Rust-Bench
  (criterion) und C++-Bench (Google Benchmark oder die bestehende C++-Benchmark-Datei) mit
  identischen Fixtures (aus `scripts/fetch_sample_tiff.sh` / `make_fixtures.py`) und einem
  gemeinsamen Messprotokoll (Median, repeats).
- Ergebnisse in `benchmarks/benchmark-results/` (bereits vorhandenes Format) mit klarer
  Metadaten-App (OS, CPU, Rust/C++-Version, Codec, Parallelität).

---


# 13. Build Strategy

## 13.1 Ist-Zustand (CMake + Conan)

Der bisherige Build ist CMake (C++23) + Conan 2 mit 12 externen C/C++-Dependencies
(`fmt`, `spdlog`, `catch2`, `pugixml`, `nlohmann_json`, `openexr`, `zstd`, `zlib`, `libdeflate`,
`libjpeg-turbo`, `libcurl`, `cpp-httplib`). Die konkreten Slow-Build-Ursachen sind in §2.2
dokumentiert (header-schwere Templates, ~60 TUs, Conan-Quellcode-Build von openexr/libcurl,
fehlende Cargo-Feature-Trennung).

## 13.2 Vergleich: CMake+Conan vs. Cargo vs. Hybrid

| Kriterium | CMake+Conan (heute) | Cargo (Rust-only) | Hybrid |
|-----------|---------------------|-------------------|--------|
| Linux | ✅ | ✅ | ✅ |
| macOS | ✅ | ✅ | ✅ |
| Windows | ✅ (MSVC) | ✅ (MSVC/gnu) | ✅ |
| ARM64 | ✅ | ✅ (Rust-Standard) | ✅ |
| x86_64 | ✅ | ✅ | ✅ |
| Static linking | ✅ | ✅ (`static` crates) | ✅ |
| Dynamic linking | ✅ | `cdylib`-Crate | ✅ |
| System libraries | Conan/vcpkg | Cargo-Crates (meist reine Rust) | beide |
| Package manager | Conan 2 | Cargo | Cargo (Haupt) + Conan (nur C++-Wrapper) |
| CI | build.yml-Matrix | Cargo-Workspace-Jobs | anfangs beide |
| Release artifacts | CPack (deb/rpm/pkg/nix) | `cargo build --release` + maturin/wheels | hybrid |

**Vorteile von Cargo für diesen Fall:**

- **Feature-Gating** baut nur, was benötigt wird (löst Slow-Build-Ursache 6 & große Teile von
  Ursache 1).
- **Reine-Rust-Crates** (flate2, serde_json, quick-xml, ureq, rayon) haben keinen C++-Compile.
- **Cargo-Caching** (sccache / CI-Cache) ist einfach reproduzierbar.
- **Cargo-Workspace** liefert ein binäres CLI und libs aus einer Quelle.

**Nachteile/ehrliche Grenzen:**

- Einige teure C-Bibliotheken bleiben (libjpeg-turbo): sie werden über System- oder prebuilt-Bindings
  angesprochen – nicht durch Cargo magisch beseitigt, aber sie entfallen als Conan-Quellcode-Build.
- Es gibt keine offizielle „Cargo-Ein“-Standard-Packaging für .deb/.rpm wie CMake/CPack; wir
  führen die bestehenden CPack/Releases (über CMake-Wrapper, der Cargo aufruft) weiter.

## 13.3 Empfohlener Hybrid

**Empfehlung für PTIFF 1.0: Cargo als Build-Kern + dünner CMake-Wrapper für Pakete, die
verpackt werden müssen.**

1. **Rust-Core & CLI & Python:** reine Cargo-Workspace (`cargo build --release`, `maturin build`,
   `cargo test`). Die Haupt-Dev-Schleife ist Cargo.
2. **C-ABI & C++-Wrapper:** Cargo erzeugt `staticlib`/`cdylib` (`ptiff-c`); die C++-Wrapper baut
   mit CMake, das über das C-ABI verlinkt. Ein CMake-Target `ptiff_c`/`ptiff_cpp` ruft Cargo
   (analog zum bereits bestehenden `ptiff_cli`-Custom-Target im Root-CMakeLists).
3. **CPack/native packages:** Der bestehende Root-CMakeLists bleibt als „Verpackungs-Orchestrator“
   (ruft Cargo-Targets auf, erzeugt deb/rpm/pkg/nix). Das bewahrt die bestehenden Release-Workflows.
4. **Linux/macOS/Windows/ARM64/x86_64:** Cargo-Workspace deckt alle Target-Triples ab; CI führt
   eine Cargo-Matrix (und die bestehende CMake-Matrix für die Wrapper-Pakete).
5. **Static/dynamic:** `ptiff-core` wird statisch, `ptiff-c` kann static (`staticlib`) oder
   dynamic (`cdylib`) gebaut werden.

## 13.4 Warum kein voller Verzicht auf CMake

- Der C++-Wrapper `ptiff-cpp` (nie ersetzt, nur neuer Wrapper) braucht weiter CMake für seinen
  Build; das ist ein Kompatibilitätsbedarf, nicht das Kern-Target.
- Die bestehenden CPack/Nix/Release-Flows (native Packages, Docs) hängen an CMake; wir halten
  alles konsistent, indem CMake zu einem **Thin-Orchestrator um Cargo** wird.

---

# 14. Scientific / Planetary Data Requirements

Die Rust-Migration darf die wissenschaftlichen Eigenschaften von PTIFF **nicht** verschlechtern.

## 14.1 Explizit zu bewahrende Eigenschaften

| Anforderung | Maßnahme |
|-------------|----------|
| **Deterministische Verarbeitung** | Ergebnis der Decompression/Kompression für verlustfreie Codecs ist byte-identisch (kein Zufall, keine Float-Jitter). Parallele Ausführung ändert das Ergebnis nicht (deterministischer Plan, §6.5). |
| **Numerische Genauigkeit** | `float32`/`float64`-Pixel unverändert (IEEE-754), kein truncation, kein endianness-Fehler beim Read/Write. |
| **Endianness** | TIFF-`byteOrder` wird exakt gehandhabt (LE/BE), BigTIFF-20-Byte-Records korrekt. Rust liest/schreibt byte-orientiert mit klaren `from_be_bytes`/`from_le_bytes`. |
| **Floating point / NaN / Inf** | `f32`/`f64` bleiben als rohe Bitmuster erhalten (kein Normalisieren). NaN/Inf werden byte-erhaltend kopiert; kein `min/max`-Beschneiden. |
| **Nodata** | `nodata`-Semantik (extra samples / spezifische Werte) unverändert; kein Clipping oder Reskalieren. |
| **Metadaten-Preservation** | Die fünf PTIFF-Tags (65001–65005) mit dem versionierten Payload-Codec (RFC-7002) werden byte-erhaltend/deterministisch round-trippen. Unbekannte Keys/Neu-Versionen werden verbatim erhalten. |
| **CRS** | GeoTIFF/Planeten-CRS-Metadaten (Koordinatensystem-IDs) konservativ übernommen, keine Interpretation die verändern könnte. |
| **SPICE** | Nur Referenzen/abgeleitete Werte speichern; unterstützende Kernel-Bibliothek bleibt optional. |
| **PDS4 / ISIS3** | Backends bleiben kompatible Reader/Writer; Label-Handling konservativ (XML-Struktur erhalten). |
| **Provenance** | Geschichte/Provenanz-Felder exakt und verlustfrei round-trippen. |
| **Reproduzierbarkeit** | Deterministic-Builds (feature-gated), feste Zahlenformate, keine hardware-abhängigen Codecs im Kern. |
| **Langzeit-Archivierung** | Keine Verwendung von Algorithmen mit unspezifizierter Ausgabe; Kodierung strikt nach RFC-7002. |

## 14.2 Besonders konservativ zu behandeln

1. **JPEG:** DCT-Koeffizienten/Quantisierung können je nach libjpeg-Version/CPU variieren. Für
   Archiv-Zwecke: verlustfreie Formate (None/Deflate/LZW) sind deterministisch; JPEG
   dokumentieren wir als „Bitgenauigkeit v. libjpeg-turbo-Parameter abhängig“ mit klar geprüfter
   Toleranz.
2. **CRS/Georeferenzierung:** Keine ungeprüfte Projektionstransformation im Kern (kein silent
   geodätisches Rechnen); nur Tag-/WKT-/ID-Serialisierung.
3. **Endianness und Byte-Exaktheit beim Schreiben:** Der Writer muss OFDs/Offsets deterministisch
   und auf BL-Byte-Ebene reproduzierbar erzeugen (damit Golden-Digests stabil sind).
4. **Floating-Point-Reproduzierbarkeit:** Keine Float-Toleranzannahmen in Round-Trip-Tests, die
   „zufällige“ Jitter erlauben; exakte Bitmuster-Vergleiche ergänzt durch Toleranztests nur dort,
   wo Berechnungen einfließen (z. B. K-Matrix, Projektion).

## 14.3 Determinismus-Feature

Der Cargo-Feature-`simd` ist standardmäßig **deaktiviert** (nicht-reproduzierbar, hardware-
abhängig). Der Default-Pfad nutzt portable, deterministische Implementierungen (wie C++ heute).
SIMD wird nur als opt-in-Performance-Pfad angeboten, und die Ausgabe folgt dennoch denselben
Werten (verlustfreie Codecs), da nur schneller gerechnet wird, nicht anders gerundet.

---

# 15. Build / Performance-Hinweise zu PTIFF 1.0

> Dieser Abschnitt fasst die wichtigsten beweisbaren Fakten zusammen (Details in §2/§13).

- Der **heutige Build ist langsam**, weil er die teuren C-Bibliotheken (openexr, libcurl,
  libjpeg-turbo) aus Conan-Quellcode baut, header-schwere Templates in ~60 TUs instanziiert und
  keine Feature-Trennung zwischen Kern und optionalen Backends hat.
- Eine **Rust-Architektur beseitigt das teilweise** – nicht durch „Rust kompiliert schneller“,
  sondern weil reine-Rust-Crates kleiner sind, Cargo nur aktivierte Features baut und teure
  C-Bibliotheken durch kleinere Abhängigkeiten ersetzt werden.
- **libjpeg-turbo bleibt** die einzige unvermeidbare schwere C-Abhängigkeit für die
  JPEG-Determinismus/Performance-Anforderung; sie wird über Bindings gelinkt, nicht mehr aus
  Conan-Quellcode neu gebaut. Der Hybrid (§13) sichert die bestehenden CPack/Nix/Release-Flows ab.

---

# 16. PTIFF 1.0 Definition

Konkrete Definition von **PTIFF 1.0**, die ein Team als Ziel anstreben und erfüllen kann.

## 16.1 Core

- **Rust-Referenzimplementierung**: `ptiff-core` ist der einzige maintained Core (Safe-Rust,
  getestet, fuzzbar).
- **Stabile C-ABI**: `ptiff-c` exponiert opaque handles, stabilen Fehlerpfad, dokumentierte
  Ownership- und Lifetime-Regeln (§7). SemVer + `PTIFF_ABI_VERSION`.
- **Thread-sichere API**: Lesen aus mehreren Threads an einem Handle erlaubt; Schreiben
  synchronisiert (§6).
- **Dokumentiertes Ownership-Modell**: „wer öffnet, schließt“; Kind-Handles gültig, solange
  das Eltern-Handle lebt.

## 16.2 Sprachen

- **Rust** (idiomatisch via `ptiff` crate), **C** (roh auf C-ABI), **C++** (`ptiff-cpp` moderner
  Wrapper), **Python** (PyO3 + NumPy-integriert), **GNU Octave** (MEX via C-ABI). Go/Ruby via
  bestehende SWIG-Bindings auf C-ABI.

## 16.2a Plattformen

Mindest-Ziel für PTIFF 1.0 (weitere Targets werden nach tatsächlichem Bedarf evaluiert):

```text
Linux   x86_64
Linux   ARM64
macOS   x86_64
macOS   ARM64
Windows x86_64
```

Diese Plattformen werden durch den Cargo-Workspace (Rust-Target-Triples) abgedeckt;
zusätzliche Plattformen ergeben sich aus der bestehenden CMake-/Conan-Matrix (die für die
C++-Wrapper-/Packaging-Flows erhalten bleibt) und aus der Rückwärtskompatibilität zur
C++-Referenz.


## 16.3 Format

- **TIFF/BigTIFF**: eigenständiger Parser/Writer; `CompressionKind` für None/Lzw/Deflate/Jpeg
  (+ optional ZSTD, gemäß RFC-0011).
- **PTIFF-Extensions**: die fünf Private-Tags 65001–65005 mit RFC-7002-Payload-Codec; Lesen
  toleriert unbekannte Versionen, Schreiben deterministisch (sortierte Keys).
- **Wissenschaftliche Metadaten**: Camera, CRS, SPICE, Scientific-Layers, Provenance-Felder
  konservativ preserved.

## 16.4 Quality

- **Kompatibilitätstests**: Golden-, Binary-, Metadaten-, Pixel-, Roundtrip-, Kompressions-,
  Corrupted-File-, Fuzz- und Cross-Language-tests (§11) grün.
- **Fuzzing**: `cargo fuzz`-Ziele für TIFF-Parser und Decoder (kein Panic, keine OOM,
  keine falsche Interpretation).
- **Benchmarks**: C++ vs. Rust auf identischen Fixtures in `benchmarks/benchmark-results/`
  (§12); keine Regression auf den relevanten Pfaden.
- **Dokumentation**: RFCs und Doxygen/Rustdoc; API-Nutzungsbeispiele pro Sprache.
- **API-Stabilität**: SemVer für C-ABI + Rust + C++ + Python + Octave; Breaking Changes nur
  per Major.

## 16.5 Distribution

- **Source**: Open Source (Apache-2.0) über Cargo-Workspace + Repository.
- **Binaries**: über Cargo `--release` + bestehendes CPack (deb/rpm/pkg/nix) ± Cross-Target.
- **Python-Wheels**: `maturin` pro Plattform (PyPI wheel).
- **C/C++-Entwicklungspaket**: `ptiff-c` + `ptiff-cpp` inkl. `libptiff_c`/C-Header/CMake-Package.
- **Octave-Package**: `.m`+`.oct` via `mkoctfile` gebündeltes Package (sofern sinnvoll).

**Packaging wird zu einem First-Class-Ziel.** Die Rust-Migration soll die Distribution
verbessern, nicht nur die Implementierung. Die gewünschte Nutzererfahrung je Sprache:

```text
Python:   pip install ptiff
Rust:     cargo add ptiff
C/C++:    install libptiff + ptiff.h
Octave:   install PTIFF package
```

Der Nutzer soll dafür nicht verstehen müssen:

```text
CMake, Conan, C++23, libstdc++
```

Wo native Abhängigkeiten (z. B. libjpeg-turbo, SPICE) nötig bleiben, werden sie **isoliert und
als Teil der unterstützten Plattform-Artefakte** mitgeliefert, wo rechtlich und technisch
angemessen (bzw. als klar dokumentierter System-Dependency). Dies wird als Architektur-
Anforderung behandelt (§3.1.2), nicht als nachgelagerte Optimierung.


---

# 17. Migration Phases

Dieser Abschnitt adaptiert die vorgeschlagene Phasenliste an die reale Repository-Struktur.
**Jede Phase endet mit funktionierendem Zustand** (Regressionstests grün, beides kompilierbar).

## 17.0 Abgleich mit `ROADMAP.md`

**Wichtig:** `ROADMAP.md` und dieses Dokument sind **zwei unterschiedliche Sichten**, die sich
ergänzen, nicht widersprechen. Sie können direkt vereinbart werden:

- **`ROADMAP.md`** ist die **Feature-/Produkt-Roadmap**, geordnet nach Meilensteinen M0–M4
  (funktionale Fähigkeiten: Format/Backends/Extension-Domänen/Ausreifung). Sie beschreibt **was**
  PTIFF kann und ist format-, sprach- und **implementierungsneutral** (heute C++).
- **Dieses Dokument (§17)** ist die **Implementierungs-Roadmap** der Rust-Migration, geordnet
  nach Phasen 0–13 (technische Umsetzung des Kerns). Sie beschreibt **wie** die C++-Referenz
  schrittweise durch den Rust-Core ersetzt wird.

Daher muss **ROADMAP.md nicht umgeschrieben werden, nur referenziert und inhaltlich ergänzt.**
Die Zuordnung (M-Meilsteine ↔ Phasen):

| ROADMAP.md | Inhalt (funktional) | Status heute | Korrespondierende §17-Phasen | Stand der Migrationsplanung |
|------------|---------------------|--------------|-------------------------------|------------------------------|
| **M0** Grundstein | Build, API-Kern, Domain-Modell, Storage-Architektur, Backend-Stubs, Governance | ✅ abgeschlossen (C++) | Phase 1/2 (Workspace, Datenmodell, Grundgerüst) | Muss **funktional äquivalent** in Rust reproduziert werden (gleiche Feld-Semantik) |
| **M1** TIFF/BigTIFF r/w | Header/IFD/Tags, Kompression (LZW, PackBits, Deflate, JPEG), Schreiben, Mehrbild | ✅ abgeschlossen (C++) | Phase 3/4 (TIFF-Kern, Kompression) | Golden-/Roundtrip-Äquivalenz zur C++-Referenz als DoD |
| **M2** Weitere Backends | PDS4, ISIS3 CUB, Zarr, OpenEXR, Memory, Cloud-Read | ✅ abgeschlossen (C++) | Phase 4 (ZSTD/Zarr), Phase 5 (HTTP-Range), Phase 11 (CLI) | Backends als Rust-Features; Funktion bleibt, Implementierung wechselt |
| **M3** Extension-Domänen | Cam., CRS, SPICE, Scientific-Layers, Provenienz; + Stereo/Mesh/Multi-File offen | 🚧 teilweise (C++) | Phase 6 (Metadaten/PTIFF-Tags 65001–65005), Phase 2 | Die **Tag-Serialisierung** (65001–65005, RFC-7002) wandert mit; die **normativen Feld-Schemata** und **Domain-Klassen** sind weiterhin RFC-/Design-Aufgaben und **unabhängig von der Sprache** – sie bleiben sowohl für C++ als auch Rust offen |
| **M4** Ausreifung & Verbreitung | Stable-Core-RFCs, Konformität, GIS-Integration, Referenzdaten, Performance, „GeoTIFF der Planeten“ | ☐ teils offen | Phase 12/13 (Kompatibilität/Benchmarks, 1.0), weitere nach 1.0 | Teilweise ein **Post-1.0-Ziel** (GIS-/Community-Ausbreitung); 1.0 deckt Core/Conformance/Distribution ab (§16) |

**Konsequenzen für den Migrationsplan:**
1. **M0–M2 sind die technische Basis**, die in Rust funktional reproduziert werden muss. Erst
   wenn sie auf dem Rust-Core laufen, ist eine Feature-Parität erreicht → das ist der Kern des
   DoD von Phase 3–6/12.
2. **M3 hat zwei Aspekte:** Die **Container-/I/O-Ebene** (Tags + RFC-7002-Codec) ist Bestandteil
   der Migration (Phase 6). Die **fachliche Ebene** (Schema-Standardisierung pro Domäne,
   Domain-Klassen) ist eine **RFC-/Design-Aufgabe**, die unabhängig von der Implementierungssprache
   voranschreitet und parallel zur Rust-Migration laufen kann.
3. **M4** ist überwiegend ein **Vor-/Nach-1.0-Kontinuum:** Die Konformitäts-/Golden-/Benchmark-Teile
   (`tests/golden/`, `tests/conformance/`, Performance) sind Pflichtteile von PTIFF 1.0 (§16.4),
   während GIS-/Community-/„GeoTIFF der Planeten“-Ausbreitung nach 1.0 adressiert werden.

**Empfohlene Pflege:** `ROADMAP.md` behält in den Meilenstein-Beschreibungen den funktionalen
Fokus und wird um einen kurzen Hinweis ergänzt (z. B. „Milestones sind implementierungsneutral;
die aktuelle Referenzimplementierung ist C++ `libptiff`, Zielimplementierung ab 1.0 ist der
Rust-Core – Details siehe `PTIFF-1.0-RUST-CORE-PLAN.md` §17“). Die Meilenstein-Beschreibungen
selbst bleiben unverändert gültig.

## 17.1 Migrations-Status (Stand: 2026-08-24, aktualisiert)

> **Update 2026-08-24 (nachmittags): `libptiff` ist bereits gelöscht.** Commit `3fa458d`
> ("refactor: replace C++ libptiff with pure-Rust workspace (Phases B-D)") hat `libptiff/`
> (244 Dateien) sowie `bindings/c/` komplett entfernt und den Workspace auf Version `1.0.0`
> gesetzt (Tag `v1.0.0-alpha.1`). **Das lief der eigenen Reihenfolge-Empfehlung dieses
> Dokuments zuwider** (s. u.: "Phase 12 vor 13 zwingend") — Phase 12 (systematische
> Cross-Validation-Benchmarks ggü. C++-Oracle) wurde nicht als eigener, dokumentierter Schritt
> durchgeführt, bevor der C++-Oracle selbst entfernt wurde. Was tatsächlich an Äquivalenznachweis
> existiert: golden-digest-Tests (eingefroren vor der Löschung) und die R4-Bindings-Tests gegen
> ein eingefrorenes Oracle-Fixture (`scripts/samples/ptiff_interop_fixture.tif`), s. Phase 7/12
> unten — das ist eine punktuelle, keine systematische Cross-Validation. `ROADMAP.md` ist ab
> jetzt das primär gepflegte, autoritative Status-Dokument; dieser Abschnitt (§17.1) wird nur noch
> nachgezogen, nicht mehr führend fortgeschrieben.

**Ursprüngliches Kriterium für „libptiff löschen“ (Phase 13 DoD, s. u.): „C++-Referenz wird
archiviert (nicht weiter maintained), Rust ist Single-Core.“** In der Praxis wurde `libptiff`
nicht archiviert, sondern ersatzlos gelöscht (s. Update oben) — das Kriterium ist damit
**übererfüllt, aber außer der Reihe** erfüllt. Status unten durch tatsächlichen Build+Testlauf
verifiziert (`cargo build --workspace --all-features`, `cargo test --workspace --all-features`:
515/515 grün), nicht nur aus Doku übernommen.

| Phase | Ziel | Status | Anmerkung |
|-------|------|--------|-----------|
| 0 | Analyse | ✅ | dieses Dokument |
| 1 | Workspace + Grundgerüst | 🚧 | `ptiff-core` + `ptiff` (Rust-API, `crates/ptiff-rust`) + `ptiff-c` (C-ABI, Version/Error/Pixel-Type-/Compression-Enums/Image/Backend) da — Workspace kompiliert und testet; Grundgerüst i. W. fertig |
| 2 | Datenmodell (StorageModel + Domain-Typen) | 🚧 | `Scene`/`Image`/`StorageModel`/`Serializer`/`Deserializer` fertig; `Camera`/`CoordinateReferenceSystem`/`Geometry` existieren als eigenständige Rust-Typen, sind aber **nicht** in `Scene` verdrahtet (kein `addCamera`/`addGeometry`) — DoD nicht vollständig erfüllt |
| 3 | TIFF/BigTIFF-Kern | ✅ | TIFF/BigTIFF-Header/IFD/Directory/Tile-Layer fertig (`TiffBackend`, IFD-Kette lesen/schreiben, Mehrbild); **ISIS3 CUB-, PDS4-, OpenEXR- und Zarr-Backend alle fertig** (2026-08-24, `crates/ptiff-core/src/io/backend/{isis,pds4,openexr,zarr}/`) — **alle 4 C++-Backends sind damit portiert, C++-Gegenstück existiert nicht mehr** (s. Update oben). Plus: Cloud-Object-Storage-Lesetransport (`HttpRangeBinaryReader`) laut `ROADMAP.md` Meilenstein 2 ebenfalls fertig, war in dieser Phasenliste ursprünglich nicht vorgesehen. Verifiziert via `cargo build --all-features` + `clippy` + `fmt`, 515/515 Tests grün im Gesamt-Workspace |
| 4 | Kompression | ✅ | PackBits/LZW/Predictor (dependency-frei) fertig; **Zarr-ZSTD fertig** (in `backend/zarr/codec.rs`, als Zarr-Chunk-Kompression via `zstd`/`flate2`); **Deflate fertig** (`backend/tiff/compression/deflate.rs`, via `flate2`/zlib); **JPEG fertig** (`backend/tiff/compression/jpeg.rs`, reines Rust via `jpeg-encoder`/`jpeg-decoder`, feature-gated hinter `tiff-codecs`); alle Codecs roundtrip- und (verlustfrei) golden-getestet; **ZSTD als TIFF-Codec offen** (RFC-0011) |
| 5 | Tiles / parallele Verarbeitung (Rayon) | ❌ | nicht begonnen — war die **Hauptmotivation** der Migration (§2.1 Punkt 1); der Rewrite hat seinen eigenen Kernvorteil bislang nicht eingelöst |
| 6 | Metadaten-Erweiterungen (65001–65005) | 🚧 | RFC-7002-Codec (encode/decode) fertig und getestet; Scene-Verdrahtung offen (s. Phase 2) |
| 7 | **C-ABI (`ptiff-c`)** | 🚧 **erster Slice committet, C-Fähigkeitstest + Logger/Camera grün** | der **einzige harte Blocker** für „libptiff löschen“. Neues Crate `crates/ptiff-c` (cdylib+staticlib+rlib) implementiert die handgepflegten C-Header in `bindings/c/` unverändert: Version-ABI, Image-Bridge, Pixel-Bridge lesen+schreiben, Backend-Names, `ptiff_open_path` — **33 Unit-Tests grün** → 38 unit + 2 integration (flache Feld-Sicht, s. u.). **C-ABI-Fähigkeitstest gegen echtes C-Programm ✅** (`crates/ptiff-c/tests/c/ptiff_c_abiltest.c` gegen `staticlib`). **Logger ✅:** `ptiff_logger_*` forwarden jetzt auf den dependency-freien Core-Logger (set/level/log, Level-Ordering = C-Header). **Camera ✅:** `ptiff_open_path_camera` (read) + `ptiff_sink_create_camera` (write) über das strukturierte Camera-Domain; dafür die `ptiff.camera.*`-Feldnamen im Core-Marshal auf den C-ABI/Oracle-Kontrakt (`focal_length_x`, `rotation_*`, `position_*`) ausgerichtet (vorher `focal_px`/`rot_*`/`pos_*` — schematische Divergenz zum C++-Oracle geschlossen, Golden-Digest regeneriert). **Flache Feld-Sicht ✅ (2026-08-24):** `ptiff_open_path_fields` + `ptiff_fields_free` + `ptiff_field` implementiert (`crates/ptiff-c/src/metadata.rs`): re-derive der formatneutralen `StorageModel` über `TiffBackend::deserialize_model` (C++-Oracle-Vertrag), liefert alle `ptiff.<domain>.<name>`-Felder aus Tags 65001-65005 lexikographisch; 5 neue Unit-Tests (Null-Arg/Empty/NotFound/Roundtrip/Free) + 2 Integrationstests gegen den **C++-Oracle-Fixture** `scripts/samples/ptiff_interop_fixture.tif` grün (exakt die vom Go/Python/Ruby-Binding erwarteten `ptiff.camera.*`/`ptiff.spice.frame`-Werte). **38 Unit + 2 Integration + 1 C-Fähigkeitstest grün.** **R4-Bindings-Kompatibilitäts-Nachweis ✅ (2026-08-24):** Go/Python/Ruby tatsächlich gegen das Rust-`libptiff_c` laufen lassen — **Go 25/25, Python alle, Ruby alle bis auf 1 erwartete Version-Diff** (`test_runtime_version_fields` pinnt hart 0.3.0 = C++-Oracle-Version; Rust-ABI meldet korrekt 1.0.0; Schema funktioniert, runtime==compile). Dafür drei echte Rust-Gaps geschlossen: `ptiff_sink_create("")` leerer Pfad → NULL; `camera_from_model` Extrinsics optional (intrinsics-only OEM); `crs_from_model` akzeptiert RFC-0004-Schema (`ptiff.crs.body`/`projection`/`reference_frame`) aus dem Oracle-Fixture. Reproduzierbar via `CARGO_TARGET_DIR=/tmp/...` + `libptiff_c.pc` + `make {go,python,ruby}`. **Octave-Binding ✅ (2026-08-24, R4-Erweiterung):** `ptiff.oct` über SWIG gegen das Rust-`libptiff_c` neu gebaut (rpath → `target/debug`, sauberer Workflow ohne `/tmp`-Override) — **alle 8 Octave-Tests grün** (`run_tests_octave`): backend/error/image/logger/roundtrip/sink/version/wrapper; `test_wrapper` deckt die idiomatische Schicht (`Metadata`/`Camera`/`Image`) über den Oracle-Fixture ab und `test_sink` verifiziert den R4-Fix (Empty-Path→NULL); `test_version` prüft nur runtime==compile (kein hart kodierter Oracle-Version-Check), läuft daher gegen Rust-1.0.0 grün. **CRS-Writer RFC-0004 ✅ (2026-08-24):** `crs_fields` emittiert jetzt genau das normative RFC-0004-Schema (`ptiff.crs.body`/`projection`/`reference_frame`, `reference_frame` = effektives Frame), statt des Extended-Schemas (`planet_name`/`iau_id`/ellipsoid/`frame`/`param.*`) — geschlossene schematische Divergenz zum Oracle-Fixture; dokumentiert lossy (Name wird vom NAIF-`body` re-deriviert, Ellipsoid/Projektionsparameter werden nicht vom RFC-Schema getragen → UNSPECIFIED/unset); Reader liest weiterhin beide Schemata (Legacy-Extended rückwärtskompatibel). 3 neue Marshal-Tests (RFC-0004-Emissions-Konformanz, effektives Frame, Legacy-Extended-Read) + Byte-Roundtrip-Integrationstest angepasst. **498 Workspace-Tests grün, clippy + fmt clean.** Damit ist Löschen von `libptiff` deutlich entriskt (nicht mehr „vier tote Sprachbindings“). |
| 8 | C++-Wrapper (`ptiff-cpp`) | ❌ | nicht begonnen |
| 9 | Python (PyO3) | ❌ | nicht begonnen |
| 10 | Octave (MEX über C-ABI) | ❌ | nicht begonnen |
| 11 | CLI auf `ptiff-core` | ✅ | `ptiff-cli/` läuft über das idiomatische `ptiff`-Crate direkt auf `ptiff-core` (`Tiff::open`/`read_image_pixels`/`tile_layout`/`to_bytes_with_pixels`); Reports Version über `ptiff::{APP_VERSION, VERSION_STR}`. Keine C-ABI/C++-Abhängigkeit mehr. 9 Unit + 8 Integrationstests grün |
| 12 | Kompatibilität / Benchmarks (Cross-Validation ggü. C++-Oracle) | 🚧 **punktuell, nicht systematisch** | keine dedizierte Cross-Validation-Benchmark-Suite wie in §5/§17.0 vorgesehen; was existiert: golden-digest-Tests (vor der Löschung eingefroren) + R4-Bindings-Tests (Go/Python/Ruby/Octave) gegen ein eingefrorenes C++-Oracle-Fixture (`scripts/samples/ptiff_interop_fixture.tif`). Da `libptiff` inzwischen gelöscht ist, ist ein Nachholen der ursprünglich geplanten systematischen Cross-Validation **nicht mehr möglich**, außer durch Wiederherstellen von `libptiff` aus der Git-Historie |
| 13 | PTIFF 1.0 (C++ archivieren) | ✅ **außer der Reihe erreicht** | `libptiff` in Commit `3fa458d` gelöscht (nicht nur archiviert), Workspace auf `1.0.0` gesetzt, Tag `v1.0.0-alpha.1` existiert. **Voraussetzung „Phase 12 vor 13" wurde nicht eingehalten** (s. Update-Hinweis oben) — Phase 5 (Parallelisierung, weiterhin ❌) ebenfalls nicht vor 13 nachgeholt, entgegen der Reihenfolge-Empfehlung unten |

**Zusätzlicher, phasenübergreifender Gap:** Die N-Band-Multispektral-Erweiterung von
`samplesPerPixel` (C++ `libptiff` ≥ 0.4.0, motiviert durch anstehende Merkur-Spektrenbilddaten,
s. `documents/paper/ptiff/ptiff.tex` §Begrenzungen) ist im C++-Kern implementiert und getestet,
im Rust-Kern (Phase 3/4) **noch nicht nachgezogen** — weiterer, konkreter Parity-Gap zwischen den
beiden Implementierungen, unabhängig von den 13 Phasen oben zu schließen.

**Reihenfolge-Empfehlung:** Phase 7 vor 8–11 (alle Bindings-Phasen hängen an der C-ABI); Phase 12
vor 13 zwingend (kein „libptiff löschen“ ohne bewiesene Parität). Phase 5 (Parallelisierung)
kann parallel zu 7–11 laufen, sollte aber vor Phase 13 nachgeholt werden, sonst liefert 1.0 den
eigenen Haupt-Business-Case der Migration nicht.

## Phase 0 – Analyse (done in diesem Dokument)
- **Ziel:** Repository + Anforderungen verstanden, Architekturentscheidungen votiert.
- **Betroffen:** alle Module (Bestand).
- **Abhängigkeiten:** keine.
- **Tests:** Bestandstests als Baseline-Hash.
- **Risiko:** Entscheidungen falsch (mitigiert durch §20-Open-Questions).
- **DoD:** Dieses Dokument abgenickt, Zielarchitektur verabschiedet.

## Phase 1 – Rust-Workspace + Grundgerüst
- **Ziel:** leeres Cargo-Workspace mit `ptiff-core` (Fehler/Rust) + `ptiff-c` (leere C-ABI-Header)
  + `ptiff` (Rust-API) anlegen; CI baut und testet.
- **Betroffen:** neue Crates; bestehendes C++ bleibt unberührt.
- **Abhängigkeiten:** Cargo; keine externen C-Deps nötig.
- **Tests:** `cargo test` Basis; Version/Error-Konformität zur C++-API.
- **Risiko:** niedrig.
- **DoD:** Workspace kompiliert, C-ABI-Header (Version/Fehler) existieren, CI-Matrix grün.

## Phase 2 – PTIFF-Datenmodell (StorageModel + Domain-Typen)
- **Ziel:** `Scene`, `Image`, `Metadata`, `Camera`, `CRS`, `ScientificLayer` + `StorageModel`/
  `Serializer`/`Deserializer` in Rust portieren; identische Feld-Semantik.
- **Betroffen:** `libptiff/include/ptiff/{scene,image,metadata,geometry,...}` → `crates/ptiff-core`.
- **Abhängigkeiten:** nur std + eigene Crates.
- **Tests:** Unit-Tests äquivalent zu `tests/unit`; Übertragungs-Tests der Metadaten-Struktur.
- **Risiko:** gering (meist Datenstrukturen).
- **DoD:** Domain-Modell-Tests grün, Metadaten-Vergleich-Roundtrips mit C++-Referenz grün.

## Phase 3 – TIFF/BigTIFF-Kern
- **Ziel:** Header/IFD/Tag-Parser, Directory, Endianness, BigTIFF, Tile/Strip-Lesen.
- **Betroffen:** `libptiff/.../backend/tiff/` → Rust.
- **Abhängigkeiten:** Phase 2; keine externen C-Deps.
- **Tests:** Golden-Read-Tests gegen C++-Referenzdateien; Corrupted-File-Entwürfe.
- **Risiko:** Endianness/Overflow-Bugs (durch Property-Tests + Fuzz).
- **DoD:** Rust liest C++-erzeugte TIFF/BigTIFF byte-identisch (Metadaten + Pixel unkomprimiert).

## Phase 4 – Kompression
- **Ziel:** PackBits, LZW, Predictor (eigene), Deflate (flate2), JPEG, Zarr-ZSTD.
- **Betroffen:** `src/compression/*` → Rust-Features.
- **Abhängigkeiten:** flate2, zstd, jpeg-Encoder/-Decoder (Status: erledigt — JPEG wurde als
  **reines Rust** (`jpeg-encoder`/`jpeg-decoder`) umgesetzt, nicht libjpeg-turbo; Determinismus-
  Überlegungen s. §5.3/§14).
- **Tests:** Roundtrip je Codec cross-implementiert; Golden (verlustfrei byte-exakt, JPEG
  Pixel-Toleranz).
- **Risiko:** JPEG-Determinismus (mitigiert §5.3/§14).
- **DoD:** alle Codecs lesen/schreiben C++-kompatible Daten; Kompressionstests grün.

## Phase 5 – Tiles / parallele Verarbeitung
- **Ziel:** Tile-Layout genau wie C++; paralleles Lesen (Rayon) und Komprimieren (Rayon);
  Thread-Safety-Modell (§6).
- **Betroffen:** `ptiff-core` I/O-Routen; `ptiff-c` erhält parallele Laufzeit.
- **Abhängigkeiten:** rayon, memmap2, channel.
- **Tests:** parallele vs. sequentielle Ergebnisse identisch; Thread-Safety-Tests.
- **Risiko:** Race-Conditions (mit Property-Tests + TSAN-äquivalenten Rust-Tests).
- **DoD:** Parallel-Mode aktiv, Ergebnisse deterministisch, Benchmarks gegen C++ gemessen.

## Phase 6 – Metadaten / wissenschaftliche Erweiterungen
- **Ziel:** PTIFF-Tags 65001–65005, RFC-7002-Codec, Camera/CRS/SPICE/Layer/Provenance-Felder.
- **Betroffen:** `ptiff_metadata.*` → Rust; Domain-Felder.
- **Abhängigkeiten:** Phase 3/4; serde optional.
- **Tests:** Metadaten-Vergleich gegen C++-Referenz; Golden-Payloads.
- **Risiko:** wenig.
- **DoD:** Rust erzeugt/liest dieselben PTIFF-Metadaten-Payloads wie C++.

## Phase 7 – C ABI (ptiff-c)
- **Ziel:** stabile C-Header + `extern "C"`-Implementierung über den Rust-Core; opaque handles,
  Fehler-/Ownership-, Thread-Safety- und Versionierungssemantik (§7).
- **Betroffen:** `bindings/c/` (Header) + neuer `crates/ptiff-c`.
- **Abhängigkeiten:** Phase 3–6.
- **Tests:** C-ABI-Tests (C-Fähigkeitstest), ABI-Kompatibilitätstest (bestehende Go/Ruby-Bindings).
- **Risiko:** ABI-Versprechen (SemVer).
- **DoD:** C-ABI funktional über Rust; bestehende Bindings laufen unverändert dagegen.

## Phase 8 – C++-Wrapper (ptiff-cpp)
- **Ziel:** neuer moderner C++-Wrapper über C-ABI; alte API-Namen (`Image`, `Scene`, `Reader`,
  `Writer`, `Result`, `Error`) unter neuem Namen ohne Kollision.
- **Betroffen:** `libptiff` (bleibt parallel), neuer `ptiff-cpp`.
- **Abhängigkeiten:** Phase 7.
- **Tests:** C++-Wrapper-Tests (RT), API-Paritätstests gegen bestehende C++-Tests.
- **Risiko:** API-Inkompatibilität (DoD: gleiche Semantik wie C++-Referenz).
- **DoD:** C++-Nutzer können via `ptiff-cpp` auf Rust-Core zugreifen, C++-Referenz läuft weiter.

## Phase 9 – Python (PyO3)
- **Ziel:** PyO3-Binding (`maturin`) + NumPy-Tile-API; SWIG-Python bleibt temporär.
- **Betroffen:** neue `crates/ptiff-python`, bestehendes `bindings/python` (Rückfall).
- **Abhängigkeiten:** Phase 7/15 (C-ABI bzw. direkt ptiff-core).
- **Tests:** bestehende Python-Tests + Feature-Paritätstest (Version/Logger/Image/Source/Sink/
  Metadata/Camera/PDS-Layer).
- **Risiko:** Feature-Parität mit SWIG-Bindung.
- **DoD:** PyO3-Modul erreicht vollständige Parität; SWIG-Python wird deprecated (nicht sofort
  gelöscht).

## Phase 10 – Octave (MEX via C-ABI)
- **Ziel:** eigener `ptiff-octave`-Adapter (C++-MEX + `.m`-API) über ptiff-c; SWIG-Octave bleibt
  Rückfall.
- **Betroffen:** `bindings/octave/`.
- **Abhängigkeiten:** Phase 7.
- **Tests:** Octave-Roundtrip + Feature-Paritätstest zur SWIG-Bindung.
- **Risiko:** Plattform (Windows-Octave).
- **DoD:** `ptiff-octave` Feature-Parität; beide nutzen dieselbe ABI.

## Phase 11 – CLI
- **Ziel:** `ptiff-cli` auf `ptiff-core` umstellen (statt nur über bindings/rust FFI), CLI-
  Fläche erweitern (info, copy, tile-list, metadata, thumbnail).
- **Betroffen:** `ptiff-cli/`.
- **Abhängigkeiten:** Phase 3–6.
- **Tests:** CLI-Behavior-Tests (CLI-Benchmarks erweitert).
- **Risiko:** niedrig.
- **DoD:** CLI funktional auf Rust-Core, alte CLI-Eingaben kompatibel.

## Phase 12 – Kompatibilität / Benchmarks
- **Ziel:** vollständige Cross-Validation C++-Oracle vs. Rust; Benchmark-Parität und Regression
  (§11/§12).
- **Betroffen:** `tests/`, `conformance/`, `benchmarks/`.
- **Abhängigkeiten:** Phasen 3–11.
- **Tests:** alle Cross-Pfade + überwachte Hypothese-Suite (§12) grün.
- **Risiko:** Performance-/Kompatibilitäts-Drift.
- **DoD:** alle Golden-/Fuzz-/Conformance-Tests grün; Benchmark-Hypothesen erfüllt oder
  dokumentierte Abweichung.

## Phase 13 – PTIFF 1.0
- **Ziel:** Definition §16 erfüllt; Release-Prozess (Cargo + CPack + maturin + octave-pkg)
  etabliert.
- **Betroffen:** gesamtes Produkt.
- **Abhängigkeiten:** Phasen 0–12.
- **Tests:** Release-Suite.
- **Risiko:** Verpackungsprobleme.
- **DoD:** PTIFF-1.0-Kriterien (§16) erfüllt; C++-Referenz wird archiviert (nicht weiter
  maintained), Rust ist Single-Core.

---

# 18. Weitere technische Entscheidungen (konsolidiert)

- **Rust soll der Core werden**, nicht lediglich zusätzlicher Wrapper – implementiert über
  `ptiff-core`.
- **C-ABI zentrale Interoperabilitätsschicht** – einzige Sprachbrücke für Nicht-Rust.
- **Python und Octave First-Class-Integrationen** – PyO3 (Python) + MEX-Adapter (Octave) auf
  derselben C-ABI.
- **Migration schrittweise, jederzeit funktionsfähig** – jede Phase endet grün.

---

# 19. Risiken

| # | Risiko | Schwere | Mitigation |
|---|--------|---------|------------|
| R1 | **JPEG-Determinismus** (Lib-Version/CPU-abhängig) | hoch | libjpeg-turbo als Standard; Golden nur für verlustfreie; Pixel-Toleranztests; Doku zur Bitgenauigkeit |
| R2 | **Endianness/Overflow-Bugs** im eigenen TIFF-Parser | hoch | byte-orientierte Tests, Property-Tests, Fuzzing aus Phase 3 |
| R3 | **Thread-Safety-/Race-Bugs** | hoch | klare Ownership (§6), Parallel- vs. sequentielle Gleichheitstests, TSAN-ähnliche Rust-IPC |
| R4 | **ABI-Brüche** für bestehende Bindings (Go/Ruby) | mittel | SemVer + `PTIFF_ABI_VERSION`; Bindings laufen gegen stabilen C-ABI |
| R5 | **Feature-Parität Python/Octave** (SWIG-Rückfall löst langfristig ab) | mittel | Schrittweise PyO3/MEX-Adapter, Paritäts-Tests je Phase |
| R6 | **Cargo-Feature-Komplexität** (zu viele Cargo-Flags) | niedrig | wenige, klar definierte Features |
| R7 | **Build-Zeit** entgegen Erwartung nicht besser | mittel | sccache/CI-Cache; nur echte Deps aktivieren; Metriken in §12 |
| R8 | **Reproduzierbarkeit** (archiv) | mittel | deterministischer Default (kein simd-Feature im Kern), RFC-7002-kanonische Kodierung |
| R9 | **Wissensverlust** bei Portierung feiner Logik (LZW/Predictor-Semantik, Extrasamples) | mittel | Querverweise zu C++-Quellen im Code; Goldene + Cross-Validation |
| R10 | **libcurl→ureq Semantik** (Range-Verhalten, Redirect, Auth) | niedrig | Zusätzliche HTTP-Integrationstests in Phase 5/12 |

---

# 20. Open Questions

Diese Punkte muss das Team vor Beginn der Implementierung klären, ohne die zentrale
Architektur zu gefährden:

1. **ZSTD als TIFF-Codec?** Ja/Nein je nach RFC-0011. (Heute nur Zarr.)
2. **JPEG-Ziel-Encoder exakt libjpeg-turbo?** Falls ja, welche Version als Minimum? Einfrieren
   der DCT-/Quantisierungskonfiguration für Determinismus?
3. **`cxx`?** Auch wenn ich es für die zentrale C++-Brücke ablehne – für bestimmte Use Cases
   (schnelle, typsichere interne Rust-C++-Pipelines) prüfbar, aber nicht als primärer Pfad.
4. **Wie lautet die genaue C-ABI-Grenze für Resource-ownership?** Out-Buffer wer besitzt was,
   Who frees Strings (bereits `ptiff_free_string` vor). Kann verfeinert werden.
5. **Neuer `ptiff-cpp`-Name mangels Kollision** mit bestehenden `ptiff/*.hpp`-Headern – klar
   benennen (z. B. `ptiff-cpp/lib/include`).
6. **Go/Ruby** bleiben SWIG auf C-ABI – okay? (Ich empfehle ja, da SWIG dort gut funktioniert.)
7. **Determinismus/SIMD**: Soll SIMD-Pfad standardmäßig deaktiviert sein? (§14 sagt ja im Kern.)
8. **Octave-Windows** – ist das ein Release-Ziel? Falls ja, braucht der MEX-Adapter Extra-Arbeit.
9. **Performance-Ziel-Zahlen**: Welche konkreten Zahlen (z. B. „≥2× parallele Decompression“)
   ergeben sich aus der Akzeptanz? (– Baseline aus bestandenen Benchmarks.)
10. **`tiff`-Crate als optionales Interop-Backup** – behalten oder nicht (offen, nicht kritisch).

---

# 21. Zusammenfassung / Fazit

Die bestehende C++-PTIFF-Implementierung ist bereits erheblich fortgeschritten: eigener
TIFF/BigTIFF-Parser, PTIFF-Tags 65001–65005, mehrere Backends, C-ABI, fünf Sprachbindings und
ein Rust-CLI existieren. Die angestrebte Zielarchitektur (Rust-Core → stabile C-ABI →
C++/Python/Octave/Rust) baut genau auf dem bereits etablierten C-ABI-First-Muster auf.

**PTIFF 1.0 wird erreicht**, indem wir den heutigen C++-Core schrittweise durch einen
deterministischen, thread-sicheren Rust-Core ersetzen (jederzeit funktionsfähig), die C-ABI als
einzigen Sprach-Interop beibehalten, einen modernen C++-Wrapper und PyO3-/Octave-Adapter über
diese ABI legen, die bestehende C++-Implementierung als Oracle nutzen und alle wissenschaftlichen
Anforderungen (Endianness, Float/NaN, Metadaten-Preservation, CRS/SPICE, Provenance,
Reproduzierbarkeit) konservativ absichern. Der Build wird schneller – nicht weil „Rust schneller
kompiliert“, sondern weil Cargo Feature-basiert nur das Nötige baut und schwere C-Bibliotheken
durch kleinere, reine-Rust-Crates ersetzt (libjpeg-turbo bleibt als einzige schwere C-Dependency
für den deterministischen JPEG-Pfad).

**Nächster Schritt:** Phase-1/Workshop, Architekturentscheidungen aus §20 votieren, dann Phase
1 starten.

---

_Ende des Planungsdokuments._
