# RFC-0015: Internationalization & Units

**Status:** Draft — conceptual / reserved
**Category:** Normative (planned)
**Requires:** RFC-0001 (PTIFF Core)
**Obsoletes:** None
**Version:** 0.1.0
**Date:** 2026-08-22

---

### Revision History

| Version | Date       | Author            | Changes |
|---------|------------|-------------------|---------|
| 0.1.0   | 2026-08-22 | PTIFF Maintainers | Initial draft. Reserves the Internationalization & Units domain scope per RFC-0001 §17 item 14. |

---

## 1. Abstract

This RFC reserves the **Internationalization & Units** domain of PTIFF: canonical unit systems
and any text / locale handling requirements for human-readable metadata fields. It resolves
RFC-0001 §17 item 14.

This RFC is currently **conceptual / reserved**: no canonical unit vocabulary or locale policy is
defined yet.

---

## 2. Status and baseline assumptions

The reference implementation stores all extension metadata values as UTF-8 strings (RFC-7002
§4). The following baseline assumptions constrain a future RFC:

- Field values are UTF-8 encoded.
- Numeric fields are decimal strings (e.g. focal lengths in pixels, positions in abstract world
  units); unit semantics are currently implied by each domain (`specification/*`).
- ISO-8601 is used for timestamps in the Camera and SPICE domains (`ptiff.camera.timestamp`,
  `ptiff.spice.observation_time`).

A future RFC codifies: canonical physical units (length, radiance / reflectance, angles),
whether world units in the Camera/CRS domains are meters or body-defined, and text / locale
handling for human-readable metadata.

---

## 3. Planned scope (required of a future extension RFC)

A future normative Internationalization & Units RFC SHOULD specify:

- a **canonical unit system** and the unit for each dimensioned field (length, angle,
  radiance / reflectance, time),
- whether world-space units in the Camera and CRS domains are fixed (e.g. meters) or
  body-relative,
- text / locale handling for human-readable fields (encoding is UTF-8; any collation /
  normalization requirements),
- mapping to the Photometry (RFC-0010) and CRS (RFC-0004) domains where units are especially
  load-bearing.

---

## 4. Conformance

There are currently no normative I18n / unit requirements. A PTIFF reader MUST NOT fail a file
because a unit or locale is not recognized.

---

## 5. References

- `RFC-0001-Core.md` — §17 item 14.
- `RFC-0004` — CRS domain.
- `RFC-0010` — Photometry domain (radiance / reflectance units).
- RFC-7002 — UTF-8 string payload codec.
