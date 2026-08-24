# PTIFF Specification — SPICE Domain

**Status:** Draft
**Version:** 0.1.0
**Private tag:** 65001 (`PtiffSpice`)
**Storage notation:** `ptiff.spice.*`
**Normative basis:** `RFC-0001-Core.md`, reference implementation
`crates/ptiff-rust/examples/write_ptiff_fixture.rs` and `crates/ptiff-core/tests/`.

---

## 1. Purpose

The SPICE domain records **NAIF SPICE <sup>1</sup>-derived acquisition context** for a
planetary image: the reference frame, the instrument, the time system, and the observation
time. Together these identify the pointing / position geometry sidecar data (SPICE kernels)
that a consumer would load to reconstruct the observing geometry, complementing the explicit
camera calibration in the Camera domain (tag 65002).

> <sup>1</sup> SPICE — the NAIF ancillary information system: Kernels (SPK, CK, FK), frames
> and coordinate reference frames used for planetary trajectory and pointing.

The tag 65001 is described in the reference implementation as
*"SPICE-derived geometry/pointing kernels"*.

---

## 2. Field register

| Field                          | Type   | Required | Meaning                                              |
|--------------------------------|--------|----------|------------------------------------------------------|
| `ptiff.spice.frame`            | string | no       | SPICE / natural reference frame, e.g. `IAU_MOON`     |
| `ptiff.spice.instrument`       | string | no       | Instrument/instrument ID, e.g. `LROC_NAC`, `LROC_WAC`|
| `ptiff.spice.time_system`      | string | no       | Time system, e.g. `TDB`                              |
| `ptiff.spice.observation_time` | string | no       | ISO-8601 observation time of the acquisition         |

### 2.1 Example values (from the interop fixture)

```
ptiff.spice.frame            = IAU_MOON
ptiff.spice.instrument       = LROC_NAC
ptiff.spice.time_system      = TDB
ptiff.spice.observation_time = 2026-08-20T00:00:00.000
```

---

## 3. Semantics

- All values are informational metadata that a PTIFF-aware consumer can use to locate and
  apply the matching SPICE kernels; PTIFF itself does not embed kernel binary data.
- `frame` SHOULD be a valid NAIF frame name/ID.
- `time_system` SHOULD be a NAIF time system name (`TDB`, `ET`, `UTC`, …).
- The Camera domain's extrinsic pose (tag 65002) and the SPICE kernels referenced here are
  two representations of the same observing geometry; when both are present they SHOULD
  describe the same pose.

---

## 4. References

- [`../core/container-encoding.md`](../core/container-encoding.md) — tag 65001 payload encoding.
- [`../camera/calibration.md`](../camera/calibration.md) — explicit extrinsics.
