# PTIFF Doxygen Kommentar-Stil (Google-orientiert)

Diese Datei beschreibt den einheitlichen Doxygen-Stil für den C++-Code in **libptiff**
(`libptiff/include/ptiff/**` und `libptiff/src/**`). Das Ziel: Jeder public/exportierte
Symbol hat einen vollständigen, Doxygen-kompatiblen Doc-Kommentar mit Beispiel — lesbar in
der IDE, in Doxygen-HTML und im generierten XML.

Der Stil folgt der **Google-Docstring-Konvention** und dem bestehenden Stil in
`docs/doxygen/*.dox`. Er ist bewusst deklarativ und beispielreich aufgesetzt.

## Grundregeln

1. **Block-Kommentare mit `///`** pro Zeile — nicht `/** ... */` im Header-Code (die dox
   Seiten in `docs/doxygen/` dürfen `/** ... */` nutzen, der C++-Code nutzt `///`).
2. Jeder öffentliche Typ, jedes Mitglied, jede Funktion/Methode und jeder enum-Wert wird
   dokumentiert. Interne/detail-Symbole (im `detail`-Namespace) können kompakter bleiben.
3. **Trennzeile**: Nach dem `///`-Absatz eine Leerzeile aus `///` vor Tags. Tags beginnen
   am absoluten Spaltenanfang nach `/// `.
4. **Zeilenlänge**: möglichst ≤ 100 Spalten (Konsistenz mit dem C++-Code).

## Pflicht-Tags je Funktion

| Nummern | Tags |
|---------|------|
| Kurzbeschreibung | immer eine führende **`@brief`**-Zeile (ein Satz). |
| Parameter | **`@param name`** für jeden Parameter, mit Typ/Kontext. Bei Default-Parametern den Default erwähnen. |
| Rückgabe | **`@return`**: Erfolgsfall klar, dann Fehlerfälle mit dem jeweiligen
  `ptiff::ErrorCode` aufzählen (z. B. `@ref ptiff::ErrorCode::OutOfRange "OutOfRange"`). |
| Template-Parameter | **`@tparam T`** wo generisch. |
| Beispiel(e) | mindestens ein fenced Codeblock:
  `@code{.cpp} ... @endcode` mit `assert(...)`-Zusicherungen statt unbewiesenem Text. |
| Zusätze (optional) | **`@note`**, **`@warning`**, **`@see`** mit `@ref`-Verlinkung auf verwandte Symbole. |

Für Klassen/Strukturen: `@brief` + Absatz + `@section nome_abschnitt Abschnittstitel` für
mehrteilige semantische Blöcke, plus ein Gesamtbeispiel (`@code{.cpp}`).

## `@code`-Beispiel-Konvention

- `@code{.cpp}` für C++-Beispiele, `@verbatim` nur wenn Doxygen-Markup nicht gewünscht ist.
- Jedes Beispiel ist **selbstaussagend und korrekt wirkend**: Es nutzt reale Member und
  echte Fehlercodes, setzt Ergebnisse mit `assert(...)` oder `REQUIRE(...)` und zeigt auch
  **Fehlerpfade** (z. B. `assert(!bad.has_value())`).
- Beispielcode ist **nicht** Teil des Builds, muss aber syntaktisch plausibel sein.
- Keine absoluten Pfade, Projekt-Header per `#include <ptiff/...>`.

## Qualifikation und Verlinkung

- **Verlinken mit `@ref`** und voll qualifiziertem Namen für quer-symbole, plus Anzeigename:
  `@ref ptiff::io::ImageSource "ImageSource"`.
- Verwende **`\c symbol`** (oder Markdown-Codestyle `` `symbol` ``) für Inline-Erwähnungen
  von Member-Namen, Tag-Namen, Enumeratoren.
- Parameter im Fließtext mit **`\p name`** referenzieren.
- **Beispiel für `@param`/`@return`-Durchgängigkeit**: Siehe
  `libptiff/include/ptiff/io/tile/tile_layout.hpp` — das ist der Referenz-/Goldstandard, an
  dem neue/überarbeitete Header gemessen werden.

## Was definitiv zu vermeiden ist

- `@ref`-Ziele, die nicht existieren (erzeugt Doxygen-Warnung *"unable to resolve reference"*).
- Behauptungen ohne Beleg: jeden Fehlerpfad mit dem realen `ErrorCode` verknüpfen.
- Doku, die über die Implementierung hinausgeht (kein "Trello"-Modus, keine TODOs).
- Mixed-Stil: Mehrmalige `///`-Blöcke pro Symbol oder fehlende `@param`-Blöcke.

## Quellen / Pflege

- Die Doxygen-Generierung läuft über `doxygen Doxyfile` (schreibt nach `build/docs`).
  Erwartete, **vorhandene** Warnungen betreffen nur veraltete Doxyfile-Tags und die
  Markdown-Seiten in `docs/doxygen/*.md` — nicht den Code-Stil oben.
- **Bekannte Doxygen-Falle (1.17) mit `@code` in `///`-Doccomments**: Enthält ein `@code`
  Block Konstrukte wie `case X:`-Labels oder sehr komplexe, blockartige `switch`-Körper,
  kann Doxygen das abschließende `@endcode` übersehen und meldet *"reached end of file
  while inside a 'code' block"*. **Regel**: `@code`-Beispiele simpel und flach halten
  (einzelne Anweisungen, `assert`-Zeilen, höchstens flache `if`-Blöcke); mehrere
  aufeinanderfolgende `@code`-Blöcke in *einem* Docblock sind ok.
- Erst mal ein Google-Stil-Referenzdokument; kann später durch eine CI-Doku-Warnungsprüfung
  (max warnings) ergänzt werden.
