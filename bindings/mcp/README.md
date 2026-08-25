# PTIFF MCP Server (experimental)

Ein [MCP](https://modelcontextprotocol.io)-Server, der einem LLM Zugriff auf
[PTIFF](https://github.com/) / `libptiff` gibt: ein LLM kann die eingebetteten
Metadaten von Planeten-/TIFF-Dokumenten lesen, Tile-Pixel-Statistiken abfragen
und kleine tiled Dokumente schreiben — ohne die C-API oder das
Containerformat direkt zu fassen.

> **Status: experimental (0.0.1).** Der Server ist eine dünne Anwendungsebene
> über die PyO3-Python-Bindung `ptiff_pyo3` (`crates/ptiff-python`), die direkt
> den Rust-Core spricht. Siehe `DESIGN.md` für Architektur & bewusste Grenzen
> (u. a. keine vollen Raster-Buffer in LLM-Antworten).

## Prinzip

```
    crates/ptiff-rust + ptiff-core (idiomatischer Rust-Core)
         │
 crates/ptiff-c → libptiff_c  (Rust-extern-"C"-C-ABI, cargo build -p ptiff-c)
         │
 crates/ptiff-python (PyO3)  ←  `ptiff_pyo3`-Modul (maturin)
         │
 bindings/mcp (dieser Server)        ←  MCP-Tools
         │
        LLM / MCP-Client (z. B. Claude Code)
```

Der Server ändert **nichts** an `libptiff`, `crates/ptiff-c` oder
`bindings/swig` — nur eine zusätzliche Anwendungs-Ebene über die Python-Bindung.

## Voraussetzungen

- Der Rust-Core + C-ABI sind gebaut (`cargo build -p ptiff-c --release` aus dem
  Repo-Root → `target/release/libptiff_c.*` + `target/ptiff_c.h`).
- CPython ≥ 3.9 und `maturin` (zum Bauen von `ptiff_pyo3`).

## Setup

Die PyO3-Bindung `ptiff_pyo3` ist **nicht** auf PyPI — sie wird lokal mit
`maturin` in `crates/ptiff-python` gebaut und in die venv installiert:

```bash
cd bindings/mcp
python3 -m venv .venv
.venv/bin/python -m pip install -e .          # ptiff-mcp (mcp, numpy, anyio, pytest)
.venv/bin/python -m pip install maturin
cd ../../crates/ptiff-python
../../bindings/mcp/.venv/bin/maturin develop  # installiert ptiff_pyo3 in die MCP-venv
```

Danach ist das `ptiff_pyo3`-Modul aus der MCP-venv importierbar.

## Start (stdio)

```bash
cd bindings/mcp
.venv/bin/python -m ptiff_mcp.server
```

Für einen MCP-Client (z. B. Claude Code `mcpServers`) den Server als
`command` registrieren:

```jsonc
{
  "mcpServers": {
    "ptiff": {
      "command": "/pfad/zu/bindings/mcp/.venv/bin/python",
      "args": ["-m", "ptiff_mcp.server"]
    }
  }
}
```

## Tools

| Tool | Zweck |
|------|-------|
| `get_version` | Laufzeit- & Compilezeit-Version von libptiff |
| `list_backends` | Registrierte Backend-Namen |
| `read_metadata` | Primärbild-Metadaten + PTIFF-Extension-Fields (`ptiff.*`) + structured Camera (K/[R|t]/P, Modell, Timestamp) |
| `read_scene` | `read_metadata` als „Scene"-Blick (JSON) |
| `read_tile` | Ein Tile/Strip lesen → kompakte Statistik (min/max/mean, Histogramm) |
| `read_pixel_sample` | Statistik für das Tile, das Pixel (x, y) enthält |
| `write_image_file` | Neues tiled Bild anlegen, optional konstant befüllen; optional `camera` persistieren (stateless) |
| `create_image` | Tiled Bild für sequentielle Tile-Writes öffnen (low-level, liefert `sink_key`); optional `camera` |
| `write_tile` | Ein Tile mit `data_hex` oder `fill` beschreiben |
| `close_image` | Offenen Sink flushen/schließen (macht die Datei lesbar) |

Details, Parameter und bewusste Grenzen: `DESIGN.md`.

## Test

```bash
cd bindings/mcp
.venv/bin/python -m pytest test -q
```

Deckt sowohl die In-Process-Dispatch als auch einen echten stdio-MCP-Roundtrip
(Server als Unterprozess) ab. Eine Referenz-TIFF wird bei Bedarf on-demand mit
`ptiff_pyo3` erzeugt (Fixture `sample_tiff`), sodass keine fremden Dateien
nötig sind.

## Hinweise

- Der Server gibt **nie** volle Raster-Buffer zurück; Tile-Statistiken sind auf
  `MAX_SAMPLE_BUCKETS = 64` begrenzt, um den LLM-Kontext klein zu halten.
- Low-level-Schreib-Tools halten offene Sinks in einem In-Prozess-Registry
  (`runtime._open_sinks`), die über stdio-Aufrufe hinweg im selben Prozess
  überleben — ein langer Server-Prozess verwaltet sie.
