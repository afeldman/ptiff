# Mitwirken an PTIFF

Danke, dass du zu PTIFF beitragen möchtest! Dieses Projekt lebt von Beiträgen —
sei es Code, Dokumentation, Tests, Bug-Reports oder Design-Feedback. PTIFF ist ein
offener Standard (lizenziert unter Apache-2.0) mit einem transparenten
Governance-Prozess; bitte lies die Dateien `GOVERNANCE.md` und
`CODE_OF_CONDUCT.md`, bevor du loslegst.

## Schnellstart

```bash
# Workspace vollständig bauen, testen und linten (siehe auch README.md)
cargo build --workspace --all-features
cargo test --workspace --all-features
cargo fmt --all -- --check
cargo clippy --workspace --all-features -- -D warnings
```

Die Referenzimplementierung ist ein reiner Rust-Workspace
(`ptiff-core` → `ptiff-rust` → `ptiff-cli` / `ptiff-c`); Cargo ist der reale
Build. CMake/CPack dient nur noch als dünnes Packaging für die Rust-Artefakte.

## Art der Beiträge

| Kategorie | Beschreibung |
|-----------|--------------|
| **Bugs & Verbesserungen** | Einen Bug melden oder einen Fix als Pull Request beisteuern. |
| **TIFF/BigTIFF-Backend** | Lese-/Schreib-Unterstützung für echte TIFF/BigTIFF, Kompression (LZW/PackBits), Tile/Strip-Zugriff. |
| **Weitere Backends** | PDS4, ISIS3 CUB, Zarr, OpenEXR, Memory, Cloud-Object-Storage. |
| **C-ABI / Bindings** | `ptiff-c` (C-ABI über den Rust-Kern) und Go-/Python-/Ruby-/Octave-Bindings. |
| **Spezifikation (RFCs)** | Neue oder geänderte RFCs für Extension-Domänen (camera, CRS, SPICE, stereo, …). |
| **Tests & Konformität** | Unit-, Integrations-, Golden- und Konformitätstests. |
| **Dokumentation** | `ARCHITECTURE.md`, `RUST-WORKSPACE.md`, rustdoc/-Doxygen-Kommentare, Guides, Doku-Fixes. |

## Ablauf bei Code-Änderungen

1. **Issue öffnen** und Vorhaben kurz beschreiben (Bug, Feature, Design-Frage).
2. **Fork + Branch** anlegen (sprechender Name, z. B. `fix/tiff-predictor`).
3. **Code schreiben** nach den Richtlinien unten.
4. **Tests schreiben** — jede Änderung an Produktionscode braucht (mindestens) einen
   Test, der sie absichert.
5. **Build & Tests lokal ausführen**, bis alles grün ist (`cargo test`, `fmt`, `clippy`).
6. **Pull Request** erstellen, der auf das zugehörige Issue verweist.

## Coding-Richtlinien (Rust)

- **Sprache / Stil:** stabiler Rust (MSRV 1.97); Code folgt `cargo fmt` (rustfmt)
  und `cargo clippy` mit `-D warnings` (in CI durchgesetzt).
- **Safety:** der Kern (`ptiff-core`, `ptiff-rust`) hat `#![forbid(unsafe_code)]`.
  Unsafe ist nur dort erlaubt, wo es nötig ist (FFI in `ptiff-c`), und dann
  minimal, kommentiert und an der Rust-Sicherheitsgrenze.
- **Fehlerbehandlung:** jede fehlbare Operation gibt `Result<T, Error>` zurück
  (Core-`Error`/`ErrorCode`); kein `unwrap`/`panic`/`expect` auf Laufzeitpfaden
  in der Bibliothek, keine Panics bei missgebildetem Input.
- **Öffentliche API:** öffentliche Symbole sind mit `pub`-Doku versehen und
  dokumentieren das *Warum*, nicht nur die Signatur (`#![warn(missing_docs)]`).
- **Dateistruktur / Module:** kleine, fokussierte Module; Domain-Typen im Kern,
  C-ABI strikt im separaten `ptiff-c`-Crate (nicht im Kern).
- **Conventions:** Dokumentation und Commit-Meldungen auf Englisch; Tests mit
  `#[test]` (Unit-Tests inline in `src/`), `tests/`-Integrations-Crates und
  `crates/ptiff-core/tests/golden.rs` für byte-exakte Golden-Digests.

## Spezifikations-Änderungen (RFCs)

PTIFF ist ein Standard; normative Änderungen an der Spezifikation laufen über den
**RFC-Prozess** (siehe `GOVERNANCE.md`). Reine Implementierungs-Änderungen an der
Rust-Referenzimplementierung brauchen keinen RFC, solange sie die Standardsignatur
nicht verändern. Neue Extension-Domänen oder Tag-Allokationen erfordern einen
eigenen, unabhängig versionierten RFC.

## Test-Ablauf

| Ebene | Zweck |
|-------|-------|
| Unit-Tests (`src/**/*.rs` in jedem Crate) | Domain-/Wert-Typen und einzelne Komponenten. |
| Integration `crates/*/tests/` | Zusammenspiel mehrerer Komponenten. |
| Golden (`crates/ptiff-core/tests/golden.rs`) | Regressionsprüfung gegen bekannte "goldene" Byte-Digests. |
| Property (`crates/ptiff-core/tests/property.rs`) | Eigenschaftsbasierte Tests (Roundtrips, Invarianzen). |
| C-ABI (`crates/ptiff-c/tests/c`) | C-Akzeptanztest gegen den generierten `ptiff_c.h`-Header. |

Einzelnen Test ausführen:

```bash
cargo test -p ptiff-core --test golden          # z. B. Golden-Digests
make -C crates/ptiff-c/tests/c test             # C-ABI "ALL OK"
```

## Kommunikation / Verhaltensregeln

Bitte lies die `CODE_OF_CONDUCT.md`. Grundsatz: respektvoll, konstruktiv,
wissenschaftlich ehrlich. Das Projekt bedient eine Community aus Raumfahrt,
Wissenschaft und Industrie — der Ton ist kooperativ und fachlich.

## Danksagung

Anleitung adaptiert nach dem
[GitHub-Handbuch für Contribution Guidelines](https://docs.github.com/en/communities/setting-up-your-project-for-healthy-contributions/setting-guidelines-for-repository-contributors).
