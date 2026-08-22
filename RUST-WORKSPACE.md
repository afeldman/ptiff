# PTIFF 1.0 Rust Workspace

Dieses Dokument beschreibt den **sauberen Start** des PTIFF-1.0-Rust-Kerns auf dem
Branch `rust`. Es ergänzt den Plan in [`PTIFF-1.0-RUST-CORE-PLAN.md`](PTIFF-1.0-RUST-CORE-PLAN.md)
(§4.1) um den konkreten, aktuell gebauten Zustand.

## Entscheidung

- **Green-Field 1.0:** Branch `rust`, sauberer Neustart. Die 1.0 definiert den neuen,
  verbindlichen Kontrakt (Breaking Change). Es gibt **keine** Rückwärtskompatibilitäts-Last zur
  bestehenden C++-`libptiff`.
- **Rust-First:** `ptiff-core` ist die Referenz- und einzige maintained Core-Implementierung.
  Alle anderen Sprachen (C++, Python, Octave, Go, Ruby) werden zu Consumern über die C-ABI
  (kommt in einer späteren Phase, in `ptiff-c`).

## Struktur

```
ptiff/                          (Workspace-Root; Cargo.toml [workspace])
├── Cargo.toml                  workspace root (members: crates/ptiff-core + excludes)
├── rust-toolchain.toml         Pin auf stable (reproduzierbare Builds)
├── .cargo/config.toml          konservative Dev-Defaults (debug = line-tables-only)
├── crates/
│   └── ptiff-core/             der Kern (kein C-ABI, keine externen Pflicht-Deps)
│       └── src/
│           ├── lib.rs          Re-Exports
│           ├── error.rs        ErrorCode / Error / Result (stabil, additiv)
│           ├── pixel_type.rs   PixelType (UInt8..Float64, additiv)
│           ├── image/          image-Domain-Typen
│           │   ├── compression_kind.rs  CompressionKind (None/Lzw/Deflate/Jpeg)
│           │   ├── image_descriptor.rs  ImageDescriptor (width/height/pixelType/...)
│           │   └── tile_info.rs         TileInfo (tileWidth/tileHeight)
│           └── tile/           Tiling-Grid & Tile-Mapping
│               ├── tile_extent.rs       TileExtent  (width/height)
│               ├── tile_index.rs        TileIndex   (column/row/level)
│               ├── tile_layout.rs       TileLayout  (columns/rows/region_for/index_for)
│               └── tile_region.rs       TileRegion  (x/y/extent)
├── bindings/
│   └── c/                      bestehende hand-gepflegte C-Header (verbleiben als SOT)
└── PTIFF-1.0-RUST-CORE-PLAN.md architektonischer Plan (Referenz)
```

## Grundtypen (übersetzt aus C++)

| Rust | C++ | Hinweis |
|------|-----|---------|
| `ptiff_core::ErrorCode` | `ptiff::ErrorCode` | stabil, **additiv**; `#[non_exhaustive]` |
| `ptiff_core::Error` | `ptiff::Error` | code + message; implementiert `std::error::Error` |
| `ptiff_core::Result<T>` | `ptiff::Result<T>` = `std::expected<T, Error>` | Standard-Alias |
| `ptiff_core::PixelType` | `ptiff::PixelType` | UInt8, UInt16, UInt32, Float32, Float64 |
| `ptiff_core::CompressionKind` | `ptiff::CompressionKind` | None, Lzw, Deflate, Jpeg |
| `ptiff_core::ImageDescriptor` | `ptiff::ImageDescriptor` | width/height/pixelType/channelCount/... |
| `ptiff_core::TileInfo` | `ptiff::TileInfo` | tileWidth/tileHeight |
| `ptiff_core::tile::TileExtent` | `ptiff::io::tile::TileExtent` | width/height |
| `ptiff_core::tile::TileIndex` | `ptiff::io::tile::TileIndex` | column/row/level |
| `ptiff_core::tile::TileRegion` | `ptiff::io::tile::TileRegion` | x/y/extent |
| `ptiff_core::tile::TileLayout` | `ptiff::io::tile::TileLayout` | + columns/rows/region_for/index_for/from_descriptor |

Die `TileLayout`-Abfragen (`columns`, `rows`, `region_for`, `index_for`, `from_descriptor`)
sind semantisch **identisch zum C++-Referenzverhalten** (gleiche Rundung `/ div_ceil`, gleiche
`OutOfRange`-Fehlerfälle, gleiche Pyramid-Downsampling-Logik via `>> level`). Das ist die Basis
für spätere Golden-/Roundtrip-Tests.

## Qualitätsstandards

- `#![forbid(unsafe_code)]` in `ptiff-core` (kein `unsafe` im Kern).
- `#![warn(missing_docs)]` — alle öffentlichen Items dokumentiert.
- CI-Check lokal: `cargo build && cargo test && cargo clippy --all-targets && cargo fmt --check`
  muss grün sein (Stand: **24 Tests grün**).
- Dependencies bewusst minimal (derzeit **keine** externen Pflichtdeps im Kern).

## Nächste Schritte (aus dem Plan §4.2)

Die Reihenfolge folgt `PTIFF-1.0-RUST-CORE-PLAN.md`:
1. Weitere stabile Grundtypen ergänzen (`Tile`, `TileProvider`, `StorageModel`, ...).
2. `ptiff-rust` als idiomatische Rust-API auf `ptiff-core` (Paketname `ptiff`).
3. `ptiff-c` (C-ABI) — erst wenn der Kern Funktionalität trägt.
4. Tests / Golden / Property & Fuzz gemäß §11.

> **Wichtig:** Der Kern enthält **keine** `#[no_mangle]`-Funktionen. Die C-ABI ist die
> Plattform-Grenze (Architectural Response §3.1.4) und lebt in `ptiff-c`, nicht hier.
