# PTIFF Conformance Suite

This directory hosts the **normative conformance suite** for the Planetary TIFF (PTIFF)
format, as called for by RFC-0001 §17.11 ("Validation and conformance testing").

A conformance suite is the executable + documentary agreement by which an implementation
may claim to be PTIFF-conformant. It defines:

- **What** is tested (the observable on-disk and behavior contracts),
- **How deeply** it is tested (a ladder of *conformance levels*), and
- **The evidence** a self-declaration of conformance must cite.

PTIFF deliberately lives *on top of* the baseline TIFF/BigTIFF specifications. Therefore the
bottom rung of the ladder is the **baseline / container level**, and exactly that rung is what
this directory begins to fill with executable tests (Option C scope). Everything above it
(camera, CRS, SPICE, stereo, photometry, mesh, AI) is reserved for the corresponding extension
RFCs and lands here as those RFCs are ratified and implemented.

---

## Directory layout

```
conformance/
├── README.md            # this file -- suite purpose, scope, how to use
├── levels.md            # normative conformance levels (RFC §17.11 part 1)
├── matrix.md            # test matrix: requirements -> levels -> test files
└── (extension domains)  # future per-RFC suites, e.g. camera/, crs/, spice/, ...
```

Executable tests live next to the implementation they validate so they can link the library
under test. The reference implementation is a Rust workspace: **baseline / container
conformance** is covered by the Rust tests under `crates/ptiff-core/tests/` (see
[matrix.md](matrix.md) for the files). Future extension-domain suites will follow the same
split: normative prose here, executable Rust tests under `crates/ptiff-core/tests/`.

> **History:** the original baseline conformance executable tests were written as C++
> (`libptiff/tests/conformance/`, Catch2/CTest). With the migration to the pure-Rust
> reference implementation those are being re-expressed as Rust `#[test]` integration
> tests in the workspace; the normative requirements below are unchanged.

## Conformance levels (summary)

The full normative definitions are in [levels.md](levels.md). In short, from bottom to top:

| Level | Name | Meaning |
|-------|------|---------|
| 0 | **Baseline / container** | Parseable as valid TIFF/BigTIFF; generic readers find & decode the baseline image |
| 1 | **PTIFF Core** | Baseline image + PTIFF structured metadata (private-tag IFD/sub-IFD encoding) |
| 2+ | **Extension domain** | One domain each (camera, CRS, SPICE, stereo, photometry, mesh, AI) |
| 3 | **Full PTIFF** | Core + all ratified extension domains |

## How to run

The baseline conformance requirements are enforced by the Rust workspace test suite
(pure `cargo test`; Cargo is the real build):

```bash
# Run the whole workspace test suite (includes the conformance/baseline tests):
cargo test --workspace --all-features

# Or just the ptiff-core tests:
cargo test -p ptiff-core
```

To run a specific conformance-focused test, filter by its name/number as usual, e.g.
`cargo test -p ptiff-core --test golden` or `cargo test -p ptiff-core conformance` once
dedicated conformance tests exist under `crates/ptiff-core/tests/`.

## Drafting rule

Anything inside `conformance/` that is normative (levels, required tests, self-declaration
criteria) must be ratified via an RFC before an implementation may cite it in a conformance
claim. Rationale, scope, and open questions inherited from RFC-0001 §17 live in the linked
extension RFCs as they are written.
