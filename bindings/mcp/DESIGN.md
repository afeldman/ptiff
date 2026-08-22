# PTIFF MCP Server — Design

**Status:** Draft / experimental (0.0.1)
**Scope:** bindings/mcp (application layer, not a libptiff subsystem)

## Ziel

Ein [MCP](https://modelcontextprotocol.io)-Server, der einem LLM (z. B. Claude
Code) Zugriff auf [PTIFF](https://github.com/) / `libptiff` ermöglicht: ein
LLM soll eingebettete PTIFF-/Planetenbild-Metadaten lesen, Tiles/Pixel abfragen
und kleine Dokumente schreiben können, ohne die C++-API oder das Containerformat
direkt zu fassen.

## Architektur-Entscheidung: dünne Anwendungsebene

Der MCP-Server ist **kein Kernteil von `libptiff`**. Er ist eine zusätzliche
Anwendungs-Ebene, die ausschließlich über die vorhandene, sprachneutrale
C-ABI-Veneer `libptiff_c` auf die Bibliothek zugreift — konkret über die
SWIG-Python-Bindings (`bindings/python`). So gilt:

- Keine Änderung an `bindings/c`, `bindings/swig` oder `libptiff` selbst.
- Der Server lebt in `bindings/mcp/`, analog zu den bestehenden Sprachanbindungen.
- Neuer fachlicher Bedarf (z. B. ein zusätzliches Feld) wird zuerst als Feature
  der C-ABI + Python-Bindung umgesetzt, danach als MCP-Tool.

```
                    libptiff (C++23)
                         │
                 bindings/c → libptiff_c  (extern "C" Veneer)
                         │
       bindings/python (SWIG, promoted)  ← ptiff module
                         │
             bindings/mcp (dieser Server)
                         │
                     MCP-Client / LLM
```

## Laufzeit & Konfiguration

- Python 3.13 (projektpins: `uv`, siehe `bindings/python/pyvenv.cfg`).
- Abhängigkeiten: das offizielle `mcp`-Paket (≥ 2.0) + `numpy` (für
  Pixel-Sampling). Die Bindbar `ptiff` wird aus `bindings/python/src` geladen.
- Wie die übrigen Bindings findet der Server `libptiff_c` über die
  Umgebungsvariablen `PTIFF_C_LIB_DIR` / `PTIFF_LIB_DIR` oder pkg-config
  (siehe `bindings/python/README.md` → "Prerequisites").
- Transport: **stdio** (der einfachste Weg für lokale MCP-Clients). Start:

  ```bash
  cd bindings/mcp
  PTIFF_C_LIB_DIR=/pfad/.../bindings/c PTIFF_LIB_DIR=/pfad/.../libptiff \
  .venv/bin/python -m ptiff_mcp.server
  ```

  Alternativ als konfigurierter MCP-Server in `~/.config/.../mcp.json` / Claude
  Code `mcpServers` mit `command` = obiges Kommando.

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
  lesbarer Meldung; C-ABI-Fehlercodes werden auf menschenlesbare Texte gemappt.
- **Kein volles Raster-Dekodieren** im Servercontext über einzelne Tiles hinaus.
- **Bindungs-Reifegrad** : Die Python-Bindung ist SWIG-generierte
  "under-construction"-Ausgabe; der Server wird als **experimentell/Preview**
  markiert, bis die Bindung priorisiert wird.

## Tests

- `test/test_server.py` führt die Tool-Handler **in-process** (ohne echten
  MCP-Transport) gegen die vorhandene Beispiel-Datei
  `bindings/python/test/roundtrip_python.tif` aus:
  - `read_metadata` liefert korrekte Dimensionen,
  - `read_tile`/`read_pixel_sample` liefern Plausibilitäts-Statistiken,
  - `get_version` / `list_backends` geben sinnvolle Werte,
  - Schreib-Tools `create_image` + `write_tile` erzeugen eine lesbare Datei.

## Roadmap (mögliche nächste Schritte, NICHT Teil dieses Dokuments)

- Multi-Image/`Scene`-Zugriff über die C-ABI (wenn vorhanden).
- `read_cog_tile` für Cloud-Optimized TIFFs (via `HttpRangeBinaryReader`).
- Structured-Content (Bilder-Höflichkeit) für visuelle Stichproben statt nur Stats.
- SSE/HTTP-Transport zusätzlich zu stdio.
