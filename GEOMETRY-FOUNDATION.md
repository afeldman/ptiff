# PTIFF Geometry Foundation — `multicalc` + Screw Theory Integration

**Status:** Analysis + Phases I–IV implemented on `rust` branch (proposal text below documents the plan)
**Scope:** Current `rust` branch of the PTIFF 1.0 Rust core
**Branch:** `rust`

> **Implementation status** (updated after Phase IV):
> - ✅ **Phase I** (commit `4246dd9`): `multicalc` dep, storage values (`Vec3`, `Quaternion`,
>   `Extrinsics`, `Intrinsics`), `Frame`/`FramePair`.
> - ✅ **Phase II** (commit `b6e8725`): `Pose` (SE3+frames), `Screw`/`ScrewAxis`/`ScrewMotion`,
>   `Camera` (K·[R|t]), `Planet`, `Ellipsoid`, `LensModel`, `Projection`,
>   `CoordinateReferenceSystem`, `Geometry`.
> - ✅ **Phase III** (this commit): `Quaternion` Euler helpers (`from_euler_angles[_deg]`,
>   `to_rad`/`quaternion2rad`/`to_deg`, `to_angle_axis`), serialization MVP (serde round-trip
>   tests for the serde-capable geometry values), 182 tests green.
> - ✅ **Phase IV** (this commit): `SpiceState` SPICE pose mapping (`(p, q, v, ω)` → `Pose` +
>   `Twist`/`Screw`, `SE3::adjoint` body↔spatial transform, `propagate`/`propagate_screw`).
>   Scene wiring + `ptiff-c`/`ptiff-rust` exposure deliberately deferred: the C++ `Scene`
>   oracle has no `addCamera`/`addGeometry` yet (M3 is an open RFC per `ROADMAP.md`) and the
>   `ptiff-c` crate does not yet exist.

> This document is an implementation-oriented architecture proposal. It does **not**
> redesign PTIFF 1.0. It extends the *current* Rust core with a reusable mathematical
> geometry foundation while keeping the established Rust-core / C-ABI / PyO3 / Octave
> architecture, API boundaries, FFI and packaging strategy intact.

---

## 1. What already exists (inspection result)

### 1.1 Rust core today
The `ptiff-core` crate is a lean, dependency-free, `#![forbid(unsafe_code)]`,
`#![warn(missing_docs)]` domain-model core. It currently contains **no geometry,
linear-algebra, or transform code whatsoever**:

- `error`, `id`, `pixel_type`, `image`, `scene`, `tile`, `io`
- Optional, feature-gated `serde` / `memory-backend`
- **99 tests green**, default build dependency-free (`cargo tree --no-default-features`
  shows only `ptiff-core`)

### 1.2 The C++ geometry domain (the migration oracle)
The C++ library (`libptiff/include/ptiff/geometry/`, `libptiff/src/geometry/`) defines
PTIFF's **domain semantics** for spatial data. It is deliberately **storage-value**,
not math:

| C++ type | Kind | Note |
|----------|------|------|
| `Vec3` | POD `{x,y,z}` | "No linear algebra operations are provided yet" |
| `Quaternion` | POD `{w,x,y,z}`, default `(1,0,0,0)` | scalar-first (Hamiltonian) |
| `Extrinsics` | `{Quaternion rotation; Vec3 translation}` | camera-to-world pose |
| `Intrinsics` | `{fx,fy,cx,cy}` | pinhole pixel params |
| `Camera` | composes intrinsics+extrinsics+timestamp | synthesises `P = K·[R\|t]` |
| `Planet` | name, IAU id, `Ellipsoid`, reference frame | e.g. `IAU_MOON` |
| `Ellipsoid`, `CoordinateReferenceSystem`, `Projection`, `LensModel`, `Geometry` | domain values | mostly interface stubs in Sprint 1 |

Key confirmation: the C++ `Quaternion` doc explicitly says math operations are
**intentionally deferred** so PTIFF stays free of an external math dependency. The
architecture intent is therefore: **PTIFF owns the domain values, an external math
library owns the operations.**

### 1.3 C++ scene coupling
`Scene` does **not** yet reference `Camera`/`Geometry` in the C++ side either — these
geometry types are standalone domain modules awaiting integration. So there is **no
existing Rust geometry module to migrate or preserve**; the Rust geometry module is a
**new, green-field addition** mirroring the C++ `geometry/` layout.

---

## 2. `multicalc` evaluation (v0.10.0)

`https://crates.io/crates/multicalc` · MIT · `#![no_std]` · **zero hard dependencies** ·
`alloc` optional. Designed for "state estimation, control, kinematics, Lie groups,
autodiff, and linear algebra … no heap, no panics, no unsafe."

**Critical to PTIFF's constraints:** `multicalc` is **dependency-free**, so adding it
to `ptiff-core` as an **always-on core dependency** does **not** break the "default
build dependency-free" quality gate. It is also `#![forbid(unsafe_code)]`-compatible.

### 2.1 Coverage vs. PTIFF needs

| Need | `multicalc` provides |
|------|----------------------|
| Vector types | `Vector<3/6, T>`, `Vector3D`, `Vector6D` (const-generic `Matrix<M,N,T>`) |
| Matrix types | `Matrix3D`, `Matrix4D`, `Matrix6D`, full linear algebra (LU/QR/SVD/…) |
| Quaternion | `Quaternion<T>`: scalar-first `[w,x,y,z]`, conjugate/inverse/norm, slerp, euler, axis-angle, scaled-axis, `from_two_vectors`, rotation_matrix ↔ quaternion, exp/ln |
| SO(3) | `SO3<T>`: identity, `from_quaternion`, compose, inverse, act, exp/log, hat/vee, adjoint, `from_two_direction_pairs`, interpolate (slerp), left/right Jacobians + inverses |
| SE(3) | `SE3<T>`: `from_parts(rotation, translation)`, compose/inverse via `*`, act, **exp/log** `[v;ω]`, **adjoint** `[[R,[t]ₓR],[0,R]]`, hat/vee, `to_matrix`/`try_from_matrix`, **interpolate** (geodesic screw motion), left/right Jacobians + inverses |
| so(3)/se(3) tangent | hat/vee on all four; `SE3::exp(Vector6D)`/`log`; `SO3::exp(Vector3D)`/`log` |
| Twist / Wrench | `Twist` (`[v;ω]`, typed linear+angular, scale/add/sub/neg, `from_vector`) and reciprocal `Wrench` (`[f;τ]`) — the screw-theory carriers |
| Adjoint | `SE3::adjoint` → `Matrix6D`; `SO3::adjoint` → `Matrix3D` |
| Jacobians | left/right + inverses (SO(3) and SE(3)), with small-angle Taylor-series fallbacks for numerical stability at θ→0 |
| f32/f64 | `Numeric` trait implemented for both; whole stack generic over `T: Numeric` |
| Autodiff | `Dual`, `HyperDual`, `Jet`, `Primal`, `ScalarFn(N)`, `Const`, `c()` — for later Jacobian/optimization work |
| `no_std` | `#![no_std]` default; no-alloc spatial/linear-algebra layer |

### 2.2 Conventions (verified)
- **Quaternion scalar-first `[w,x,y,z]`** — identical to C++ PTIFF `Quaternion`. 1:1 mapping.
- **Se(3) tangent ordering `[v; ω]`** (linear first, angular second) — matches SPICE/robotics convention; `Twist::new(linear, angular)`.
- **Wrench reciprocal `[f; τ]`** — force-first, reciprocal to the twist ordering.
- SO(3)/SE(3) use **rotation group geodesic interpolation** (`interpolate` = screw motion for SE(3), slerp for SO(3)/quaternion).

### 2.3 Gaps (what `multicalc` does **not** provide that PTIFF wants)
`multicalc` gives the **Lie-algebra machinery** but deliberately does **not** wrap the
se(3) `log` output into *named* screw primitives, nor attach PTIFF frame semantics:

- No named `ScrewAxis` / `ScrewMotion` / `ScrewPitch` types (the `[v;ω]` twist, SE(3)
  log decomposition, and `interpolate` provide the raw machinery; a small PTIFF-side
  wrapper turns a twist into an axis + point + pitch).
- No notion of **frames** (`camera → spacecraft`, `body → camera`, `spacecraft → J2000`,
  …). That is PTIFF domain semantics, not math-library scope.
- No ISO-8601 timestamp / `Planet` / `CRS` / camera intrinsics — all PTIFF domain.

---

## 3. Screw Theory mapping

**Screw Theory is a mathematical extension of the SE(3)/se(3) foundation.** Everything
PTIFF's spatial camera/SPICE needs is directly representable on `multicalc`:

| Screw concept | Representation | `multicalc` support |
|---------------|----------------|---------------------|
| Twist | se(3) element `[v; ω]` | `Twist`, `SE3::log`, `Vector6D` |
| Screw axis | unit twist (normalised `v`, `ω`) | decompose from `Twist`/`log` (PTIFF wrapper) |
| Screw motion | `exp(ξ·θ)` / `SE3::interpolate(t)` | `SE3::exp`, `SE3::interpolate` |
| Screw pitch | `h = (v·ω)/\|ω\|²` (or `\|v\|/\|ω\|` for pure rotation) | PTIFF wrapper over `Twist` |
| Spatial velocity | body-fixed twist in inertial frame | `Twist` + `SE3::adjoint` chain rule |
| Body velocity | body-frame twist | `Twist`, `SE3::inverse().adjoint()` |
| Exponential coordinates | `[v;ω]` (SE(3) log output) | `SE3::exp`/`SE3::log` |
| Adjoint | `[[R,[t]ₓR],[0,R]]` | `SE3::adjoint` |

**Conclusion:** no separate `screw-theory` crate is warranted. The concept stack is
cleanly expressed as (a) `multicalc` Lie algebra + (b) a **thin PTIFF-side `Screw`/
`Pose` wrapper** that adds axis/pitch extraction and frame labels. A standalone crate
would add packaging overhead without independent reusability.

---

## 4. Proposed architecture (extends current core, changes nothing)

```
                 ┌─────────────────────────────────────────────┐
                 │              ptiff-core                      │
                 │   (reference core, no C ABI, no_std-safe)    │
                 │                                               │
                 │   domain/model types (POD storage values)     │
                 │     Image, Scene, Tile, StorageModel, ...     │
                 │                                               │
                 │   ┌─ geometry foundation (NEW) ─────────────┐ │
                 │   │  math kernel      = multicalc (dep)     │ │
                 │   │  PTIFF wrappers:  Pose/Frame/SE3-view   │ │
                 │   │                    Screw (axis/pitch)   │ │
                 │   │  domain types:    Vec3, Quaternion,     │ │
                 │   │                    Extrinsics, Camera,   │ │
                 │   │                    Planet, CRS, ...      │ │
                 │   └─────────────────────────────────────────┘ │
                 └─────────────────────────────────────────────┘
                          │
                       stable C ABI (ptiff-c, later)
```

### 4.1 Where it lives
New `crates/ptiff-core/src/geometry/` module, mirroring `libptiff/include/ptiff/geometry/`
file-for-file so the C++↔Rust migration oracle maps cleanly:

```
crates/ptiff-core/src/geometry/
  mod.rs                 # re-exports
  vector3.rs             # Vec3 (PTIFF storage value)
  quaternion.rs          # Quaternion (PTIFF storage value; defaults identity)
  extrinsics.rs          # Extrinsics { rotation: Quaternion, translation: Vec3 }
  intrinsics.rs          # Intrinsics { fx, fy, cx, cy }
  camera.rs              # Camera (model, intrinsics, extrinsics, timestamp) + P=K·[R|t]
  planet.rs              # Planet, Ellipsoid
  coordinate_reference_system.rs  # CRS
  projection.rs          # ProjectionKind + named params
  lens_model.rs          # LensModelKind + named params
  scene_geometry.rs      # GeometryKind + named-param store (stub, mirrors C++ Geometry)
  pose.rs                # PTIFF-specific SE(3) pose with explicit frames  (NEW, Rust-side)
  frames.rs              # FrameId / frame-pair labels                     (NEW, Rust-side)
  screw.rs               # ScrewAxis, ScrewMotion, ScrewPitch wrapper      (NEW, Rust-side)
```

### 4.2 Dependencies
- **Add** `multicalc = "0.10"` as an **always-on** `ptiff-core` dependency (dependency-free,
  so the default-build gate stays green).
- **No other new dependency.** No nalgebra/glam/cgmath needed — `multicalc` covers
  vectors/matrices/quaternions/Lie groups/autodiff.
- **Remove:** nothing.

### 4.3 What `multicalc` should provide (the boundary)
`multicalc` = the **mathematical kernel**, generic over `T: Numeric`, `no_std`, no-alloc:
- `SE3`/`SO3` with exp/log/adjoint/hat/vee/Jacobians/compose/inverse/act/interpolate.
- `Twist`/`Wrench`, `Quaternion`, `Vector3D/6D`, `Matrix3/4/6D`.
- f32/f64 via `Numeric`; autodiff via `Dual`/`Jet` (later).

### 4.4 What PTIFF should provide (the domain)
PTIFF = the **domain semantics** riding on the kernel:
- Storage-value types (`Vec3`, `Quaternion`, `Extrinsics`, `Intrinsics`) — 1:1 with C++
  PODs; **these remain the canonical PTIFF serialization/interop representation.**
- `Pose` — a PTIFF wrapper that owns an `SE3<f64>` **and an explicit `Frame` pair**
  (`from`/`to`). This answers "is this `camera→spacecraft` or `spacecraft→J2000`?"
- `Frame` labels (`FrameId`, with convenience constants) — cheap, non-generic,
  carries the scientific meaning without type-level over-engineering.
- `Screw` wrappers (axis/pitch/motion) over `Twist`.
- Camera projection (`intrinsicsMatrix`, `extrinsicsMatrix`, `projectionMatrix`),
  `Planet`/`CRS`/`Projection`/`LensModel` domain values — mirroring C++.

### 4.5 Which existing PTIFF types change
**None of the serialized domain types change.** The migration oracle (C++ `Vec3`,
`Quaternion`, `Extrinsics`) stays as-is. The recommendation for the C++ `Camera pose`
(see §5) is to keep **quaternion + translation as the canonical storage** and make
`SE3` the **computed/view representation** — which is exactly the current C++ design.

---

## 5. Camera geometry decision

**Decision: quaternion + translation remains the canonical PTIFF representation;
SE(3) becomes the computed view/conversion.**

Rationale (per the user's requested criteria — not elegance, but engineering):

- **Serialization + interop:** `Vec3`/`Quaternion` are the established C++/FFI PODs.
  Switching the file format / ABI to SE(3) would churn bindings and format needlessly.
- **API stability:** the C++ API (`Extrinsics`, `Camera::extrinsicsMatrix()`) is the
  stable surface; introducing `SE3` as an input type would be a breaking change.
- **Numerical behavior:** quaternion + translation is a compact, well-conditioned
  storage; `SE3` (rotation + translation) is internally the *same* data — so `SE3`
  is a *view* over the same numbers, losing nothing numerically.
- **Scientific meaning:** an `SE3` alone cannot say "camera-to-world." Attaching
  `Frame` to the PTIFF `Pose` wrapper preserves that meaning; the underlying `SE3`
  stays a transparent mathematical object.

So: PTIFF `Extrinsics` provides `as_se3()` / `from_se3()`; PTIFF `Camera` and
`Pose` operate on `SE3` internally (for composition, interpolation, adjoint) while
**storing** quaternion + translation.

---

## 6. SPICE mapping (no CSPICE integration unless requested)

SPICE state `(position, velocity, orientation, angular_velocity)` maps onto the
SE(3)/Twist foundation as:

```
 SPICE state
   position p           ──►  SE3 translation
   orientation q        ──►  SE3 rotation (Quaternion→SO3→SE3)
   velocity v           ──►  Twist.linear   (spatial/body)
   angular velocity ω   ──►  Twist.angular
              │
              ▼
   SE3 (pose)  +  body/spatial Twist(se(3))
              │
       SE3::adjoint ● Twist     (velocity coordinate transform)
   SE3::exp(v,ω)·Δt ──► SE3::interpolate (propagation over Δt)
              ▼
   PTIFF camera geometry (Pose + Frame)
```

SPICE (CSPICE) integration is **not** implemented now — this is the mathematical
mapping layer only, kept behind the `Pose`/`Screw` abstraction so it can be added in a
later phase without touching the format or the domain values.

---

## 7. Serialization (keep the format minimal)

**Recommendation: do NOT add Screw/Twist to the PTIFF file format yet.**

- The `[v;ω]` twist, screw axis/pitch, and spatial/body velocity are **derived /
  runtime state** from pose + epoch, not independent persistent scientific metadata.
- PTIFF already persists frames/poses via the domain values; velocity/attitude-rate
  is a `Pose`/`Screw` concern, not a storage-model field.
- Keep serialization additions gated behind a *future* decision only if a concrete
  scientific need (e.g. trajectory segments) emerges — and only after `serde`-derive
  on the domain values is reviewed.

---

## 8. Implementation plan (incremental, non-breaking)

### Phase I — geometry kernel (pure add, nothing changes)
1. Add `multicalc = "0.10"` as a hard `ptiff-core` dependency (default-build gate stays
   green; verify with `cargo tree --no-default-features`).
2. `geometry/{vector3,quaternion,extrinsics,intrinsics}.rs` — storage PODs 1:1 with C++,
   with `#[cfg(feature="serde")]` serde derives added to the existing serde derive set.
3. `geometry/frames.rs` — `Frame`/`FrameId` labels (`IAU_MOON`, `J2000`, `body`,
   `spacecraft`, `camera`, generic `from_/to_`).

### Phase II — SE(3) pose + screw wrappers (adds on the kernel)
4. `geometry/pose.rs` — `Pose { se3: SE3<f64>, frame_from, frame_to }` with
   `compose`/`inverse`/`relative`/`act`/`as_se3`/`from_se3`.
5. `geometry/screw.rs` — `Screw` (axis/pitch/motion) wrapping `Twist`/`SE3::log`.
6. `geometry/camera.rs` — `Camera`, `intrinsicsMatrix`, `extrinsicsMatrix`,
   `projectionMatrix` = `K·[R|t]` (verify against the existing C++ implementation).
7. `geometry/{planet,ellipsoid,coordinate_reference_system,projection,lens_model,scene_geometry}.rs`
   — domain values mirroring C++.
8. Re-export from `geometry/mod.rs` + `lib.rs`.

### Phase III — tests (property-based preferred)
9. Tests from Step 9: SE(3) identity/composition/inverse/relative,
   quaternion↔rotation↔SO(3), pose↔SE(3), twist↔SE(3), exp/log round trips, screw-axis,
   frame correctness, numerical edge cases (small-angle, pure rotation, pure translation).
   Property tests (e.g. `exp∘log == id`) where appropriate; roundtrip vs. the C++ camera.
10. Serialization MVP behind the existing `serde` feature only.

### Phase IV — later (out of this proposal's code scope)
- Wire geometry into `Scene`/`Image`/`StorageModel` (Stage per the C++ roadmap).
- SPICE `Pose` mapping behind the abstraction; camera pose propagation via `interpolate`.
- `ptiff-rust` / `ptiff-c` exposure via the stable C ABI (unchanged strategy).

---

## 9. Concrete recommendation

1. **`multicalc` should provide** the complete, `no_std`, zero-dep Lie-algebra and
   linear-algebra kernel: `SE3`/`SO3` (+ exp/log/adjoint/hat/vee/Jacobians/interpolate),
   `Twist`/`Wrench`, `Quaternion`, `Vector3D/6D`, `Matrix3/4/6D`, generic over
   `T: Numeric` (f32/f64), with autodiff available later. It already does — **adopt it
   as the math backend as-is**, no forking.

2. **PTIFF should provide** the domain semantics: storage-value types 1:1 with the C++
   migration oracle, the `Pose`/`Frame` abstraction (explicit `from→to` frame labels),
   `Screw` axis/pitch/motion wrappers, camera/projection, and `Planet`/`CRS`/`Projection`/
   `LensModel` — all as a new `geometry` module in `ptiff-core`.

3. **No separate Screw-Theory crate is necessary** — the Lie machinery already lives in
   `multicalc`; PTIFF adds a thin `Screw`/`Pose` wrapper inside `geometry/`.

4. **Existing PTIFF types do not change.** `Vec3`/`Quaternion`/`Extrinsics` remain the
   canonical, serialized, FFI-stable representation. `SE3` is a computed view.

5. **Smallest safe sequence:** add `multicalc` dep → add storage PODs → add `Frame` →
   add `Pose`+`Screw` → add `Camera`+domain values → property/roundtrip tests → wire
   into `Scene` (Phase IV). Each step compiles green, keeps the default build
   dependency-free, and keeps the C++ migration oracle intact.

---

## 10. Open questions for the user
- **f32 support:** PTIFF is `f64`-first (C++ uses `double`). Keep `geometry` `f64` only,
  or expose the generic `T: Numeric` surface? (Recommendation: `f64` canonical, generic
  behind a `multicalc`-only escape hatch.)
- **Frame representation:** lightweight string/`FrameId` labels (recommended) vs.
  typed enums in the public API.
- **SPICE / CSPICE:** confirm whether a real CSPICE binding is wanted only in a *later*
  phase (this proposal maps the math now, integrates SPICE later).
