# ptiff-python

PyO3 (maturin) Python binding over the PTIFF 1.0 Rust core. Provides an
idiomatic, NumPy-integrated tile API (Plan §9, Option B): it calls the Rust
core directly — no SWIG/ctypes round-trip through the C ABI.

```python
import ptiff_pyo3 as ptiff
import numpy as np

with ptiff.open("scene.tif") as doc:
    img = doc.image(0)
    print(img.width, img.height, img.pixel_type)
    tile = img.read_tile(column=1, row=0)  # np.ndarray
```

Build the extension module with `maturin build --release` (or
`maturin develop` for an editable install into the active virtualenv).

## Building and testing

The module is a normal PyO3 extension (built with `maturin`). From this
directory:

```bash
# Build a wheel and install it into the active virtualenv
maturin build --release

# Or install editably into the active virtualenv and run the pytest suite
maturin develop --release
python -m pytest python/tests -v
```

The pytest suite exercises the full API surface — version / backend names /
constant ordering, logger round-trip, write→read pixel roundtrip (Source/Sink),
NumPy tile dtype+shape, camera write+read, and read-only metadata.

Rust unit tests (no Python runtime needed) run via
`cargo test -p ptiff-python`.

> **Workspace note:** `ptiff-python` is excluded from the root Cargo workspace
> (root `Cargo.toml` `exclude`) and carries its own `[workspace]` stub + lockfile,
> so a plain `cargo build --workspace` never needs a Python development install.
> The PyO3 `extension-module` link (libpython symbols resolved at import time)
> only applies when `maturin` builds the wheel (see `pyproject.toml`
> `[tool.maturin] features`); `cargo build`/`cargo test` here link libpython via
> the active interpreter.
