# Rebuilding the Python binding for the new package layout

The SWIG `bindings/swig/Makefile` regenerates the promoted per-language outputs
from `ptiff.i`. With the Python package restructure, the Python target needs
one path fix and a safe Python interpreter before you `make python`.

> These steps must be run by hand: the working `bindings/swig/Makefile` is
> excluded from the assistant environment (`.gooseignore`), so it cannot be
> edited or regenerated here.

## The C ABI now comes from Rust (`target/`)

Since 2026-08-24 the C ABI `libptiff_c` is produced by the Rust core
(`crates/ptiff-c`), **not** by the deleted `bindings/c/` CMake build. cbindgen
generates the single header `target/ptiff_c.h`, and Cargo builds the lib into
`target/release/`. The SWIG `Makefile` reflects this:

- `C_ABI_DIR := ../../target` (header dir for `-I`)
- `C_LIB_DIR := ../../target/release` (lib dir for `-L` / `-l` / `-Wl,-rpath`)
- a `staticlib:` target that runs `cargo build -p ptiff-c --release` if the lib
  is missing from `target/release`
- `cgo_flags.go` is generated with direct `-I`/`-L`/`-Wl,-rpath` pointing at
  `target/release` (no pkg-config, no CMake)

So the old `PTIFF_C_LIB_DIR=$PWD/../../build/bindings/c` override is gone;
there are no `build/bindings/c` output paths anymore.

## What changed

* Python is now a package rooted at `bindings/python/src/ptiff/` (lib = `ptiff/`
  dir + the SWIG low-level `ptiff.py` + `_ptiff.so`), not a flat
  `bindings/python/src/`.
* SWIG must therefore emit `ptiff.py`/`ptiff_wrap.c`/`_ptiff.so` **into
  `.. /python/src/ptiff/`** instead of `../python/src/`.
* The system `python3` is 3.14, which the SWIG shadow-class loader segfaults
  on. Use CPython 3.13 (via the project's `uv` venv) for both the include dir
  and the runtime test command.

## Step 1 — fix `PY_DIR` in `bindings/swig/Makefile`

In the `# Per-language output dirs.` block, change:

```make
PY_DIR  := ../python/src
```

to:

```make
PY_DIR  := ../python/src/ptiff
```

(The `python` target's `ptiff.py`/`ptiff_wrap.c`/`_ptiff.so` rules already use
`$(PY_DIR)`, so that single variable drives all three outputs into the package
dir.)

## Step 2 — build with Python 3.13

The `PYINC` variable is derived from `$(PYTHON)`, and `PYTHON ?= python3` is
3.14 on this machine. Override it with the 3.13 interpreter from the Python
project's venv, and make sure the Rust-built C ABI is present:

```bash
cd bindings/swig
make staticlib   # builds target/release/libptiff_c.* via
                 #   cargo build -p ptiff-c --release   (if missing)
PYTHON="uv run --project ../python -p 3.13 python" \
make python
```

* `make staticlib` (or `cargo build -p ptiff-c --release` from the repo root)
  builds `libptiff_c` into `target/release/`, which the SWIG Makefile links
  against by default (`LIB_DIR := ../../target/release`).
* No `PTIFF_C_LIB_DIR` override is needed (and no `PKG_CONFIG_PATH`) — the
  Makefile points straight at `target/release`; `PTIFF_C_LIB_DIR` is only there
  as an escape hatch for pointing at a different directory that contains
  `libptiff_c.{dylib,so,a}`.

After a successful build you should see the package dir populated:

```
bindings/python/src/ptiff/ptiff.py
bindings/python/src/ptiff/ptiff_wrap.c
bindings/python/src/ptiff/_ptiff.so
```

## Step 3 — confirm it loads on 3.13 (not 3.14)

```bash
cd bindings/python
uv run --project . -p 3.13 python - <<'EOF'
import ptiff
print(ptiff.__file__)                 # .../src/ptiff/__init__.py
print(ptiff.ptiff_backend_names())    # "isis, memory, ..."
m = ptiff.Metadata("../../scripts/samples/ptiff_interop_fixture.tif")
print(m.field("ptiff.camera.model"))  # "pinhole"
EOF
```

`__file__` must point at the `src/ptiff/__init__.py` package (not a stale flat
`ptiff.py`).

## Step 4 — run the Python suite on 3.13

```bash
cd bindings/python
uv run --project . -p 3.13 python -m pytest test/ -q
# or, without pytest: uv run --project . -p 3.13 python -m unittest discover -s test
```

Expect 8 tests green (camera, image, metadata, logger, version, backend,
error, sink).

## Regression check across the whole pipeline

The Go/Ruby outputs were already regenerated (with the new metadata
surface) and their native files rebuild themselves from `ptiff.i` via the same
Makefile; they are unaffected by the `PY_DIR` change (it only touches `$(PY_DIR)`).
(Octave is no longer a SWIG target — build the MEX binding separately via
`make -C ../octave/mex build`; see `bindings/octave/README.md`.) Re-run to be
safe (each target's `staticlib` dependency ensures the Rust-built
lib in `target/release/` is present):

```bash
make -C ../swig go
make -C ../swig ruby
```
