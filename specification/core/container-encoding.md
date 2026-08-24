# PTIFF Specification — Core Domain

**Status:** Draft
**Version:** 0.1.0
**Normative basis:** `RFC-0001-Core.md`, implementation `crates/ptiff-core/src/io/backend/tiff/`
**Scope:** The PTIFF container as a strict extension of TIFF 6.0 / BigTIFF, the private
extension tags, the versioned extension payload format, and the core image metadata model.

---

## 1. Overview

PTIFF (Planetary TIFF) is an **open, scientific image data standard** for planetary imagery,
defined as a *strict extension* of TIFF 6.0 and BigTIFF. PTIFF does **not** introduce a new
binary container. Instead it defines a structured, namespaced way to attach
planetary-science metadata (camera model, SPICE-derived pointing, coordinate reference
system, photometry, stereo, mesh, and AI-derived products) to TIFF/BigTIFF files so that:

- **generic, unmodified TIFF readers MUST be able to read the baseline image**, and
- **PTIFF-aware software MAY additionally read** the structured planetary metadata from the
  same file.

This document (the Core domain) defines the container-level mechanics: which TIFF tags PTIFF
uses, how their value payloads are encoded, and the required core image metadata. Extension
domains (`camera/`, `spice/`, `crs/`, `photometry/`, `stereo/`, `mesh/`, `ai/`) build on this
foundation in their own documents.

---

## 2. Container model

### 2.1 TIFF 6.0 and BigTIFF

A PTIFF file is a valid TIFF 6.0 file or a valid BigTIFF file:

- The header declares the byte order (`II` little- or `MM` big-endian). The PTIFF extension
  payloads described below are always little-endian regardless of the file's declared byte
  order.
- Baseline configuration MUST use the tags defined in RFC-0001 and the TIFF 6.0 baseline.
- Strip or tiled layout is permitted. Tiled images MUST use each of `tileWidth`/`tileHeight`
  (`TileWidth` 322 / `TileLength` 323) a multiple of 16, per TIFF 6.0.

### 2.2 Strict-extension guarantee

Because PTIFF stores its scientific metadata in **private TIFF tags** (≥ `0x8000`) that
generic readers skip by spec, a PTIFF file remains a normal TIFF file to the surrounding
ecosystem:

> PTIFF-aware software MAY additionally read structured planetary, camera, geometric,
> stereo, photometric, and derived-product metadata from the same file.
> — RFC-0001, §1 Abstract

Core requirement: **the primary (baseline) image MUST be decodable without any PTIFF
extension metadata present.**

---

## 3. Private extension tags

PTIFF reserves five Private Tag IDs (the range ≥ 65001 was chosen to avoid colliding with
common registered extensions such as GeoTIFF's 33550–34735 or GDAL's 42112–42113):

| Tag ID | Name                        | Domain          | Storage notation     |
|--------|-----------------------------|-----------------|----------------------|
| 65001  | `PtiffSpice`                | SPICE geometry  | `ptiff.spice.*`      |
| 65002  | `PtiffCameraGeometry`       | Camera          | `ptiff.camera.*`     |
| 65003  | `PtiffCrs`                  | CRS             | `ptiff.crs.*`        |
| 65004  | `PtiffScientificLayers`     | Scientific layers | `ptiff.layers.*`   |
| 65005  | `PtiffProvenance`           | Provenance      | `ptiff.provenance.*` |

**Field type.** Each of these five tags is a `BYTE`-typed tag (TIFF field type 1). The
tag's value area / offset points to the versioned byte payload described in
[Section 4](#4-extension-payload-format).

**Absence.** A plain TIFF writer emits none of these tags; a model carrying no `ptiff.*`
fields emits no extension tag. Readers MUST tolerate their absence.

**Unknown tags.** Per TIFF 6.0, readers MUST skip tags they do not recognize. This is what
guarantees interoperabiltiy of PTIFF files in the wider ecosystem and forward-compatibility
of PTIFF itself.

**Multiple IFDs / COG pyramid levels.** When a PTIFF file carries more than one IFD (e.g. a
Cloud-Optimized-GeoTIFF pyramid), each IFD that carries an image MAY have its own set of
extension tags; the model layer represents them as distinct image nodes.

---

## 4. Extension payload format

Each of the five extension tags carries a **small, self-describing, versioned map** of UTF-8
string key/value pairs. All integers in the payload are **little-endian**.

### 4.1 Byte layout

```
 0                 magic   "PTIFF"                         5 bytes (ASCII)
 5                 ver     payload format version, u16     currently 1
 7                 nrec    number of (key, value) records  u32
 11  …  per record, repeated nrec times:
                       klen   key byte length              u16
                       key    UTF-8 key bytes              klen bytes
                       vlen   value byte length            u32
                       value  UTF-8 value bytes            vlen bytes
```

`kPtiffMetadataVersion = 1`.

### 4.2 Canonical ordering

Records are sorted by key in ascending byte-wise order so the encoding is
**deterministic**: the same logical metadata always yields the same bytes. This is what a
golden digest (content hash) of the extension payload depends on.

### 4.3 Record keys

Keys select well-known per-domain field names (defined by each extension-domain
specification; see the per-domain documents and the field register in `appendices/field-register.md`).
The reader stores them prefixed as `ptiff.<domain>.<key>`.

- Unknown keys MUST be preserved verbatim by a reader so that a future writer's additional
  fields survive a read-modify-write round trip.
- A reader that encounters a payload `ver` greater than it knows MUST NOT fail the whole
  file: it MAY skip that tag (treating the domain as absent) exactly as a non-PTIFF reader
  already ignores private tags.

### 4.4 Decoding errors

A payload is invalid when it is empty, lacks the `PTIFF` magic, has a `ver` greater than the
reader's `kPtiffMetadataVersion`, or is malformed/truncated. A compliant reader MUST reject
an invalid payload for that tag rather than mis-parsing it.

---

## 5. Core image metadata model

The baseline image metadata is represented on the model's image node as scalar decimal/string
fields. PTIFF defines the following core fields; all values are stored as ASCII/UTF-8 strings.

| Field             | Type/Enum                       | Required | Notes                                  |
|-------------------|---------------------------------|----------|----------------------------------------|
| `imageWidth`      | u32 (decimal)                   | yes      | Pixel columns                          |
| `imageHeight`     | u32 (decimal)                   | yes      | Pixel rows                             |
| `samplesPerPixel` | u32 (decimal) ≥ 1               | yes      | Band/channel count                     |
| `pixelType`       | enum                            | yes      | `UInt8`, `UInt16`, `UInt32`, `Float32`, `Float64` |
| `compression`     | enum                            | yes      | `None`, `Deflate`, `JPEG`, … (see register) |
| `predictor`       | enum                            | yes      | `None`, `Horizontal` (1), `FloatingPoint` (2) |
| `tileWidth`       | u32 (decimal), multiple of 16   | no       | Tiled layout (paired with tileHeight)  |
| `tileHeight`      | u32 (decimal), multiple of 16   | no       | Tiled layout (paired with tileWidth)   |
| `container`       | enum                            | no       | `Classic` (default) or `BigTiff`       |
| `jpegQuality`     | integer ∈ [1, 100]              | no       | Only when `compression = JPEG`         |

**Pairing rule.** `tileWidth` and `tileHeight` MUST both be set or both be absent. A file in
strip layout has neither.

**Baseline decode rule.** A compliant PTIFF reader MUST be able to decode the baseline image
from the TIFF tags `ImageWidth`, `ImageLength`, `BitsPerSample`, `SamplesPerPixel`,
`Compression`, `PhotometricInterpretation`, (`StripOffsets`/`RowsPerStrip`/`StripByteCounts`
or `TileWidth`/`TileLength`/`TileOffsets`/`TileByteCounts`), `PlanarConfiguration`, and
`SampleFormat`. The `StorageModel` fields above are the normalized, format-neutral
representation of those tags.

---

## 6. Conformance

- **PTIFF writer:** MUST emit a valid TIFF 6.0 / BigTIFF file per §2; MUST serialize any
  `ptiff.*` fields into the matching private tag with the payload format of §4; MUST sort
  records canonically per §4.2.
- **PTIFF reader:** MUST decode the baseline image from the TIFF tags alone (§5); MUST skip
  or correctly decode the private extension tags; MUST tolerate absent extension tags and
  unknown payload versions without failing the file.

---

## 7. References

- `RFC-0001-Core.md` — PTIFF Core (motivation, goals, design principles).
- `RFC-0002-GeoTIFF.md` — PTIFF's relationship to GeoTIFF.
- `crates/ptiff-core/src/io/backend/tiff/tag.rs` — tag IDs.
- `crates/ptiff-core/src/io/backend/tiff/ptiff_metadata.rs` — payload format definition.
