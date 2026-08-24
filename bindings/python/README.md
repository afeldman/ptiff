# Python bindings for libptiff

Idiomatic Python bindings for [libptiff](../..), via the language-agnostic C ABI
(`libptiff_c`) produced by the Rust crate
[`crates/ptiff-c`](../../crates/ptiff-c): `cargo build -p ptiff-c --release`
builds `target/release/libptiff_c.*` and cbindgen generates the single header
`target/ptiff_c.h`. Pure-C standard-library `ctypes` — no C++ compilation, no
third-party runtime deps.

## Prerequisites

The binding loads `libptiff_c` at import time, so you must first build the C ABI
library (and its transitive Rust core):

```bash
# from the repo root, with the Rust toolchain available:
cargo build -p ptiff-c --release
```

The package then locates the shared library in this order:

1. **`PTIFF_C_LIB_DIR` / `PTIFF_LIB_DIR`** env vars — explicit override for in-tree
   dev/CI against the Rust-built `target/release` (no install step).
2. **pkg-config `libptiff_c`** — the clean, layout-agnostic path for an
   *installed* library: install the `target/release` artifacts once, then point
   `PKG_CONFIG_PATH` at the prefix's `lib/pkgconfig`.
3. **Repository-relative build output** — last resort for ad-hoc dev.

For an installed prefix:

```bash
# (after cargo build -p ptiff-c --release) install libptiff_c + pc file, e.g.:
# cp target/release/libptiff_c.* /where/ever/lib && cp target/ptiff_c.h /where/ever/include
PKG_CONFIG_PATH=/where/ever/lib/pkgconfig uv run pytest
```

## Development

The bindings use `uv` for dependency management, `hatchling` for builds, and
`ruff` / `mypy` / `sphinx` for linting, type-checking and docs.

```bash
# create the virtualenv and install all optional groups (test, lint, type, docs)
uv sync --all-extras --dev

# lint & format
uv run ruff check .
uv run ruff format . --check

# static type-check (strict)
uv run mypy .

# tests (unit + cucumber/Gherkin scenarios, requires a built libptiff_c)
uv run pytest

# documentation (requires a built libptiff_c for autodoc)
uv run sphinx-build -b html docs docs/_build/html
```

## Test

Tests come in two flavours, both driven by `pytest`:

- **Unit tests** in [`test/`](test/test_ptiff.py) — fast, imperative checks of
  the binding surface.
- **Cucumber / BDD tests** in [`features/`](features/) — Gherkin `.feature`
  files ([`version.feature`](features/version.feature),
  [`backends.feature`](features/backends.feature),
  [`logger.feature`](features/logger.feature), and
  [`image.feature`](features/image.feature)) with step definitions in
  [`features/steps/`](features/steps/), driven by `pytest-bdd`.

```bash
# run everything (unit + cucumber)
uv run pytest

# run only the cucumber scenarios
uv run pytest features

# run only the unit tests
uv run pytest test
```

Without `uv`, or to run under a specific interpreter with explicit paths:

```bash
PTIFF_C_LIB_DIR=/path/to/repo/target/release \
PTIFF_LIB_DIR=/path/to/repo/target/release \
python3 -m unittest test_ptiff -v
```

## Covered API

- `Version` / `runtime_version()` / `compile_time_version()`
- `ErrorCode`, `LogLevel`, `PixelType`, `CompressionKind`
- `backend_names()`
- `Logger` (process-wide singleton handle)
- `Image` (RAII over the opaque handle) + `TileInfo`

See the [API reference](https://ptiff.org/en/latest/) for full documentation.
