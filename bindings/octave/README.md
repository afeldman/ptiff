# Octave bindings for ptiff

A hand-written GNU Octave MEX binding over the language-agnostic C ABI
(`libptiff_c`) produced by the Rust crate
[`crates/ptiff-c`](../../crates/ptiff-c) (`cargo build -p ptiff-c --release`
→ `target/release/libptiff_c.*` + `target/ptiff_c.h`). It is the **only**
Octave binding: the earlier SWIG-generated Octave module was removed in favour
of this C++ adapter (see `PTIFF-1.0-RUST-CORE-PLAN.md` §10). The idiomatic
`.m` API lives in `bindings/octave/mex/` and calls a single native entry point,
`ptiff_octave` (built as a `.oct` shared module), which dispatches to the C ABI
— no SWIG code generation, no classdef layer.

Pipeline:

```
Octave -> ptiff_octave.oct (C++ MEX adapter) -> C ABI (libptiff_c) -> Rust core
```

Requires **GNU Octave >= 8** to run and `mkoctfile` (ships with Octave) to
build.

## Build

Build the C ABI library once from the repo root, then build the `.oct`:

```bash
cargo build -p ptiff-c --release        # or: cargo build -p ptiff-c
make -C bindings/octave/mex build
```

The MEX Makefile links against the Rust `target/release` dir by default
(`PTIFF_C_LIB_DIR` overrides it), and embeds an rpath to `libptiff_c` so no
`DYLD_LIBRARY_PATH`/`LD_LIBRARY_PATH` is needed at runtime.

## Test

```bash
make -C bindings/octave/mex test        # runs run_ptiff_mex_tests.m
```

or directly:

```bash
octave --no-gui --eval "addpath('bindings/octave/mex'); run('bindings/octave/mex/run_ptiff_mex_tests.m');"
```

## Use

The `.m` wrappers in `bindings/octave/mex/` are the idiomatic API. Add that
directory to your Octave path (`addpath('bindings/octave/mex')`) and call the
`ptiff_*` functions directly.

Reading:

```octave
h = ptiff_open('a.tif');              % opaque uint64 source handle
s = ptiff_source_info(h);             % width/height/pixel_type/tile grid
tile = ptiff_read_tile(h, col, row);  % uint8 row vector
ptiff_close(h);                       % release (also auto-closed on clear)
```

Writing (with optional persisted camera):

```octave
c = ptiff_constants_mex();
h = ptiff_create('out.tif', 32, 32, c.PTIFF_PIXEL_UINT8, 1, 16, 16);
si = ptiff_sink_info(h);              % tile_columns/tile_rows/tile_byte_size
ptiff_write_tile(h, col, row, uint8(7 * ones(1, si.tile_byte_size)));
ptiff_sink_close(h);                  % flush; only then is the file valid to read
```

Inspection / metadata / logging:

```octave
d = ptiff_info('a.tif');              % header descriptor (no handle retained)
m = ptiff_metadata('a.tif');          % flattened ptiff.<domain>.<name> keys/values
cam = ptiff_octave('camera', 'a.tif');% camera intrinsics/extrinsics/projection
v = ptiff_version();                  % compile_/runtime_ (major,minor,patch)
names = ptiff_backends();             % registered decoding backends
ptiff_logger_set_level(1);            % 0=trace .. 6=off
ptiff_logger_log(4, 'boom');          % emit a log record
```

## Covered API

Version + ABI break counter, error-code / pixel-type / compression constants
(`ptiff_constants_mex()`), logging, the backend registry, image descriptor
reads (`ptiff_open` + `ptiff_info`), flattened metadata, camera intrinsics /
extrinsics / projection, and tile read/write (`ptiff_{source,sink}_*`). See
`test_ptiff_mex_*.m` in `bindings/octave/mex/` for the full surface covered by
the test suite.

## Notes

- `ptiff_octave` is a single C++ `DEFUN_DLD` entry point; the `.m` wrappers
  are thin labelled clients. Resources are tracked in a process-global
  `uint64 -> Resource` registry and released on `ptiff_close` / `ptiff_sink_close`
  or the `ptiff_octave('clear')` command.
- This binding is built and exercised in CI by `.github/workflows/octave-bindings.yml`
  and measured by `benchmarks/run_benchmarks.sh` (the `octave` case).
