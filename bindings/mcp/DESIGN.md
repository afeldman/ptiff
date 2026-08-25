# PTIFF MCP Server — Design

**Status:** Draft / experimental (0.0.1)
**Scope:** bindings/mcp (application layer, not a libptiff subsystem)

## Ziel

Ein [MCP](https://modelcontextprotocol.io)-Server, der einem LLM (z. B. Claude
Code) Zugriff auf [PTIFF](https://github.com/) / `libptiff` ermöglicht: ein
LLM soll eingebettete PTIFF-/Planetenbild-Metadaten lesen, Tiles/Pixel abfragen
und kleine Dokumente schreiben können, ohne die C-API oder das Containerformat
direkt zu fassen.

## Architektur-Entscheidung: dünne Anwendungsebene

Der MCP-Server ist **kein Kernteil von `libptiff`**. Er ist eine zusätzliche
Anwendungs-Ebene, die über die Python-Bindung auf die Bibliothek zugreift —
konkret über die PyO3-Bindung **`ptiff_pyo3`** (`crates/ptiff-python`), die den
idiomatischen Rust-Core direkt spricht (kein C-ABI-Umweg). Der Server lebt in
`bindings/mcp/`, analog zu den bestehenden Sprachanbindungen:

- Keine Änderung an `crates/ptiff-c`, `bindings/swig` oder `crates/ptiff-core`
  selbst.
- Neuer fachlicher Bedarf (z. B. ein zusätzliches Feld) wird zuerst als Feature
  der PyO3-Bindung (Rust-Core) umgesetzt, danach als MCP-Tool.

```
    crates/ptiff-rust + ptiff-core (idiomatischer Rust-Core)
         │
 crates/ptiff-python (PyO3)  ←  `ptiff_pyo3`-Modul (maturin)
         │
             bindings/mcp (dieser Server)
         │
                     MCP-Client / LLM
```

## Laufzeit & Konfiguration

- CPython ≥ 3.9 (getestet mit 3.14).
- Abhängigkeiten: das offizielle `mcp`-Paket (≥ 2.0) + `numpy` (für
  Pixel-Sampling) + das lokal gebaute `ptiff_pyo3` (nicht auf PyPI; via
  `maturin develop` in `crates/ptiff-python` installiert).
- Transport: **stdio** (der einfachste Weg für lokale MCP-Clients). Start:

  ```bash
  cd bindings/mcp
  .venv/bin/python -m ptiff_mcp.server
  ```

  Alternativ als konfigurierter MCP-Server in `~/.config/.../mcp.json` / Claude
  Code `mcpServers` mit `command` = obiges Kommando.

Wichtig: Die PyO3-Bindung wirft **Python-Exceptions** statt C-ABI-Fehlercodes;
der Server mappt sie auf MCP-Fehlermeldungen (es gibt keine `PTIFF_ERROR_*`-
Codes mehr auf dieser Ebene).

## Tool-Oberfläche

Bewusst eine **fokussierte, kleine** Menge — kein "alles" über die ABI, sondern
das, was ein LLM im Kontext sinnvoll nutzen kann. Alle Pixel-Antworten sind auf
kleine Stichproben/ROI begrenzt (LLM-Kontextgrenzen, siehe "Grenzen").

| Tool | Zweck | Wichtige Parameter |
|------|-------|--------------------|
| `get_version` | Laufzeit- & Compilezeit-Version von libptiff | — |
| `list_backends` | Registrierte Backend-Namen | — |
| `read_metadata` | Metadaten + PTIFF-Extension-Fields (`ptiff.*`) + structured Camera (K/[R|t]/P, Modell, Timestamp) | `path` |
| `read_scene` | Dasselbe als „Scene"-Blick (aktuell = Primary Image), JSON | `path` |
| `read_tile` | Ein Tile/Strip lesen, inkl. Eckstatistik (min/max/mean, Histogramm-Brackets) | `path`, `column`, `row`, `max_samples` |
| `read_pixel_sample` | Kleine ROI (stats) aus einer Tile-Region | `path`, `x`, `y`, `radius` |
| `create_image` | Neue leere TIFF/BigTIFF-Datei anlegen (tiled), optional mit `camera` | `path`, `width`, `height`, `pixel_type`, `channel_count`, `tile_width`, `tile_height`, `compression`, `camera` |
| `write_tile` | Ein einzelnes Tile mit Rohdaten beschreiben | `path` (open sink), `column`, `row`, `data` (Hex/Base64) |

`read_scene` ist mit `read_metadata` redundant; er bleibt als dünner Alias für
die semantische Brücke zum `Scene`-Konzept von PTIFF erhalten, bis
Multi-Image/`Scene`-Zugriffe über die C-ABI ausgeprägt sind.

## Grenzen (bewusst)

- **Kontextgröße** : Der Server gibt **nie** volle Raster-Buffer zurück. Voller
  Pixelfluss ist nur über `read_tile`/`read_pixel_sample` mit festen
  Ober-/Untergrenzen möglich, die minimale Aussagekraft mit minimalem Kontext
  kombinieren (z. B. max. 64 Statistikwerte pro Antwort).
- **Schreiben** ist tiled und deterministisch (`write_tile` je Tile), kein
  Streaming großer Arrays.
- **Fehler** : Rückgabe als MCP `CallToolResult` mit `isError=True` und
  lesbarer Meldung; die PyO3-Bindung wirft Python-Exceptions, die der Server
  abfängt und als MCP-Fehler weitergibt (keine rohen Fehlercodes).
- **Kein volles Raster-Dekodieren** im Servercontext über einzelne Tiles hinaus.
- **Bindungs-Reifegrad** : Die PyO3-Bindung `ptiff_pyo3` ist die offizielle
  Python-Bindung; der Server bleibt als **experimentell/Preview** markiert.

## Tests

- `test/test_server.py` führt die Tool-Handler **in-process** (ohne echten
  MCP-Transport) plus einen echten stdio-MCP-Roundtrip aus. Eine Referenz-TIFF
  wird on-demand mit `ptiff_pyo3` erzeugt (Fixture `sample_tiff`, keine fremden
  Dateien):
  - `read_metadata` liefert korrekte Dimensionen,
  - `read_tile`/`read_pixel_sample` liefern Plausibilitäts-Statistiken,
  - `get_version` / `list_backends` geben sinnvolle Werte,
  - Schreib-Tools `create_image` + `write_tile` erzeugen eine lesbare Datei.

## Roadmap (mögliche nächste Schritte, NICHT Teil dieses Dokuments)

- Multi-Image/`Scene`-Zugriff über die C-ABI (wenn vorhanden).
- `read_cog_tile` für Cloud-Optimized TIFFs (via `HttpRangeBinaryReader`).
- Structured-Content (Bilder-Höflichkeit) für visuelle Stichproben statt nur Stats.
- SSE/HTTP-Transport zusätzlich zu stdio.
