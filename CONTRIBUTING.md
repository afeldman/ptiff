# Mitwirken an PTIFF

Danke, dass du zu PTIFF beitragen möchtest! Dieses Projekt lebt von Beiträgen —
sei es Code, Dokumentation, Tests, Bug-Reports oder Design-Feedback. PTIFF ist ein
offener Standard (lizenziert unter Apache-2.0) mit einem transparenten
Governance-Prozess; bitte lies die Dateien `GOVERNANCE.md` und
`CODE_OF_CONDUCT.md`, bevor du loslegst.

## Schnellstart

```bash
# Build & Test (einmalige Einrichtung, siehe libptiff/README.md)
conan profile detect --force
conan install . --output-folder=build --build=missing
cmake -B build -S . -DCMAKE_TOOLCHAIN_FILE=build/conan_toolchain.cmake
cmake --build build
ctest --test-dir build --output-on-failure
```

## Art der Beiträge

| Kategorie | Beschreibung |
|-----------|--------------|
| **Bugs & Verbesserungen** | Einen Bug melden oder einen Fix als Pull Request beisteuern. |
| **TIFF/BigTIFF-Backend** | Lese-/Schreib-Unterstützung für echte TIFF/BigTIFF, Kompression (LZW/PackBits), Tile/Strip-Zugriff. |
| **Weitere Backends** | PDS4, ISIS3 CUB, Zarr, OpenEXR, Memory, Cloud-Object-Storage. |
| **Spezifikation (RFCs)** | Neue oder geänderte RFCs für Extension-Domänen (camera, CRS, SPICE, stereo, …). |
| **Tests & Konformität** | Unit-, Integrations-, Golden- und Konformitätstests. |
| **Dokumentation** | `ARCHITECTURE.md`, Doxygen-Kommentare, Guides, Doku-Fixes. |

## Ablauf bei Code-Änderungen

1. **Issue öffnen** und Vorhaben kurz beschreiben (Bug, Feature, Design-Frage).
2. **Fork + Branch** anlegen (sprechender Name, z. B. `fix/tiff-predictor`).
3. **Code schreiben** nach den Richtlinien unten.
4. **Tests schreiben** — jede Änderung an Produktionscode braucht (mindestens) einen
   Test, der sie absichert.
5. **Build & Tests lokal ausführen**, bis alles grün ist.
6. **Pull Request** erstellen, der auf das zugehörige Issue verweist.

## Coding-Richtlinien (C++ / `libptiff`)

- **Sprache:** C++23; Code folgt `.clang-format` / `.clang-tidy` an der Repo-Wurzel
  (automatisch durchgesetzt in CI). 100-Spalten-Limit, 4-Space-Einrückung.
- **Fehlerbehandlung:** `Result<T> = std::expected<T, Error>` für erwartete
  Fehler (Domain-Fehler); Ausnahmen nur für Vertragsverletzungen via
  `PTIFF_PRECONDITION` (bug, kein Laufzeitpfad). Siehe `docs/CODING_GUIDELINES.md`.
- **Öffentliche API:** Jede exportierte Klasse/Funktion ist `PTIFF_EXPORT`. Kein
  Drittanbieter-Typ darf in einen öffentlichen Header leaken.
- **Doku-Kommentare:** `///`-Kommentare auf jeder öffentlichen Klasse/Methode, die
  das *Warum* erklären, nicht die Signatur wiederholen.
- **Dateistruktur:** Ein Typ (oder kleine eng verwandte Gruppe) pro
  Header/Source-Paar; kleine Dateien bevorzugt.
- **Ownership:** Wachsende Domain-Typen sind move-only PIMPL via
  `std::unique_ptr<Impl>`, ohne `shared_ptr` ohne echten geteilten Besitz.
- **Conventions:** Dokumentation und Commit-Meldungen auf Englisch; Tests mit Catch2 v3.
  Beiträge müssen die `///`-Doku aktualisieren (Doxygen wird aus den Kommentaren
  generiert).

## Spezifikations-Änderungen (RFCs)

PTIFF ist ein Standard; normative Änderungen an der Spezifikation laufen über den
**RFC-Prozess** (siehe `GOVERNANCE.md`). Reine Implementation-Änderungen an
`libptiff` brauchen keinen RFC, solange sie die Standardsignatur nicht verändern.
Neue Extension-Domänen oder Tag-Allokationen erfordern einen eigenen, unabhängig
versionierten RFC.

## Test-Ablauf

| Ebene | Zweck |
|-------|-------|
| `tests/unit/` | Domain-/Wert-Typen und einzelne Komponenten. |
| `tests/integration/` | Zusammenspiel mehrerer Komponenten. |
| `tests/golden/` | Regressionsprüfung gegen bekannte "goldene" Dateien. |
| `tests/conformance/` | Konformität zur PTIFF-Spezifikation. |

Einzelnen Test ausführen:

```bash
ctest --test-dir build -R <regex> --output-on-failure
```

## Kommunikation / Verhaltensregeln

Bitte lies die `CODE_OF_CONDUCT.md`. Grundsatz: respektvoll, konstruktiv,
wissenschaftlich ehrlich. Das Projekt bedient eine Community aus Raumfahrt,
Wissenschaft und Industrie — der Ton ist kooperativ und fachlich.

## Danksagung

Anleitung adaptiert nach dem
[GitHub-Handbuch für Contribution Guidelines](https://docs.github.com/en/communities/setting-up-your-project-for-healthy-contributions/setting-guidelines-for-repository-contributors).
