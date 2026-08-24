# PTIFF Documentation-Kommentar-Stil

Diese Datei beschreibt den einheitlichen Stil für Code-Dokumentation in
PTIFF. Die Referenzimplementierung ist ein **Rust-Workspace**; daher gibt es
zwei Ergänzungssysteme:

- **rustdoc** für den öffentlichen Rust-API-Comentar (primär).
- **Doxygen** für die C-ABI-`ptiff_*`-Funktionen und die Konzept-/Übersichts-
  seiten unter `docs/doxygen/` (auf Basis des cbindgen-generierten
  `target/ptiff_c.h`).

Das Ziel: Jedes öffentliche Symbol hat einen vollständigen, werkzeug-kompatiblen
Doc-Kommentar — lesbar in der IDE, in rustdoc/HTML und im generierten XML.

## rustdoc (Rust-Code)

1. Jeder `pub`-Typ, jedes `pub`-Mitglied und jede `pub`-Funktion wird
   dokumentiert. Crates setzen `#![warn(missing_docs)]` und
   `#![forbid(unsafe_code)]` (Kern) bzw. erlauben `unsafe` nur in `ptiff-c`
   an der FFI-Grenze.
2. Doc-Kommentare erklären das **Warum**, nicht die Signatur.
3. Beispiele mit ```` ```no_run ```` / ```` ``` `.` verwenden `assert_eq!`,
   `expect`-Hinweise und reale Pfade; `no_run` wo ein Beispiel eine Datei
   öffnet, damit `cargo test` es kompiliert, aber nicht ausführt.
4. **Zeilenlänge**: ≤ 100 Spalten (Konsistenz mit `cargo fmt`).
5. Markdown-Links auf andere Symbole via `[`Type`](crate::path::Type)` oder
   backtick-Referenzen; keine absoluten Pfade.

## Doxygen (C-ABI + Konzeptseiten)

- Die handgeschriebenen Übersichtsseiten liegen in `docs/doxygen/*.dox` (Blöcke
  `/** ... */`) und `docs/doxygen/*.md` (Einstiegsseite: `mainpage.md`).
- Der C-ABI-`ptiff_c.h`-Header wird bei `cargo build -p ptiff-c` von **cbindgen**
  aus den `extern "C"`-Signaturen in `crates/ptiff-c` generiert; Kommentare
  dort stammen aus dem Rust-Code und werden in den Header übernommen.
- Konzeptseiten verwenden `@page`, `@section`, `@subpage` und verknüpfen mit
  `@ref`/Backticks auf die C-ABI-Symbole (`ptiff_*`).
- Codesamples in `@code {...}` flach halten (einzelne Statements, `assert`-Zeilen);
  Doxygen 1.17 übersieht sonst gelegentlich das abschließende `@endcode`.

## Qualifikation und Verlinkung

- Rust: `ptiff::Camera` (Voll-Pfad ab Crate-Root), im Fließtext in Backticks.
- C-ABI: `ptiff_image_create` o. ä. mit `@ref`/Backticks; voll qualifizierte
  Namen, wo nötig.

## Was definitiv zu vermeiden ist

- Verweise auf nicht existierende Symbole (erzeugt rustdoc-`broken_intra_doc_links`
  bzw. Doxygen-*"unable to resolve reference"*).
- Behauptungen ohne Beleg: jeden Fehlerpfad mit dem realen `ErrorCode` verknüpfen.
- Doku, die über die Implementierung hinausgeht (kein "Trello"-Modus, keine TODOs).
- Gemischte Stile oder fehlende Doc-Blöcke an öffentlichen Symbolen.

## Quellen / Pflege

- **rustdoc**: `cargo doc --workspace --no-deps --open`; Warnungen zu fehlenden
  Docs werden über `#![warn(missing_docs)]` in CI sichtbar.
- **Doxygen**: `doxygen Doxyfile` (schreibt nach `build/docs`, nutzt zusätzlich
  das generierte `target/ptiff_c.h`). Erwartete, vorhandene Warnungen betreffen
  nur veraltete Doxyfile-Tags und die Markdown-Seiten — nicht den Code-Stil oben.

Erst mal ein Stil-Referenzdokument; kann später durch eine CI-Doku-Warnungsprüfung
(max warnings) ergänzt werden.
