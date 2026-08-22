# RFC-0012: Streaming & Cloud-Optimized Access

**Status:** Draft — conceptual / reserved
**Category:** Normative (planned)
**Requires:** RFC-0001 (PTIFF Core), RFC-7002 (Extension Metadata Codec)
**Obsoletes:** None
**Version:** 0.1.0
**Date:** 2026-08-22

---

### Revision History

| Version | Date       | Author            | Changes |
|---------|------------|-------------------|---------|
| 0.1.0   | 2026-08-22 | PTIFF Maintainers | Initial draft. Reserves the Streaming / Cloud domain scope per RFC-0001 §17 item 10 and reflects the HTTP range-request reader shipped by the reference implementation. |

---

## 1. Abstract

This RFC reserves the **Streaming & cloud-optimized access** domain of PTIFF: concrete
tiling, overview / pyramid, and range-request-friendly layout requirements so that large
planetary images can be accessed efficiently over HTTP(S). It resolves RFC-0001 §17 item 10.

This RFC is currently **conceptual / reserved**: it records the required scope and reflects the
range-request transport already shipped by the reference implementation.

---

## 2. Status and reference-implementation coverage

The reference implementation already ships a read-only cloud transport:

- **`HttpRangeBinaryReader`** reads a Cloud-Optimized TIFF / BigTIFF over HTTP(S) `Range`
  requests (via libcurl), so that only the needed tiles/strips are fetched.
- **`Reader::open`** selects this transport automatically when given an `http://` / `https://`
  URL, via `BackendFactory`.

Combined with the multi-IFD tile/pyramid support (`tileWidth`/`tileHeight`, tiled layout) in
`TiffBackend`, a COG-style layout is already producible and readable.

---

## 3. Planned scope (required of a future extension RFC)

A future normative Streaming / Cloud RFC SHOULD specify:

- concrete **tiling** requirements (tile size multiples of 16, pairing rule),
- **overview / pyramid** layout requirements across multiple IFDs,
- **range-request-friendly** byte layout (IFD offsets, tile/strip ordering) and any
  alignment/ordering constraints,
- when and how the `http://`/`https://` transport must be selected vs. local file access.

---

## 4. Conformance

There are currently no normative streaming/cloud requirements beyond the reference
implementation's documented behavior. A PTIFF reader MUST tolerate a locally-stored file that
was written for cloud access and vice versa.

---

## 5. References

- `RFC-0001-Core.md` — §12 (extensibility), §17 item 10.
- `RFC-7002` — payload codec.
- Reference implementation: `HttpRangeBinaryReader`, `Reader::open`,
  `specification/core/container-encoding.md` §3/§5 (tiled layout, multiple IFDs).
