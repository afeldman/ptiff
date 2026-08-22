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
│           ├── lib.rs          Re-Exports (alle öffentlichen Typen)
│           ├── error.rs        ErrorCode / Error / Result (stabil, additiv)
│           ├── id.rs           Id<Tag> (ImageId/CameraId/.../TileId)
│           ├── pixel_type.rs   PixelType (UInt8..Float64, additiv)
│           ├── image/          image-Domain-Typen
│           │   ├── mod.rs               Image (width/height/pixelType/...; direct)
│           │   ├── compression_kind.rs  CompressionKind (None/Lzw/Deflate/Jpeg)
│           │   ├── image_descriptor.rs  ImageDescriptor (width/height/pixelType/...)
│           │   └── tile_info.rs         TileInfo (tileWidth/tileHeight)
│           ├── scene.rs        Scene (add_image/image/image_at, monotonic ImageId)
│           ├── io/             Byte-Transport & format-neutral Schicht
│           │   ├── backend_capabilities.rs  BackendCapabilities (4 Flags)
│           │   ├── binary_reader.rs         BinaryReader (read/seek/position/size)
│           │   ├── binary_writer.rs         BinaryWriter (write/seek/position/flush)
│           │   ├── memory_binary_reader.rs  MemoryBinaryReader (Arc<[u8]>)
│           │   ├── memory_binary_writer.rs  MemoryBinaryWriter (Vec<u8>)
│           │   ├── storage_model.rs         StorageModel (BTreeMap-Felder + Child-Tree)
│           │   ├── serializer.rs            Serializer Trait (Scene → StorageModel)
│           │   ├── scene_serializer.rs     SceneSerializer (C++-1:1-Feld-Schema)
│           │   ├── deserializer.rs          Deserializer Trait (StorageModel → Scene)
│           │   ├── scene_deserializer.rs   SceneDeserializer (C++-1:1)
│           │   ├── image_source.rs          ImageSource Trait (layout + read_tile)
│           │   ├── image_sink.rs            ImageSink Trait (layout + write_tile)
│           │   ├── tile_provider.rs         TileProvider (layout + provide_tile)
│           │   ├── storage_backend.rs       StorageBackend Trait (name/capabilities/...)
│           │   ├── storage_model.rs         (s. o.)
│           │   ├── backend_factory.rs       BackendFactory (Registry+Factory; Singleton)
│           │   └── backend/
│           │       ├── mod.rs               Backend-Sammlung (feature-gated)
│           │       ├── memory_layout.rs     MemoryImageInfo + geometry (image_info_from_model)
│           │       ├── memory_image_source.rs MemoryImageSource (read_tile on demand)
│           │       ├── memory_image_sink.rs   MemoryImageSink (write_tile seek-and-write)
│           │       └── memory_backend.rs    MemoryBackend (serde_json-Codec; feature)
│           └── tile/           Tiling-Grid & Tile-Mapping
│               ├── mod.rs               Tile (id/index/region, zero-copy &[u8])
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
| `ptiff_core::Id<Tag>` | `ptiff::Id<Tag>` | strongly-typed opaque handle (u64 + PhantomTag) |
| `ptiff_core::ImageId` ... | `ptiff::ImageId` ... | ImageId/CameraId/LayerId/AnnotationId/GeometryId/TileId |
| `ptiff_core::io::BinaryReader` | `ptiff::io::BinaryReader` | read/seek/position/size (Trait) |
| `ptiff_core::io::BinaryWriter` | `ptiff::io::BinaryWriter` | write/seek/position/flush (Trait) |
| `ptiff_core::io::StorageModel` | `ptiff::io::StorageModel` | BTreeMap-Felder + Child-Tree, ascending order |
| `ptiff_core::io::TileProvider` | `ptiff::io::TileProvider` | layout + provide_tile (Trait) |
| `ptiff_core::Image` | `ptiff::Image` | width/height/pixelType/channelCount/... (no pixels) |
| `ptiff_core::Scene` | `ptiff::Scene` | add_image/image/image_at; monotonic ImageId |
| `ptiff_core::tile::Tile<'a>` | `ptiff::io::tile::Tile` | id/index/region + zero-copy &[u8] data |
| `ptiff_core::tile::TileLayout` | `ptiff::io::tile::TileLayout` | + columns/rows/region_for/index_for/from_descriptor |
| `ptiff_core::io::Serializer` | `ptiff::io::Serializer` | Scene → StorageModel (Trait) |
| `ptiff_core::io::SceneSerializer` | `ptiff::io::SceneSerializer` | C++-1:1-Feld-Schema (imageWidth/.../compression) |
| `ptiff_core::io::Deserializer` | `ptiff::io::Deserializer` | StorageModel → Scene (Trait) |
| `ptiff_core::io::SceneDeserializer` | `ptiff::io::SceneDeserializer` | C++-1:1; read/write-Asymmetrie (None/Lzw) |
| `ptiff_core::io::BackendCapabilities` | `ptiff::io::BackendCapabilities` | 4 Flag-Struct (tiling/streaming/randomAccess/cloud) |
| `ptiff_core::io::ImageSource` | `ptiff::io::ImageSource` | layout + read_tile (Trait) |
| `ptiff_core::io::ImageSink` | `ptiff::io::ImageSink` | layout + write_tile (Trait) |
| `ptiff_core::io::StorageBackend` | `ptiff::io::StorageBackend` | name/capabilities/open/...-Trait + NotImplemented-Defaults |
| `ptiff_core::io::MemoryBinaryReader` | `ptiff::io::MemoryBinaryReader` | über Arc<[u8]>; seek-past-end → InvalidArgument |
| `ptiff_core::io::MemoryBinaryWriter` | `ptiff::io::MemoryBinaryWriter` | über Vec<u8>; buffer()/take_buffer() |
| `ptiff_core::io::BackendFactory` | `ptiff::io::BackendFactory` | Registry + Factory; **einziger Singleton**; thread-safe |
| `ptiff_core::io::backend::MemoryImageInfo` | `ptiff::io::backend::memory::MemoryImageInfo` | layout + samplesPerPixel + pixelType + tileBytes/imagePixelBytes |
| `ptiff_core::io::backend::MemoryImageSource` | `ptiff::io::backend::memory::MemoryImageSource` | read_tile on demand, Reusable-Buffer (C++-1:1) |
| `ptiff_core::io::backend::MemoryImageSink` | `ptiff::io::backend::memory::MemoryImageSink` | write_tile seek-and-write; Offsets via TileLayout |
| `ptiff_core::io::backend::MemoryBackend` | `ptiff::io::backend::MemoryBackend` | serde_json-Modell-Codec + Pixel-Tier (feature `memory-backend`); Rust-first |

Die `TileLayout`-Abfragen (`columns`, `rows`, `region_for`, `index_for`, `from_descriptor`)
sind semantisch **identisch zum C++-Referenzverhalten** (gleiche Rundung `/ div_ceil`, gleiche
`OutOfRange`-Fehlerfälle, gleiche Pyramid-Downsampling-Logik via `>> level`). Das ist die Basis
für spätere Golden-/Roundtrip-Tests.

## Qualitätsstandards

- `#![forbid(unsafe_code)]` in `ptiff-core` (kein `unsafe` im Kern).
- `#![warn(missing_docs)]` — alle öffentlichen Items dokumentiert.
- CI-Check lokal: `cargo build && cargo test && cargo clippy --all-targets && cargo fmt --check`
  muss grün sein (Stand: **99 Tests grün**).
- Dependencies bewusst minimal: der Default-Build von `ptiff-core` ist **dependency-frei**
  (keine externen Pflichtdeps). Externe Libs sind **feature-gated** (siehe unten).

## Externe Libs (feature-gated, §4.4/§5 des Plans)

Der Kern bleibt im Default dependency-frei; wo das Plan-§4/§5 eine Crate vorsieht, ist sie über
ein optionales Cargo-Feature aktivierbar:

| Feature | Crate | Liefert |
|---------|-------|---------|
| `serde` | `serde` (derive) | `Serialize`/`Deserialize` auf den Domain-Typen (PixelType, CompressionKind, ImageDescriptor, Image, Scene, StorageModel, ...) |
| `memory-backend` | `serde_json` (+ `serde`) | `MemoryBackend`-Modell-Codec; in-memory StorageBackend |

`memory-backend` impliziert `serde`. Der Default-Build bleibt dependency-frei (`cargo tree`
`--edges normal --no-default-features` zeigt keine externen Pflichtdeps). Der Memory-Pixel-Tier
(`MemoryImageSource`/`MemoryImageSink` über das Modell + Pixel-Layout) ist implementiert.

## Nächste Schritte (aus dem Plan §4.2)

Die Reihenfolge folgt `PTIFF-1.0-RUST-CORE-PLAN.md`:
1. ✅ `MemoryBackend`-Pixel-Tier (`MemoryImageSource`/`-Sink`, Multi-Image-Offsets) + `BackendFactory`.
2. TIFF/BigTIFF-Backend (Header, IFD, Tag-Parser) — §4.2/Phase.
3. `ptiff-rust` als idiomatische Rust-API auf `ptiff-core` (Paketname `ptiff`).
4. `ptiff-c` (C-ABI) — erst wenn der Kern Funktionalität trägt.
5. Tests / Golden / Property & Fuzz gemäß §11.

> **Wichtig:** Der Kern enthält **keine** `#[no_mangle]`-Funktionen. Die C-ABI ist die
> Plattform-Grenze (Architectural Response §3.1.4) und lebt in `ptiff-c`, nicht hier.
