# Rebuilding the Python binding for the new package layout

The SWIG `bindings/swig/Makefile` regenerates the promoted per-language outputs
from `ptiff.i`. With the Python package restructure, the Python target needs
one path fix and a safe Python interpreter before you `make python`.

> These steps must be run by hand: the working `bindings/swig/Makefile` is
> excluded from the assistant environment (`.gooseignore`), so it cannot be
> edited or regenerated here.

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
project's venv:

```bash
cd bindings/swig
PYTHON="uv run --project ../python -p 3.13 python" \
PTIFF_C_LIB_DIR=$PWD/../../build/bindings/c \
make python
```

* `PTIFF_C_LIB_DIR` points at the checked-in C build output (`build/bindings/c`
  contains `libptiff_c.pc`), so SWIG links against the local lib rather than a
  globally installed one.
* Alternatively export `PKG_CONFIG_PATH=/abs/path/to/install-shared/lib/pkgconfig`
  and drop `PTIFF_C_LIB_DIR`.

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

The Go/Ruby/Octave outputs were already regenerated (with the new metadata
surface) and their native files rebuild themselves from `ptiff.i` via the same
Makefile; they are unaffected by the `PY_DIR` change (it only touches `$(PY_DIR)`).
Re-run to be safe:

```bash
PTIFF_C_LIB_DIR=$PWD/../../build/bindings/c make -C ../swig go
PTIFF_C_LIB_DIR=$PWD/../../build/bindings/c make -C ../swig ruby
PTIFF_C_LIB_DIR=$PWD/../../build/bindings/c make -C ../swig octave
```
