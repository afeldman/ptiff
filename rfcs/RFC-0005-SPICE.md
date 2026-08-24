# RFC-0005: SPICE Integration

**Status:** Draft
**Category:** Normative
**Requires:** RFC-0001 (PTIFF Core), RFC-7002 (Extension Metadata Codec)
**Obsoletes:** None
**Version:** 0.1.0
**Date:** 2026-08-22

---

### Revision History

| Version | Date       | Author            | Changes |
|---------|------------|-------------------|---------|
| 0.1.0   | 2026-08-22 | PTIFF Maintainers | Initial draft. Reflects the SPICE-derived pointing fields shipped by the reference implementation (`ptiff.spice.*`, private tag 65001). |

---

## 1. Abstract

This RFC defines the **SPICE domain** of PTIFF — NAIF SPICE-derived acquisition context for a
planetary image: the reference frame, the instrument, the time system, and the observation
time. Together these identify the pointing / position geometry sidecar data (SPICE kernels) a
consumer would load to reconstruct the observing geometry, complementing the explicit camera
calibration of the Camera domain (RFC-0003, tag 65002). It is carried by the private tag
**65001** (`PtiffSpice`) under `ptiff.spice.*`. This RFC resolves RFC-0001 §17 item 5.

> SPICE — the NAIF ancillary information system: Kernels (SPK, CK, FK), frames and coordinate
> reference frames used for planetary trajectory and pointing.

The tag 65001 is described in the reference implementation as *"SPICE-derived geometry/pointing
kernels"*.

---

## 2. Status

Draft. The field schema below is normative and matches the reference components
(`crates/ptiff-rust/examples/write_ptiff_fixture.rs`, `crates/ptiff-core/tests/`).

---

## 3. Field register

| Field | Type | Required | Meaning |
|-------|------|----------|---------|
| `ptiff.spice.frame` | string | no | SPICE / natural reference frame, e.g. `IAU_MOON` |
| `ptiff.spice.instrument` | string | no | Instrument / instrument ID, e.g. `LROC_NAC`, `LROC_WAC` |
| `ptiff.spice.time_system` | string | no | Time system, e.g. `TDB` |
| `ptiff.spice.observation_time` | string | no | ISO-8601 observation time of the acquisition |

### 3.1 Example values (from the interop fixture)

```
ptiff.spice.frame            = IAU_MOON
ptiff.spice.instrument       = LROC_NAC
ptiff.spice.time_system      = TDB
ptiff.spice.observation_time = 2026-08-20T00:00:00.000
```

---

## 4. Semantics

- All values are informational metadata a PTIFF-aware consumer can use to locate and apply the
  matching SPICE kernels. PTIFF itself does **not** embed kernel binary data.
- `frame` SHOULD be a valid NAIF frame name / ID.
- `time_system` SHOULD be a NAIF time system name (`TDB`, `ET`, `UTC`, …).
- The Camera domain's extrinsic pose (RFC-0003, tag 65002) and the SPICE kernels referenced
  here are two representations of the same observing geometry; when both are present they SHOULD
  describe the same pose.

---

## 5. Conformance

- A **writer** MUST serialize SPICE fields into tag 65001 using the RFC-7002 codec.
- A **reader** MUST tolerate a file with no SPICE metadata.
- PTIFF does not itself validate whether the referenced kernels exist; kernel application is
  the consumer's responsibility.

---

## 6. References

- `RFC-0001-Core.md` — §9, §17 item 5.
- `RFC-7002` — payload codec, tag 65001.
- `RFC-0003` — Camera domain (explicit extrinsics, tag 65002).
- Reference implementation: `specification/spice/pointing.md`,
  `crates/ptiff-rust/examples/write_ptiff_fixture.rs`.
