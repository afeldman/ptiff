# RFC-7002: PTIFF Extension Metadata Codec

**Status:** Draft
**Category:** Normative
**Requires:** RFC-0001 (PTIFF Core); TIFF 6.0 / BigTIFF
**Obsoletes:** None
**Version:** 0.1.0
**Date:** 2026-08-22

---

### Revision History

| Version | Date       | Author            | Changes |
|---------|------------|-------------------|---------|
| 0.1.0   | 2026-08-22 | PTIFF Maintainers | Initial draft. Captures the private-tag allocation (65001–65005) and the versioned string-pair payload codec already implemented by the reference implementation (`libptiff`) and referenced throughout the codebase. |

---

## 1. Abstract

This RFC defines the **normative binary codec** that carries PTIFF extension metadata in TIFF /
BigTIFF private tags. It allocates the private-tag range **65001–65005**, assigns one tag per
scientific domain, and specifies the single, versioned, self-describing key/value payload used
by all five tags. It is the concrete implementation of RFC-0001 §17 items 1 and 2 (binary layout;
tag allocation and encoding).

This RFC deliberately carries the binary tag structure and byte layout that RFC-0001 (a
foundational document) keeps out of scope. It is the document the reference implementation
refers to as "RFC-7002" in `tiff_tag.hpp`, `ptiff_metadata.hpp`, `tiff_directory.hpp`, and the
round-trip golden test.

---

## 2. Status

Draft. This RFC defines normative binary layout for the reference implementation's already-shipped
private tags. It is intended to accompany the domain-field definitions in RFC-0003–RFC-0010. A
compliant reader/writer MUST follow the payload codec in §4 and the tag allocation in §3.

---

## 3. Tag allocation

All PTIFF extension data is carried in TIFF private-tag bytes. Five tags are allocated, each typed
`BYTE` (field type `1`) and each carrying the versioned PTIFF payload of §4.

| Tag | Name | Field type | Payload | Storage prefix |
|-----|------|-----------|---------|----------------|
| 65001 | `PtiffSpice` | BYTE (1) | versioned PTIFF | `ptiff.spice.*` |
| 65002 | `PtiffCameraGeometry` | BYTE (1) | versioned PTIFF | `ptiff.camera.*` |
| 65003 | `PtiffCrs` | BYTE (1) | versioned PTIFF | `ptiff.crs.*` |
| 65004 | `PtiffScientificLayers` | BYTE (1) | versioned PTIFF | `ptiff.layers.*` |
| 65005 | `PtiffProvenance` | BYTE (1) | versioned PTIFF | `ptiff.provenance.*` |

### 3.1 Range selection

The range **65001–65005** was chosen to avoid collision with common registered TIFF extensions:

- GeoTIFF uses 33550–34735.
- GDAL uses 42112–42113.
- 65001–65005 are in the private/extension range (≥ `0x8000` = 32768) and are currently unused.

### 3.2 Reading interoperability

A generic TIFF reader that does not know these tags MUST skip them unchanged (TIFF 6.0 requires
unknown tags be ignored). This preserves the core design commitment of RFC-0001 (§8): a generic,
unmodified TIFF reader MUST be able to read the baseline image from a PTIFF file.

### 3.3 Additional allocations

No further tags are currently allocated. New extension domains (photometry, stereo, mesh, AI)
are expected to be realized on the existing Scientific-Layers (65004) and Provenance (65005)
tags, or to request new allocations through the RFC process (RFC-0001 §17 item 2). A reader MUST
tolerate a PTIFF file carrying none, some, or all of the five tags.

---

## 4. Payload codec

Each of the five tags carries a small, self-describing, **versioned map** of UTF-8 string
key/value pairs. All integers in the payload are **little-endian**.

### 4.1 Byte layout

```
 0               magic   "PTIFF"                        5 bytes (ASCII)
 5               ver     payload format version, u16    currently 1
 7               nrec    number of (key, value) records u32
 11 …  per record, repeated nrec times:
                       klen   key byte length           u16
                       key    UTF-8 key bytes           klen bytes
                       vlen   value byte length         u32
                       value  UTF-8 value bytes         vlen bytes
```

- Magic: the 5 ASCII bytes `P T I F F` (`kPtiffMagic`). Distinguishes a PTIFF extension payload
  from arbitrary bytes left in a private tag by another writer.
- Version: `kPtiffMetadataVersion = 1` (u16).
- All multi-byte integers are little-endian.

### 4.2 Canonical ordering

Records MUST be sorted by key in ascending byte-wise order, so that the encoding is
**deterministic**: the same logical metadata always yields the same bytes. This is the property
a golden digest (content hash) of the extension payload depends on.

### 4.3 Record keys

Keys select well-known per-domain field names, defined by the extension-domain RFCs
(RFC-0003–RFC-0010) and registered in the field register. On read, a key `k` under tag `T` with
domain prefix `ptiff.<domain>.*` is surfaced as `ptiff.<domain>.k`.

- **Unknown keys MUST be preserved verbatim** by a reader so that a future writer's additional
  fields survive a read–modify–write round trip.
- A reader encountering a payload `ver` newer than it knows MUST NOT fail the whole file: it MAY
  skip that tag (treating the domain as absent), exactly as a non-PTIFF reader already ignores
  private tags.

### 4.4 Decoding errors

A payload is invalid when it:

- is empty,
- lacks the `PTIFF` magic,
- has `ver` greater than the reader's `kPtiffMetadataVersion`, or
- is malformed / truncated.

A compliant reader MUST reject an invalid payload for that tag (treating the domain as absent)
rather than mis-parsing it.

---

## 5. Conformance

- **PTIFF writer:** MUST emit the exact byte layout of §4.1, sort records canonically per §4.2,
  and place each domain's fields into the matching tag of §3.
- **PTIFF reader:** MUST skip or correctly decode the five private tags; MUST tolerate absent
  tags and unknown payload versions without failing the file; MUST preserve unknown keys verbatim.

---

## 6. References

- `RFC-0001-Core.md` — PTIFF Core (motivation, goals, design principles; §17 items 1–2).
- `RFC-0002-GeoTIFF.md` — GeoTIFF relationship / interop (range-selection rationale).
- `RFC-0003`…`RFC-0010` — extension-domain field schemas carried by tags 65001–65005.
- Reference implementation: `libptiff/include/ptiff/io/backend/tiff/ptiff_metadata.hpp`,
  `libptiff/include/ptiff/io/backend/tiff/tiff_tag.hpp`,
  `specification/core/container-encoding.md`, `specification/appendices/tag-table.md`.
