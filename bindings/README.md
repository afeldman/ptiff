# Language bindings for libptiff

All bindings share one substrate: the **Rust-generated C ABI** `libptiff_c`,
built by the Rust crate [`crates/ptiff-c`](../../crates/ptiff-c) and exposed
through a **cbindgen-generated header** (`target/ptiff_c.h`). Because every
foreign-language runtime can call a plain C ABI, that single C surface is the
source of truth all bindings build on — the Rust internals and their
dependencies stay hidden behind it.

> **Direction (2026-08-11), Go promoted (2026-08-12), Python and Ruby promoted
> (2026-08-13), Octave added (2026-08-22), Rust C ABI migration complete
> (2026-08-24).** SWIG is the **primary** binding layer (`bindings/swig`).
> **All four targeted languages are promoted**: `bindings/go/ptiff`,
> `bindings/python/src`, `bindings/ruby/lib` and `bindings/octave/lib` are all
> SWIG output, regenerated from `bindings/swig/ptiff.i`; the hand-written cgo,
> ctypes and Fiddle wrappers are all deleted. `bindings/swig` itself holds no
> per-language staged output anymore — only the shared `ptiff.i` /
> `typemaps.i` / `Makefile` driving all four. **The C ABI is now generated from
> Rust**: `bindings/c` and `bindings/swig/swig_include` were both deleted on
> 2026-08-24 — the header source of truth is now the cbindgen output
> `target/ptiff_c.h`. See [`bindings/swig`](./swig) → "Planned migration".

```
        crates/ptiff-c (Rust extern "C" surface)
                 │
        cbindgen → target/ptiff_c.h   (Rust-generated C ABI → libptiff_c)
                 │
        bindings/swig → ptiff.i + typemaps.i   (one .i, four language targets)
                 │
      ┌───────────┼───────────┬──────────┬──────────┐
      ▼           ▼           ▼          ▼          ▼
     Go         Python       Ruby      Octave
  (promoted:   (promoted:   (promoted: (promoted:
  bindings/go)  bindings/    bindings/  bindings/
                python)      ruby)      octave)
                 │
                 ▼
        bindings/mcp → ptiff_mcp (MCP server over the Python binding)
```

| Binding | Directory | Approach          | Status |
|---------|-----------|-------------------|--------|
| **C ABI**| `crates/ptiff-c` | Rust `extern "C"` surface → cbindgen → `target/ptiff_c.h`; builds `libptiff_c` | ✅ implemented, official |
| **Go**  | `bindings/go` | SWIG output over `libptiff_c` (promoted from `bindings/swig`) | ✅ tested, official |
| **Python**| `bindings/python` | SWIG output over `libptiff_c` (promoted from `bindings/swig`) | ✅ tested, official |
| **Ruby**| `bindings/ruby` | SWIG output over `libptiff_c` (promoted from `bindings/swig`) | ✅ tested, official |
| **Octave**| `bindings/octave` | SWIG output over `libptiff_c` (promoted from `bindings/swig`) | ✅ tested, official |
| **SWIG**| `bindings/swig` | SWIG (pure C) over `libptiff_c`; all four language outputs promoted to their own dirs | ✅ round-trip + full test parity in all four |
| **MCP** | `bindings/mcp` | MCP server (Model Context Protocol) over the Python binding, for LLMs | ✅ 10 tools, tested (in-process + stdio) |

Every binding covers the same implemented surface: versioning, error codes,
logging, the backend registry, `Image` domain type plus its descriptor/value
types, and tile read/write.

The SWIG driver uses a cbindgen-generated header: the `%{}` block in
`bindings/swig/ptiff.i` includes a sed-filtered copy of the generated header
(`bindings/swig/real_inc/ptiff_c.h`) that strips the `PTIFF_C_API` macro block
SWIG can't parse. The four language bindings (go/ptiff, python/src, ruby/lib,
octave/lib) are SWIG output — gitignored generated files (ptiff.go /
ptiff_wrap.c / cgo_flags.go; ptiff.py / _ptiff.so; ptiff.{bundle}; octave
wrapper) — plus tracked hand-written tests.

A **Model Context Protocol (MCP) server** sits on top of the Python binding:
[`bindings/mcp`](./mcp) exposes the same ABI as a set of LLM tools over stdio
(`get_version`, `read_metadata`, `read_tile`, `write_image_file`, …) so that MCP
clients such as Claude Code can work with PTIFF files directly. See its
[`README.md`](./mcp/README.md) and [`DESIGN.md`](./mcp/DESIGN.md).

## Building and testing

First build `libptiff_c` from the repo root (shared, so backend registration
stays visible):

```bash
cargo build -p ptiff-c --release
```

That invokes cbindgen to generate `target/ptiff_c.h` from the Rust
`extern "C"` surface and produces `libptiff_c` in `target/release`.

Then build and test all four language bindings in one shot (builds the SWIG
outputs and runs each language's tests):

```bash
make -C bindings/swig test
```

Or build/test a single language at a time — the SWIG Makefile points directly at
`target/release` (via `cgo_flags.go` for Go and rpath elsewhere):

```bash
make -C bindings/swig python   # Python (bindings/python, uv run pytest)
make -C bindings/swig go       # Go (bindings/go, go test ./ptiff)
make -C bindings/swig ruby     # Ruby (bindings/ruby, bundle exec rake test)
make -C bindings/swig octave   # Octave (bindings/octave, octave run_tests_octave)
```

## Adding an API

Extend the Rust `extern "C"` surface in `crates/ptiff-c/src/*.rs` — cbindgen
regenerates `target/ptiff_c.h` automatically — then rerun SWIG
(`make -C bindings/swig`) and wrap the new entry point thinly in each language.
High-level bindings never touch the C ABI header by hand.
