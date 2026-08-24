# PTIFF Roadmap

Diese Roadmap ordnet die geplanten Arbeiten an der PTIFF-Spezifikation und der
Referenzimplementierung (reiner Rust-Workspace) nach Meilensteinen. Stand des
Projekts (1.0.0, 2026-08-24): die Referenzimplementierung ist vollständig nach
Rust überführt — `ptiff-core` → `ptiff-rust` → `ptiff-cli`/`ptiff-c` — und liest/
schreibt TIFF/BigTIFF inklusive Kompression und Mehrbild-Dokumente, bietet
mehrere weitere echte Backends sowie einen Lese-Transport für
Cloud-Object-Storage. Die frühere C++-Implementierung (`libptiff`) und die
C++-Veneer (`libptiff_c`) sind entfernt; Cargo ist der reale Build, CMake/CPack
nur noch dünnes Packaging.

Die Meilensteine sind bewusst grob und ändern sich mit dem RFC-/Design-Prozess
(GOVERNANCE.md). Es gilt: **Reihenfolge = Priorität**, nicht verbindlicher Termin.

## Legende

- ✅ Erledigt
- 🚧 In Arbeit / Design vorhanden
- ☐ Offen

---

## Meilenstein 0 — Grundstein (abgeschlossen)

- ✅ Build-System: Rust-Workspace (Cargo/CPack; kein C++/Conan mehr im Build)
- ✅ Öffentliche API-Oberfläche & Kern-Typen (`Result`, `Error`, `Version`, `Logger`, ...)
- ✅ In-memory Domain-Modell (`Scene`, `Image`, `Camera`, `CRS`, `Metadata`, `History`, `Spice`, ...)
- ✅ Format-neutrale Speicher-/I/O-Architektur (`StorageModel`, `StorageBackend`, `BinaryReader/Writer`, Tile-Modell)
- ✅ Selbst-registrierende Backend-Stubs (TIFF, PDS4, ISIS, Zarr, OpenEXR, Memory, Cloud)
- ✅ Lizenzierung (Apache-2.0) + Governance-/Community-Dokumente
- ✅ Changelog & kontinuierliche Doku-Pflege

## Meilenstein 1 — TIFF/BigTIFF lesen & schreiben (abgeschlossen)

- ✅ TIFF/BigTIFF-Header, IFD-, Pixel-Format-, Directory-Interpretation, Tile/Strip-Lesen
- ✅ `FileBinaryReader` als erster konkreter `BinaryReader`
- ✅ Kompression: LZW, PackBits, Deflate, JPEG (`Predictor = 2`-Unterstützung)
- ✅ Robustheit/Parser-Härtung gegen missgebildete Dateien (RFC-0001 §13)
- ✅ TIFF/BigTIFF **Schreiben**: `serializeModel`/`openImageSink`,
  Kompressions-Auswahl beim Schreiben, Round-Trip-Tests
- ✅ Mehrbild-Dokumente (IFD-Kette)

## Meilenstein 2 — Weitere Backends (abgeschlossen)

- ✅ **PDS4** — XML-Labels + seekbare Roh-Pixel
- ✅ **ISIS3 CUB** — PDS3-artiges Textlabel, line-basierter Parser
- ✅ **Zarr** — JSON-Header + zstd/zlib-Chunk-Kompression, chunk == tile
- ✅ **OpenEXR** — echte `.exr`
- ✅ **Memory-Backend** — In-Memory-Format "PMEM" + `MemoryBinaryReader/Writer`
- ✅ **Cloud/Objekt-Speicher (lesend)** — HTTP `Range`-Requests via
  `HttpRangeBinaryReader`, Cloud-Optimized Access

## Meilenstein 3 — PTIFF-spezifische Extension-Domänen (RFCs)

Entsprechend RFC-0001 §17 (Open Questions); jede Domäne braucht einen eigenen RFC.
Ab 0.3.0 sind die fünf Private-Tags 65001–65005 als konkreter TIFF-Tag-Output im
Container implementiert (versionierter Payload-Codec, Schreib-/Lesepfad, Roundtrip);
offen bleiben die normativen Feld-Schemata je Domäne (RFC-Standardisierung) und die
fachlichen Domain-Klassen oberhalb des Containers:
- 🚧 **Camera model** — Tag-Serialisierung implementiert; pinhole/fisheye/pushbroom-Schema und Distortion als RFC offen
- 🚧 **CRS / planetare Georeferenzierung** — Tag-Serialisierung implementiert; Körper-IDs, Referenz-Ellipsoide, Projektionen als RFC offen
- 🚧 **SPICE-Integration** — Tag-Serialisierung implementiert; Kernel-Referenzen, Zeit/Position/Pointing, Provenienz als RFC offen
- ☐ **Stereo** — Paar-Beziehungen, relative Pose, Disparität
- 🚧 **AI-layer** — Scientific-Layer-Tag (65004) implementiert, Schema offen
- ☐ **Mesh/3D** — eingebettete/verwiesene Geometrie
- 🚧 **Provenienz/Versionierung** — Tag-Serialisierung implementiert, Schema offen
- ☐ **Multi-File-Beziehungen** — Verweise zwischen verwandten PTIFF-Dateien

## Meilenstein 4 — Ausreifung & Verbreitung

- ✅ Erste Veröffentlichung (`0.2.0`), `v0.2.0`-Tag
   - ✅ `0.3.0`-Release: PTIFF Private-Tags 65001–65005 als IFD-Tag-Output
- ☐ Stabile "Core"-RFCs (Status: Stable)
- ☐ Offizielle Konformitätsstufen & Zertifizierung
- ☐ Einbindung in Tools: GIS, Computer Vision/Photogrammetrie, planetarische Werkzeuge
- ☐ Referenz-Datensätze & Beispiel-Files in `crates/ptiff-core/tests/` (Golden- und Property-Tests)
- ☐ Performance-Optimierungen (tiled/streaming, weitere Cloud-Optimierung)
- ☐ Als das "GeoTIFF der Planetenwissenschaft" etablieren (RFC-0001 §15)

---

## Anmerkungen

- **Stand 1.0.0 (2026-08-24):** vollständige Rust-Migration abgeschlossen — die
  C++-Implementierung (`libptiff/`) und die C++-Veneer (`bindings/c`) sind
  entfernt; `cargo build`/`cargo test` (Workspace, alle Features) sind grün,
  die C-ABI (`ptiff-c`) und die Bindings (Go/Python/Ruby/Octave) laufen über den
  Rust-Kern. Siehe `RUST-WORKSPACE.md` und `CHANGELOG.md`.
- **Stand 0.3.0 (2026-08-17):** erstmals sind die PTIFF-spezifischen Private-Tags
  65001–65005 als konkreter TIFF-Tag-Output implementiert und roundtrip-fest getestet
  (Testsuite 371 grün). Die Container-/I/O-Schicht der Extension-Domänen ist damit
  abgeschlossen; die normative Feld-Schema-Standardisierung pro Domäne sowie die
  Domain-Klassen bleiben M3-Aufgaben.
- **Stand 0.2.0:** Die TIFF-Read/Write-Pfade und die übrigen Kern-Backends sind
  umgesetzt; damit sind reale Planeten-Daten (LROC, LOLA, GeoTIFF) lesbar und
  schreibbar. Der größte Mehrwert hin zu einem produktiven Einsatz liegt jetzt im
  Abschluss der Extension-Domänen (M3) und der Ausreifung (M4).
- **Nicht-Ziele** (RFC-0001 §7): kein Ersatz für PDS3/PDS4-Archivierung, kein ISIS-
  Ersatz, kein neuer Low-level-Container, keine Anwendungs-Algorithmen (Stereo-Matching etc.)
