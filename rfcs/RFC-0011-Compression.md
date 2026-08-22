# RFC-0011: Compression

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
| 0.1.0   | 2026-08-22 | PTIFF Maintainers | Initial draft. Reserves the Compression domain scope per RFC-0001 §17 item 9 and captures the schemes already implemented by the reference implementation. |

---

## 1. Abstract

This RFC reserves the **Compression domain** of PTIFF: which compression schemes are permitted
for baseline and extension data, and any planetary-imagery-specific compression considerations
(e.g. high dynamic range, scientific losslessness). It resolves RFC-0001 §17 item 9.

This RFC is currently **conceptual / reserved**: it records the required scope and reflects the
compression schemes already shipped by the reference implementation. A future normative version
defines the canonical per-scheme encoding rules and conformance requirements.

---

## 2. Status and reference-implementation coverage

The reference implementation (`libptiff`, `TiffBackend`) already reads and writes these
baseline TIFF compression schemes:

| Scheme | TIFF Compression value | Predictor support |
|--------|------------------------|-------------------|
| PackBits | 32773 | — |
| LZW | 5 | horizontal-differencing (Predictor 2) on strip writes |
| Deflate | 8 (and 32946) | horizontal-differencing (Predictor 2) on strip writes |
| JPEG | 7 | libjpeg-turbo; grayscale and RGB/YCbCr, UInt8-only; not combined with tiled write or a predictor |

The `predictor` core field (`None`, `Horizontal` (1), `FloatingPoint` (2)) is exposed on the
storage model per `specification/core/container-encoding.md`.

---

## 3. Planned scope (required of a future extension RFC)

A future normative Compression RFC SHOULD specify:

- a canonical table of permitted compression schemes and their TIFF tag values, including
  baseline and extension data,
- **scientific losslessness** requirements for high-dynamic-range and calibrated data,
- the exact `predictor` rules and their interaction with each codec (tiled vs. stripped,
  bit depth),
- conformance rounds for each scheme in the validation suite (`conformance/`, RFC-0013).

---

## 4. Conformance

There are currently no normative compression requirements beyond the baseline TIFF rules and
the reference implementation's documented behavior. A PTIFF reader MUST tolerate any
compression scheme it cannot decode by reporting a clear decoding error rather than corrupting
output.

---

## 5. References

- `RFC-0001-Core.md` — §12 (extensibility), §17 item 9.
- `RFC-7002` — payload codec.
- Reference implementation: `libptiff/src/io/backend/tiff/` (`tiff_image_source.cpp`,
  `tiff_image_sink.cpp`), `specification/core/container-encoding.md` §5.
