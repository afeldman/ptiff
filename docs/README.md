# PTIFF Documentation

This directory is the single entry point for the PTIFF **software
documentation** — the documentation that is *generated* from the source tree
and configuration committed next to it. It covers the two documentation
ecosystems of the repository:

```
PTIFF Documentation
        │
        ├── C/C++ API and FFI surface
        │      └── Doxygen          (site: build/docs/html/index.html)
        │           sources: docs/doxygen/  ·  config: ./Doxyfile
        │
        └── Rust API
               └── Rustdoc          (site: target/doc/ptiff/index.html)
                    notes: docs/rustdoc/
```

PTIFF's reference implementation is a pure-Rust workspace
(`ptiff-core` → `ptiff-rust` → `ptiff-cli` / `ptiff-c`). The C and C++ surface
is a deliberately thin FFI layer over that core, so the two documentation sets
do **not** document disjoint worlds — they document one architecture from the
two sides of the same boundary:

| What you want to read about                            | Set          | Generator |
|--------------------------------------------------------|--------------|-----------|
| Rust core & idiomatic API (`ptiff-core`, `ptiff-rust`) | Rust API     | rustdoc   |
| CLI crate (`ptiff-cli`) and C-ABI crate (`ptiff-c`)    | Rust API     | rustdoc   |
| C ABI `libptiff_c` / `target/ptiff_c.h` (cbindgen)     | C/C++ API    | Doxygen   |
| C++ binding `ptiff-cpp` (`bindings/cpp`)               | C/C++ API    | Doxygen   |
| Concept & overview pages                               | C/C++ API    | Doxygen   |

Generated documentation is **never committed**: Doxygen writes below
`build/docs/` and rustdoc writes below `target/doc/`, both of which are ignored
by Git. Everything under `docs/` itself is committed source — hand-written
Markdown, configuration notes, and the Doxygen overview pages.

---

## Generate the Doxygen documentation (C ABI + C++ binding)

The Doxygen site documents the generated C ABI header `target/ptiff_c.h`, the
header-only C++ binding under `bindings/cpp/include/`, and the hand-written
overview pages in `docs/doxygen/`. Configuration lives in the repository-root
`Doxyfile` (output: `build/docs/html/` + `build/docs/xml/`).

Prerequisites:

* Doxygen ≥ 1.9
* the generated C ABI header `target/ptiff_c.h` (created by `cargo build -p
  ptiff-c`, whose `build.rs` runs cbindgen)

```bash
# 1. Generate the C ABI header (once per clean checkout)
cargo build -p ptiff-c

# 2. Build the Doxygen site (mkdir: Doxygen >= 1.18 refuses to create the
#    output directory itself; the CI workflows do the same)
mkdir -p build/docs
doxygen Doxyfile          # -> build/docs/html/index.html

# or, when the project was configured with CMake:
cmake --build build --target docs
```

## Generate the rustdoc documentation (Rust API)

The Rust workspace (`crates/ptiff-core`, `crates/ptiff-rust`,
`crates/ptiff-cli`, `crates/ptiff-c`) is documented with rustdoc. See
`docs/rustdoc/README.md` for details.

Prerequisites: Rust stable (see `rust-toolchain.toml`; MSRV 1.97).

```bash
# All workspace library crates, all features, without dependency sources
cargo doc --workspace --all-features --no-deps --lib
# -> target/doc/ptiff/index.html (open with: cargo doc -p ptiff --open)
```

`--lib` documents the library targets of the workspace. It is required for a
clean site: the CLI package `ptiff-cli` ships a binary also named `ptiff`, and
rustdoc would otherwise collide the binary's page with the `ptiff` library
page under `target/doc/ptiff/` (the CLI binary has no public API to document).

## Generate everything

```bash
cargo build -p ptiff-c            # C ABI header for Doxygen
mkdir -p build/docs               # Doxygen >= 1.18 needs the output dir
cargo doc --workspace --all-features --no-deps --lib   # Rust API
doxygen Doxyfile                  # C/C++ API + concept pages
```

## Tool requirements

| Tool          | Used for                       | Minimum        |
|---------------|--------------------------------|----------------|
| Rust + Cargo  | rustdoc, cbindgen (`build.rs`) | stable / MSRV 1.97 |
| Doxygen       | C/C++ API site                 | 1.9            |

## How generated documentation is treated by Git

* `docs/` — **committed**: Markdown sources and configuration
  (`docs/README.md`, `docs/doxygen/`, `docs/rustdoc/`).
* `build/docs/` — **ignored**: Doxygen HTML/XML output.
* `target/doc/` — **ignored**: rustdoc HTML output (covered by `target/`).

Do not add generated output to a commit; regenerate it instead.

## CI behaviour

* Doxygen is validated and deployed to GitHub Pages on version tags
  (`v*`) via `.github/workflows/pages.yml` (Phase 3 of `ci.yml`).
* A lightweight `docs-check` runs on every push/PR
  (`.github/workflows/docs-check.yml`): it generates the C ABI header, builds
  the Doxygen site, and builds the rustdoc site with `RUSTDOCFLAGS="-D
  warnings"` so configuration and intra-doc-link regressions are caught
  without committing any generated output.
