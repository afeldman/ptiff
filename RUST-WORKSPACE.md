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
│   └── ptiff-core/             der Kern (kein C-ABI; math-Dep multicalc ist Pflicht)
│       └── src/
│           ├── lib.rs          Re-Exports (alle öffentlichen Typen)
│           ├── error.rs        ErrorCode / Error / Result (stabil, additiv)
│           ├── id.rs           Id<Tag> (ImageId/CameraId/.../TileId)
│           ├── pixel_type.rs   PixelType (UInt8..Float64, additiv)
│           ├── geometry/       Spatial-Geometry: Domain-Werte + Frame-Semantik + SE(3)/Screw (GEOMETRY-FOUNDATION.md)
│           │   ├── mod.rs        Re-Exports
│           │   ├── vector3.rs    Vec3 (x/y/z; Storage-only POD, C++ 1:1)
│           │   ├── quaternion.rs Quaternion (w/x/y/z; scalar-first, default identity)
│           │   ├── extrinsics.rs Extrinsics (rotation + translation; canonical Pose-Storage)
│           │   ├── intrinsics.rs Intrinsics (fx/fy/cx/cy; pinhole pixel params)
│           │   ├── frames.rs     Frame / FramePair (from→to; 'static Labels)
│           │   ├── pose.rs       Pose (SE(3)-Wrapper + Frame-Semantik; compose/inverse/relative/act/interpolate)
│           │   ├── screw.rs      Screw / ScrewAxis / ScrewMotion (Axis/Pitch/Motion über Twist)
│           │   ├── spice.rs      SpiceState (SPICE (p,q,v,ω)-Mapping → Pose+Twist/Screw; propagieren via SE(3); adjoint)
│           │   ├── camera.rs     Camera + intrinsics/extrinsics/projection-Matrix (K·[R|t])
│           │   ├── planet.rs     Planet (name/IAU-id/Ellipsoid/reference-frame; Extension-by-instance)
│           │   ├── ellipsoid.rs  Ellipsoid (semi-major/semi-minor, meters)
│           │   ├── lens_model.rs LensModel / LensModelKind (Pinhole/Fisheye/Pushbroom + params)
│           │   ├── projection.rs Projection / ProjectionKind (Equirect./Stereogr./Sinusoidal/Orthogr.)
│           │   ├── coordinate_reference_system.rs CRS (Planet + Frame-Override + Projection; identifier=NotImplemented-Stub)
│           │   └── scene_geometry.rs Geometry / GeometryKind (3D-Produkt; Unspecified + named params)
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
│           │       ├── memory_backend.rs    MemoryBackend (serde_json-Codec; feature `memory-backend`)
│           │       └── tiff/                TIFF/BigTIFF-Format-Schicht (feature `tiff-backend`)
│           │           ├── mod.rs             Re-Exports
│           │           ├── checked_arithmetic.rs  Overflow-geprüfte Arithmetik (RFC-0001 §13)
│           │           ├── endian.rs           Endian (II/MM) + read/write-U16/U32/U64
│           │           ├── header.rs           TiffHeader + read/write-TiffHeader (classic/BigTIFF)
│           │           ├── tag.rs              TagId / FieldType / RawTagEntry + K_PRIVATE_TAG_BASE
│           │           ├── pixel_format.rs     BitsPerSample/SampleFormat ↔ PixelType
│           │           ├── ifd.rs              TiffIfd + read_tiff_ifd (inline/offset-Indirektion)
│           │           ├── ifd_writer.rs       TiffIfdEntryToWrite + tiff_ifd_byte_size/write_tiff_ifd
│           │           ├── directory.rs        TileByteRange/TiffCompression/TiffPredictor/TiffDirectory
│           │           │                       + interpret_tiff_ifd/to_storage_model
│           │           ├── directory_writer.rs plan_tiff_write/plan_tiff_write_multi (TiffWritePlan/TiffFileWritePlan)
│           │           ├── ptiff_metadata.rs   RFC-7002-Payload (encode/decode/records_from_storage_model)
│           │           ├── image_source.rs     TiffImageSource (impl ImageSource; on-demand read + Codecs)
│           │           ├── image_sink.rs       TiffImageSink (impl ImageSink; seek-and-write + Back-Patch)
│           │           ├── tiff_backend.rs     TiffBackend (impl StorageBackend; IFD-Kette, read+write+multi-image)
│           │           └── compression/        packbits.rs / lzw.rs / predictor.rs (dependency-frei);
│           │                                   deflate.rs / jpeg.rs (feature `tiff-codecs`, pure-Rust)
│           └── tile/           Tiling-Grid & Tile-Mapping
│               ├── mod.rs               Tile (id/index/region, zero-copy &[u8])
│               ├── tile_extent.rs       TileExtent  (width/height)
│               ├── tile_index.rs        TileIndex   (column/row/level)
│               ├── tile_layout.rs       TileLayout  (columns/rows/region_for/index_for)
│               └── tile_region.rs       TileRegion  (x/y/extent)
├── crates/ptiff-rust/       idiomatische Rust-API (Paketname `ptiff`) auf ptiff-core
│   └── src/                kein C-ABI; ergonomische High-Level-Einstiege über die Core-Traits
│       ├── lib.rs          Re-Exports (Scene/Image/ImageDescriptor/geometry/...) + prelude
│       └── tiff.rs         Tiff (open/from_bytes wrt Scene; images(); to_bytes/write via TiffBackend)
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
| `ptiff_core::io::backend::TiffBackend` | `ptiff::io::backend::TiffBackend` | impl `StorageBackend`: IFD-Kette (read/write, Multi-Image), Factory-Name "tiff" (feature `tiff-backend`) |
| `ptiff_core::io::backend::tiff::Endian` | `ptiff::io::backend::tiff::Endian` | II/MM Byte-Order (feature `tiff-backend`) |
| `ptiff_core::io::backend::tiff::TiffHeader` | `ptiff::io::backend::tiff::TiffHeader` | Byte-Order + BigTIFF-Flag + first-Ifd-Offset |
| `ptiff_core::io::backend::tiff::TagId` | `ptiff::io::backend::tiff::TagId` | Baseline-TIFF-Tags 256-530 + PTIFF-Extension 65001-65005 |
| `ptiff_core::io::backend::tiff::FieldType` | `ptiff::io::backend::tiff::FieldType` | Byte/Short/Long/Long8 (integer-only Subset) |
| `ptiff_core::io::backend::tiff::RawTagEntry` | `ptiff::io::backend::tiff::RawTagEntry` | unaufgelöster 12/20-Byte-Eintrag |
| `ptiff_core::io::backend::tiff::TiffIfd` | `ptiff::io::backend::tiff::TiffIfd` | aufgelöste `tag → u64[]`-Tabelle + next-Ifd |
| `ptiff_core::io::backend::tiff::read_tiff_ifd` | `ptiff::io::backend::tiff::readTiffIfd` | Inline-/Offset-Auflösung; Overflow-/Bounds-Checks; unbekannte Typen übersprungen |
| `ptiff_core::io::backend::tiff::TiffIfdEntryToWrite` | `ptiff::io::backend::tiff::TiffIfdEntryToWrite` | u32-Werte + FieldType (SHORT/LONG/LONG8) |
| `ptiff_core::io::backend::tiff::tiff_ifd_byte_size` | `ptiff::io::backend::tiff::tiffIfdByteSize` | klassische/BigTIFF-IFD-Größe (inline/out-of-line) ohne Schreiben |
| `ptiff_core::io::backend::tiff::write_tiff_ifd` | `ptiff::io::backend::tiff::writeTiffIfd` | sortiert + serialisiert IFD (count/records/next-Ifd/ool-Values) |
| `ptiff_core::io::backend::tiff::TiffDirectory` | `ptiff::io::backend::tiff::TiffDirectory` | aufgelöste Geometrie + Layout + strip/tile-Byte-Ranges + compression/predictor |
| `ptiff_core::io::backend::tiff::TiffCompression` | `ptiff::io::backend::tiff::TiffCompression` | None/Lzw/PackBits/Deflate/Jpeg (tag 259) |
| `ptiff_core::io::backend::tiff::TiffPredictor` | `ptiff::io::backend::tiff::TiffPredictor` | None/HorizontalDifferencing (tag 317) |
| `ptiff_core::io::backend::tiff::interpret_tiff_ifd` | `ptiff::io::backend::tiff::interpretTiffIfd` | Validierung des Supported-Subsets → TiffDirectory |
| `ptiff_core::io::backend::tiff::to_storage_model` | `ptiff::io::backend::tiff::toStorageModel` | TiffDirectory → StorageModel (pixelType/compression/predictor/...) |
| `ptiff_core::io::backend::tiff::plan_tiff_write` | `ptiff::io::backend::tiff::planTiffWrite` | StorageModel → TiffWritePlan (entries + dataOffset + patch-Offset) |
| `ptiff_core::io::backend::tiff::plan_tiff_write_multi` | `ptiff::io::backend::tiff::planTiffWriteMulti` | Mehr-IFD-Chain; Offsets/Rebasing auf absolute Datei-Offsets |
| `ptiff_core::io::backend::tiff::MetadataRecord` / `encode_metadata_payload` / `decode_metadata_payload` | `ptiff::io::backend::tiff::MetadataRecord`/`encodeMetadataPayload`/`decodeMetadataPayload` | RFC-7002 Private-Tag-Payload (PTIFF-Magic + Version + Records) |
| `ptiff_core::io::backend::tiff::TiffImageSource` | `ptiff::io::backend::tiff::TiffImageSource` | impl ImageSource: on-demand read + PackBits/LZW/Predictor |
| `ptiff_core::io::backend::tiff::TiffImageSink` | `ptiff::io::backend::tiff::TiffImageSink` | impl ImageSink: seek-and-write + compression-Back-Patch |
| `ptiff_core::io::backend::tiff::compression::*` | `ptiff::compression::*` | dependency-freie Codecs: PackBits/LZW/Predictor (encode/decode); hinter `tiff-codecs`: `encode_deflate`/`decode_deflate` · `encode_jpeg`/`decode_jpeg` |
| `ptiff_core::geometry::Vec3` | `ptiff::Vec3` | x/y/z; Storage-only POD (C++ 1:1) |
| `ptiff_core::geometry::Quaternion` | `ptiff::Quaternion` | w/x/y/z; scalar-first, default identity |
| `ptiff_core::geometry::Intrinsics` | `ptiff::Intrinsics` | fx/fy/cx/cy (pinhole pixel params) |
| `ptiff_core::geometry::Extrinsics` | `ptiff::Extrinsics` | rotation + translation; canonical Pose-Storage |
| `ptiff_core::geometry::Frame` | *(Rust-first)* | `'static` Frame-Label; J2000/IAU_MOON/SPACECRAFT/CAMERA |
| `ptiff_core::geometry::FramePair` | *(Rust-first)* | gerichtetes Frame-Paar from→to |
| `ptiff_core::geometry::Pose` | *(Rust-first, auf multicalc)* | SE(3)-Wrapper + Frame-Semantik; compose/inverse/relative/act/interpolate/exp/log/adjoint |
| `ptiff_core::geometry::Screw` | *(Rust-first, auf multicalc)* | Twist + Axis/Pitch/Motion (ScrewAxis/ScrewMotion) |
| `ptiff_core::geometry::SpiceState` | *(Rust-first, auf multicalc)* | SPICE `(p,q,v,ω)` → `Pose`+`Twist`/`Screw`; `SE3::adjoint`-Transform; `propagate`/`propagate_screw` |
| `ptiff_core::geometry::Quaternion::from_euler_angles[_deg]` | *(Rust-first)* | ZYX-intrinsisch (Rz·Ry·Rx); `to_rad`/`quaternion2rad`/`to_deg`/`to_angle_axis` |
| `ptiff_core::geometry::Camera` | `ptiff::Camera` | model+intrinsics+extrinsics+timestamp; intrinsics/extrinsics/projection-Matrix |
| `ptiff_core::geometry::Camera::projectionMatrix` | `ptiff::Camera::projectionMatrix` | `P = K·[R\|t]` (pinhole) — C++ 1:1 |
| `ptiff_core::geometry::Planet` | `ptiff::Planet` | name/IAU-id/Ellipsoid/reference-frame; Extension-by-instance |
| `ptiff_core::geometry::Ellipsoid` | `ptiff::Ellipsoid` | semi-major/semi-minor, meters |
| `ptiff_core::geometry::LensModel` | `ptiff::LensModel` | LensModelKind (Pinhole/Fisheye/Pushbroom) + named params |
| `ptiff_core::geometry::Projection` | `ptiff::Projection` | ProjectionKind (Equirect./Stereogr./Sinusoidal/Orthogr.) + named params |
| `ptiff_core::geometry::CoordinateReferenceSystem` | `ptiff::CoordinateReferenceSystem` | Planet + Frame-Override + Projection; identifier()=NotImplemented-Stub |
| `ptiff_core::geometry::Geometry` | `ptiff::Geometry` | GeometryKind (Unspecified) + sourceImage + named params (3D-Produkt) |

Die `TileLayout`-Abfragen (`columns`, `rows`, `region_for`, `index_for`, `from_descriptor`)
sind semantisch **identisch zum C++-Referenzverhalten** (gleiche Rundung `/ div_ceil`, gleiche
`OutOfRange`-Fehlerfälle, gleiche Pyramid-Downsampling-Logik via `>> level`). Das ist die Basis
für spätere Golden-/Roundtrip-Tests.

## Qualitätsstandards

- `#![forbid(unsafe_code)]` in `ptiff-core` (kein `unsafe` im Kern).
- `#![warn(missing_docs)]` — alle öffentlichen Items dokumentiert.
- CI-Check lokal: `cargo build && cargo test && cargo clippy --all-targets && cargo fmt --check`
  muss grün sein (Stand: **343 `ptiff-core`-Tests** — 317 Unit + 10 Corrupted + 6 Golden + 7
  Property + 3 `tiled_write`-Integration — plus 13 idiomatische `ptiff` + 2 Doc-Tests; workspace inkl.
  idiomatischem `ptiff`-Crate).
- Dependencies bewusst minimal: der Default-Build von `ptiff-core` enthält lediglich die
  dependency-freie Mathematik-Basis `multicalc` (→ `libm`), **keine** Serialisierungs-Bibliothek.
  Externe Libs für Serialisierung sind **feature-gated** (siehe unten).

## Externe Libs

### Mathematik-Kern (always-on, GEOMETRY-FOUNDATION.md)

| Crate | Liefert | Hinweis |
|-------|---------|---------|
| `multicalc` (→ `libm`) | SO(3)/SE(3)-Lie-Gruppen, Quaternionen, Twist/Wrench, Linear-Algebra; `no_std`, dependency-frei | Immer aktiv; liefert den Rechen-Kern für `geometry`. |

### Feature-gated (§4.4/§5 des Plans)

Der Serialisierungs-Teil bleibt feature-gated — der Default-Build bleibt von
Serialisierungs-Crates unabhängig:

| Feature | Crate | Liefert |
|---------|-------|---------|
| `serde` | `serde` (derive) | `Serialize`/`Deserialize` auf den Domain-Typen (PixelType, CompressionKind, ImageDescriptor, Image, Scene, StorageModel, ...) |
| `memory-backend` | `serde_json` (+ `serde`) | `MemoryBackend`-Modell-Codec; in-memory StorageBackend |
| `tiff-backend` | *(keine zusätzlichen Deps)* | TIFF/BigTIFF-Backend: Format-Schicht (`endian`/`header`/`tag`/`pixel_format`/`ifd`), Writer (`ifd_writer`/`directory`/`directory_writer`/`ptiff_metadata`), dependency-freie Codecs (`packbits`/`lzw`/`predictor`) + `TiffImageSource`/`TiffImageSink` |
| `tiff-codecs` | `flate2` (→ miniz_oxide, pure-Rust) · `jpeg-encoder` + `jpeg-decoder` (pure-Rust) | Deflate (RFC 1950 zlib, `compression/deflate.rs`) + JPEG baseline 4:4:4 (`compression/jpeg.rs`); **impliziert `tiff-backend`** |

Daneben ist die **image-rs-`tiff`-Crate** (v0.11, `deflate`/`lzw`/`jpeg`-Features) als
**dev-dependency** eingebunden — ausschließlich als unabhängiges **Interop-Gegenlese-Orakel**
in Tests, nie im Produkt-Build (die reine Produkt-Dependency-Kette bleibt `ptiff-core →
multicalc → libm` im Default bzw. + die Codec-Crates hinter `tiff-codecs`).

`memory-backend` impliziert `serde`; `tiff-backend` ist dependency-frei (arbeitet direkt auf
`BinaryReader`/`BinaryWriter`, inkl. der Codecs PackBits/LZW/Predictor). `cargo tree --edges
normal --no-default-features` zeigt nur `ptiff-core → multicalc → libm`. Der Memory-Pixel-Tier
(`MemoryImageSource`/`MemoryImageSink` über das Modell + Pixel-Layout) ist implementiert; die
TIFF/BigTIFF-Schicht umfasst Format (Header/IFD/Tag-Parser), Writer (IFD/Directory/Metadata),
dependency-freie Codecs und `TiffImageSource`/`TiffImageSink` (E2E-Write→Read-Roundtrips für
Uncompressed/PackBits/LZW+Predictor) sowie `TiffBackend` (StorageBackend: IFD-Kette lesen,
schreiben, Multi-Image). **Geschlossen:** Compile-Time-Policies (Phase B — bewusst **nicht** in
Rust abgebildet, funktional durch den Runtime-Writer abgedeckt). Deflate-/JPEG-Codecs sind
inklusive — hinter dem `tiff-codecs`-Feature (pure-Rust: flate2/miniz_oxide + jpeg-encoder/
jpeg-decoder); ohne dieses Feature bleiben `TiffImageSource`/`TiffImageSink` weiterhin auf die
dependency-freien Codecs (PackBits/LZW/Predictor) beschränkt.

## Nächste Schritte (aus dem Plan §4.2 + GEOMETRY-FOUNDATION.md)

Die Reihenfolge folgt `PTIFF-1.0-RUST-CORE-PLAN.md` und `GEOMETRY-FOUNDATION.md`:
1. ✅ `MemoryBackend`-Pixel-Tier (`MemoryImageSource`/`-Sink`, Multi-Image-Offsets) + `BackendFactory`.
2. ✅ **Geometry Foundation Phase I:** `multicalc`-Kern + Storage-Werte (`Vec3`, `Quaternion`,
   `Extrinsics`, `Intrinsics`) + `Frame`/`FramePair` (GEOMETRY-FOUNDATION.md §8 Phase I).
3. ✅ **Geometry Foundation Phase II:** `Pose` (SE(3)-Wrapper + Frame-Semantik), `Screw`
   (Axis/Pitch/Motion), `Camera`+`Planet`/`CRS`/`Projection`/`LensModel` + edge-case-Tests
   (GEOMETRY-FOUNDATION.md §8 Phase II).
4. ✅ **Geometry Foundation Phase III/IV:** Quaternion-Euler-Helfer (`from_euler_angles[_deg]`,
   `to_rad`/`quaternion2rad`/`to_deg`), Serde-Serialisierungs-MVP + Roundtrip-Tests (Phase III);
   `SpiceState` SPICE-Pose-Mapping (`(p,q,v,ω)` → `Pose`+`Twist`/`Screw`, Adjoint, propagate)
   (Phase IV). **Scene/Camera-CRS-Wiring (Punkt 2, Option A):** `ImageDescriptor`+`Image`
   tragen jetzt `camera: Option<Camera>` und `crs: Option<CoordinateReferenceSystem>`;
   das neue `geometry/marshal`-Modul marshallt die Domain-Klassen in/aus den
   `ptiff.camera.*`/`ptiff.crs.*`-Feldern; `SceneSerializer` emittiert sie pro Bild-Child,
   `SceneDeserializer` liest sie zurück. Das roundtrippt end-to-end durch die privaten
   TIFF-Tags 65002/65003 (`Tiff::to_bytes` → `Tiff::from_bytes`). **RFC-Vorbehalt:** das
   Feld-Schema ist ein Vorschlag (M3 fachliche Ebene, open RFC — für C++ + Rust einheitlich);
   unbekannte Keys bleiben erhalten. Bekannte Einschränkung: `Frame` hält einen `'static`-id,
   daher wird ein unbekannter Frame-Override beim Lesen als abwesend behandelt.
5. 🔄 **TIFF/BigTIFF-Backend:** ✅ Format-Schicht (`endian`/`header`/`tag`/`pixel_format`/`ifd`,
   Overflow-/Bounds-Checks, `write_tiff_header`), ✅ Writer-Schicht (`ifd_writer`,
   `directory`+`interpret_tiff_ifd`, `directory_writer`+`plan_tiff_write[_multi]`,
   `ptiff_metadata`), ✅ dependency-freie Codecs (PackBits/LZW/Predictor), ✅
   `TiffImageSource`/`TiffImageSink` (impl. `ImageSource`/`ImageSink`; E2E-Roundtrips für
   Uncompressed/PackBits/LZW+Predictor/Deflate/Jpeg) +
   ✅ `TiffBackend` (impl. `StorageBackend`: IFD-Kette lesen/schreiben, Multi-Image,
   BackendFactory-Registrierung "tiff"), ✅ Deflate/JPEG-Codecs hinter `tiff-codecs`
   (pure-Rust: flate2/miniz_oxide + jpeg-encoder/jpeg-decoder; zlib RFC 1950, JPEG baseline
   4:4:4; Bomben-/Size-Guards, Dimensionen-/Component-Checks). **Geschlossen:**
   Compile-Time-Policies (Phase B — bewusst nicht in Rust abgebildet; funktional durch den
   Runtime-Writer abgedeckt), Deflate/JPEG-Codecs. **Interop-Gegenlese-Orakel:**
   mehrere Tests lesen unser geschriebenes TIFF/BigTIFF mit der externen
   image-rs-`tiff`-Crate (dev-dependency, `deflate`/`lzw`/`jpeg`-Features) gegen
   und verifizieren die Pixel-Daten unabhängig (Uncompressed Gray/RGB, BigTIFF,
   Deflate, LZW, PackBits, JPEG). Der Abgleich hat einen **LZW-Bitwriter-Bug**
   aufgedeckt: unser Encoder/Decoder waren intern konsistent, aber nicht
   standardkonform (die Breiten-Tabelle wuchs nie, weil `have_previous` nur
   bedingt gesetzt wurde) — der Bitwriter spiegelt jetzt exakt das C++-Oracle
   und die Streams werden von `weezl` gelesen.
6. `ptiff-rust` als idiomatische Rust-API auf `ptiff-core` (Paketname `ptiff`). **Begonnen:**
   Crate `crates/ptiff-rust/` als Workspace-Member angelegt (dependency-light: nur
   `ptiff-core` + `std`; `forbid(unsafe_code)` + `warn(missing_docs)`). Erster Slice:
   `Tiff`-Fassade mit `open`/`from_bytes` (→ `Scene` + `images()`-Iterator) und
   `to_bytes`/`write` (Scene → TIFF/BigTIFF via `TiffBackend` + `SceneSerializer`);
   Roundtrip für Scene-Metadaten getestet (None/LZW symmetrisch; Deflate/Jpeg im
   `SceneDeserializer` des Cores wie im C++-Oracle noch nicht rückschreibbar).

   **Pixel/Tile-Lese-Tier fertig:** `Tiff` hält die Roh-Bytes und decodiert echte
   Pixeldaten zurück: `read_image_pixels(index)` (kontiguierter Raster),
   `read_tile(index, col, row)` und `tile_layout(index)`.
   **Pixel-Schreib-Tier fertig:** Core-ebene tiled write unterstützt jetzt jede
   Kompression (None/PackBits/LZW/Deflate/JPEG) und horizontales Differencing —
   der `TiffImageSink` packt komprimierte Tiles hintereinander und patcht pro Tile
   Offset/ByteCount in die `TileOffsets`/`TileByteCounts`-Arrays zurück
   (`8447530`; Multi-Image akzeptiert weiterhin kein tiled+compressed, da die
   Datenregion dort statisch reserviert ist — als Einzelbild schreiben). Im
   idiomatischen Layer schreibt `Tiff::to_bytes_with_pixels(scene, rasters)`
   die Scene-Metadaten **und** pro Bild dessen Raster (Layout exakt wie
   `read_image_pixels` es liefert: Grid-Tiles aneinandergereiht, Randkacheln
   gepaddet) mit 4 neuen Edge-Fällen getestet (Single-Strip grayscale,
   Multi-Image inkl. 16-bit RGB, getilte 3×3-Kacheln mit Rand-Padding,
   Raster-Anzahl/-Größen-Validierung) — Roundtrip via `from_bytes` +
   `read_image_pixels`. Runtime-abfragbare Versions-Konstanten
   (`ptiff_core::{APP_VERSION, VERSION_STR}`) sind Teil des Cores, damit die CLI
   später `ptiff --version` anbieten kann.
   Camera/Geometry/CRS sind im idiomatischen Layer re-exportiert und tragen über
   `ImageDescriptor.camera`/`crs` typisiert auf jedem `Scene`-Bild (end-to-end durch die
   PTIFF-Tags 65002/65003, siehe Punkt 4).
7. `ptiff-c` (C-ABI) — erst wenn der Kern Funktionalität trägt.
8. Tests / Golden / Property & Fuzz gemäß §11. **Fertig (Kern in Rust als Referenzimpl.):
   - `tests/golden.rs` (§11.3): SHA-256-Goldendigests für die **verlustfreien** Serialsierungen
     (Unkomprimiert, PackBits, LZW) + die PTIFF-Metadaten-Tags 65001–65005 inkl. der
     RFC-provisional `ptiff.camera.*`/`ptiff.crs.*`-Schemas (Punkt 2). Bump-Regel wie im
     C++-Oracle (`kGolden*Sha256`); Regenerierung via `PTIFF_GOLDEN_REGEN=1`. Ein
     Cross-Oracle-Test liest dieselben Bytes mit image-rs `tiff` wieder (Dimensionen + Pixel
     identisch) → Byte-Exakt *und* Kompatibilität.
   - `tests/corrupted.rs` (§11.2 Corrupted-File-Tests): missgebildete TIFFs (bad byte order/
     magic, truncation, ungültige/bzw. zyklische IFD-Offsets, überbreite Tag-Counts,
     out-of-line-Werte > Datei, fehlende Pflicht-Tags) → `InvalidArgument` statt Panic.
   - `tests/property.rs` (§11.4, `proptest` als dev-dep): Tile-Arithmetik-Invarianten
     (`columns`/`rows` = `div_ceil`, `index_for`→`region_for`-Konsistenz, lückenlose Raster,
     keine Überläufe) + LZW/PackBits-Roundtrip-Fixed-Points über generierten Eingaben.
   - Kompression (§11.2): je Codec cross-Roundtrip bereits im Core (LZW/PackBits/Deflate
     byte-exakt; JPEG mit Toleranz, da reines Rust-JPEG nicht byte-identisch zu libjpeg-turbo)
     + image-rs-Oracle-Interop.
   - Fuzzing (Rust-`cargo fuzz`) ist **deferred**: braucht Nightly-Toolchain + eigenes
     Fuzz-Crate; die Corrupted-File-Tests liefern die Malformed-Input-Rückweisungs-Garantie
     bereits im Standard-Toolchain.

> **Wichtig:** Der Kern enthält **keine** `#[no_mangle]`-Funktionen. Die C-ABI ist die
> Plattform-Grenze (Architectural Response §3.1.4) und lebt in `ptiff-c`, nicht hier.
