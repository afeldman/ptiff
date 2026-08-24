# PTIFF MCP Server (experimental)

Ein [MCP](https://modelcontextprotocol.io)-Server, der einem LLM Zugriff auf
[PTIFF](https://github.com/) / `libptiff` gibt: ein LLM kann die eingebetteten
Metadaten von Planeten-/TIFF-Dokumenten lesen, Tile-Pixel-Statistiken abfragen
und kleine tiled Dokumente schreiben — ohne die C++-API oder das
Containerformat direkt zu fassen.

> **Status: experimental (0.0.1).** Der Server ist eine dünne Anwendungsebene
> über die SWIG-Python-Bindung (`bindings/python`), die selbst noch
> "under-construction" ist. Siehe `DESIGN.md` für Architektur & bewusste
> Grenzen (u. a. keine vollen Raster-Buffer in LLM-Antworten).

## Prinzip

```
    crates/ptiff-rust + ptiff-core (idiomatischer Rust-Core)
         │
 crates/ptiff-c → libptiff_c  (Rust-extern-"C"-C-ABI, cargo build -p ptiff-c)
         │
 bindings/python (SWIG, promoted)   ←  `ptiff`-Modul
         │
 bindings/mcp (dieser Server)        ←  MCP-Tools
         │
        LLM / MCP-Client (z. B. Claude Code)
```

Der Server ändert **nichts** an `libptiff`, `crates/ptiff-c` oder
`bindings/swig` — nur eine zusätzliche Anwendungs-Ebene.

## Voraussetzungen

- Gebaute `libptiff_c` (+ Rust-Core), z. B.:
  ```bash
  cd <repo-root>
  cargo build -p ptiff-c --release
  ```
  erzeugt `target/release/libptiff_c.*` und den Header `target/ptiff_c.h`.
- `uv` (für das venv) und Python 3.13.

## Setup

```bash
cd bindings/mcp
uv venv --python 3.13 .venv
uv pip install --python .venv/bin/python -e .
uv pip install --python .venv/bin/python "mcp[cli]" "anyio[trio]" pytest
```

> Der Server lädt das `ptiff`-Modul aus `bindings/python/src` (SWIG-Ausgabe).
> Wir installieren bewusst **nicht** die dist—der Server läuft mit dem
> Repo-`ptiff` via `PYTHONPATH` (siehe unten) bzw. die Tests.

## Start (stdio)

```bash
cd bindings/mcp
PYTHONPATH=src:../python/src \
.venv/bin/python -m ptiff_mcp.server
```

Die Python-Bindung findet `libptiff_c` standardmäßig im Rust-Build-Output
(`target/release`) — oder explizit:

```bash
cd bindings/mcp
PTIFF_C_LIB_DIR=/pfad/zu/<repo>/target/release \
PTIFF_LIB_DIR=/pfad/zu/<repo>/target/release \
PYTHONPATH=src:../python/src \
.venv/bin/python -m ptiff_mcp.server
```

Für einen MCP-Client (z. B. Claude Code `mcpServers`) den Server als
`command` registrieren:

```jsonc
{
  "mcpServers": {
    "ptiff": {
      "command": "/pfad/zu/bindings/mcp/.venv/bin/python",
      "args": ["-m", "ptiff_mcp.server"],
      "env": {
        "PYTHONPATH": "/pfad/zu/bindings/mcp/src:/pfad/zu/bindings/python/src",
        "PTIFF_C_LIB_DIR": "/pfad/zu/<repo>/target/release",
        "PTIFF_LIB_DIR": "/pfad/zu/<repo>/target/release"
      }
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
(Server als Unterprozess) ab — gegen die Beispiel-Datei der Python-Bindings
(`bindings/python/test/roundtrip_python.tif`).

## Hinweise

- Der Server gibt **nie** volle Raster-Buffer zurück; Tile-Statistiken sind auf
  `MAX_SAMPLE_BUCKETS = 64` begrenzt, um den LLM-Kontext klein zu halten.
- Low-level-Schreib-Tools halten offene Sinks in einem In-Prozess-Registry
  (`runtime._open_sinks`), die über stdio-Aufrufe hinweg im selben Prozess
  überleben — ein langer Server-Prozess verwaltet sie.
