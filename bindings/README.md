# Language bindings for libptiff

All bindings share one substrate: [`bindings/c`](./c), a **language-agnostic C
ABI** built as `libptiff_c`. Because every foreign-language runtime can call a
plain C ABI, that single veneer is the source of truth all bindings build on —
the C++ internals and their dependencies (fmt, spdlog) stay hidden behind it.

> **Direction (2026-08-11), Go promoted (2026-08-12), Python and Ruby promoted
> (2026-08-13), Octave added (2026-08-22).** SWIG is the **primary** binding
> layer (`bindings/swig`).
> **All four targeted languages are now promoted**: `bindings/go/ptiff`,
> `bindings/python/src`, `bindings/ruby/lib` and `bindings/octave/lib` are all
> SWIG output, regenerated from `bindings/swig/ptiff.i`; the hand-written cgo,
> ctypes and Fiddle wrappers are all deleted. `bindings/swig` itself holds no
> per-language staged output anymore — only the shared `ptiff.i` /
> `typemaps.i` / `Makefile` driving all four. **Rust stays on rust-bindgen**
> (no SWIG Rust target). See [`bindings/swig`](./swig) → "Planned migration".

```
            libptiff (C++23, fmt/spdlog)
                 │
        bindings/c → libptiff_c   (extern "C" veneer)
                 │
        bindings/swig → ptiff.i + typemaps.i   (one .i, four language targets)
                 │
      ┌───────────┼───────────┬──────────┬──────────┐
      ▼           ▼           ▼          ▼          ▼
     Go         Rust       Python       Ruby      Octave
  (promoted:   (bindgen)  (promoted:   (promoted: (promoted:
  bindings/go)            bindings/    bindings/  bindings/
                          python)      ruby)      octave)
                 │
                 ▼
        bindings/mcp → ptiff_mcp (MCP server over the Python binding)
```

| Binding | Directory | Approach          | Status |
|---------|-----------|-------------------|--------|
| **C**   | `bindings/c` | `libptiff_c` (the ABI itself) | ✅ implemented, installable |
| **Go**  | `bindings/go` | SWIG output over `libptiff_c` (promoted from `bindings/swig`) | ✅ tested, official |
| **Rust**| `bindings/rust` | Cargo `#[link]`/FFI over `libptiff_c` | ✅ tested |
| **Python**| `bindings/python` | SWIG output over `libptiff_c` (promoted from `bindings/swig`) | ✅ tested, official |
| **Ruby**| `bindings/ruby` | SWIG output over `libptiff_c` (promoted from `bindings/swig`) | ✅ tested, official |
| **Octave**| `bindings/octave` | SWIG output over `libptiff_c` (promoted from `bindings/swig`) | ✅ tested, official |
| **SWIG**| `bindings/swig` | SWIG (pure C) over `libptiff_c`; all four language outputs promoted to their own dirs | ✅ round-trip + full test parity in all four |
| **MCP** | `bindings/mcp` | MCP server (Model Context Protocol) over the Python binding, for LLMs | ✅ 10 tools, tested (in-process + stdio) |

Every binding covers the same implemented surface: versioning, error codes,
logging, the backend registry, `Image` domain type plus its descriptor/value
types, and tile read/write.

A thin **command-line tool** sits on top of the Rust binding: [`../ptiff-cli`](../ptiff-cli)
builds a `ptiff` binary that wraps the same ABI (`version`, `backends`, `logger`,
`info`).

A **Model Context Protocol (MCP) server** sits on top of the Python binding:
[`bindings/mcp`](./mcp) exposes the same ABI as a set of LLM tools over stdio
(`get_version`, `read_metadata`, `read_tile`, `write_image_file`, …) so that MCP
clients such as Claude Code can work with PTIFF files directly. See its
[`README.md`](./mcp/README.md) and [`DESIGN.md`](./mcp/DESIGN.md).

## Building and testing

First build `libptiff_c` from the repo root (shared, so backend registration
stays visible):

```bash
cmake -B build -S . -DBUILD_SHARED_LIBS=ON -DPTIFF_BUILD_C_BINDINGS=ON
cmake --build build --target ptiff_c
```

Then point each binding at the output (`PTIFF_C_LIB_DIR` / rpath) and run its
tests — see the per-binding READMEs.

### Consuming an installed prefix (pkg-config)

The C/C++ libraries also generate **relocatable pkg-config files** (`libptiff.pc`,
`libptiff_c.pc`) and, for shared builds, `libptiff_c.dylib` ships an
`@loader_path`/`$ORIGIN` instal rpath. So every binding can consume a *single
installed prefix* cleanly — no repo-relative paths, no `DYLD_`/`LD_` plumbing:

```bash
cmake --build build --target ptiff_c
cmake --install build --prefix /where/ever

# Rust binding, CLI, Go, SWIG:
PKG_CONFIG_PATH=/where/ever/lib/pkgconfig cargo test    # bindings/rust, ptiff-cli
PKG_CONFIG_PATH=/where/ever/lib/pkgconfig go test ./ptiff          # bindings/go (+CGO_*)
PKG_CONFIG_PATH=/where/ever/lib/pkgconfig make test               # bindings/swig (all four languages)

# Python, Go, Ruby and Octave are all SWIG output -- regenerate before testing
# directly (make test above does this automatically for all four):
PKG_CONFIG_PATH=/where/ever/lib/pkgconfig make -C bindings/swig python
cd bindings/python && uv run pytest

PKG_CONFIG_PATH=/where/ever/lib/pkgconfig make -C bindings/swig ruby
cd bindings/ruby && bundle exec rake test

PKG_CONFIG_PATH=/where/ever/lib/pkgconfig make -C bindings/swig octave
octave --no-gui --eval "addpath('bindings/octave/lib','bindings/octave/test'); run_tests_octave();"
```

Each binding's `build.rs`/loader falls back to `PTIFF_C_LIB_DIR` (or the
repo-relative build output) for fast in-tree dev without an install step.

## Adding an API

Extend the `ptiff_*` surface in `bindings/c/*.{h,cpp}` once, then wrap it thinly
in each language. High-level bindings never touch libptiff's C++ headers.
