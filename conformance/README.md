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

Executable tests live next to the implementation they validate so they can compile against
internal headers and link the library under test. Specifically:

- **Baseline / container conformance** → `libptiff/tests/conformance/` (Catch2, wired into
  CTest), mirroring this top-level suite. See [matrix.md](matrix.md) for the files.
- Future extension-domain suites will follow the same split: normative prose here,
  executable tests under `libptiff/tests/`.

## Conformance levels (summary)

The full normative definitions are in [levels.md](levels.md). In short, from bottom to top:

| Level | Name | Meaning |
|-------|------|---------|
| 0 | **Baseline / container** | Parseable as valid TIFF/BigTIFF; generic readers find & decode the baseline image |
| 1 | **PTIFF Core** | Baseline image + PTIFF structured metadata (private-tag IFD/sub-IFD encoding) |
| 2+ | **Extension domain** | One domain each (camera, CRS, SPICE, stereo, photometry, mesh, AI) |
| 3 | **Full PTIFF** | Core + all ratified extension domains |

## How to run

The baseline conformance tests are CMake/CTest targets just like the rest of the test tree:

```bash
# From a configured build (see the build instructions in the repository README):
cmake --build <build-dir>
ctest --test-dir <build-dir> -R conformance --output-on-failure
```

To run every conformance executable including the not-yet-populated extension domains,
drop the `-R conformance` filter once they exist.

## Drafting rule

Anything inside `conformance/` that is normative (levels, required tests, self-declaration
criteria) must be ratified via an RFC before an implementation may cite it in a conformance
claim. Rationale, scope, and open questions inherited from RFC-0001 §17 live in the linked
extension RFCs as they are written.
