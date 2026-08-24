# Octave bindings for ptiff

A [SWIG](https://www.swig.org/)-generated GNU Octave binding over the
language-agnostic C ABI (`libptiff_c`) produced by the Rust crate
[`crates/ptiff-c`](../../crates/ptiff-c) (`cargo build -p ptiff-c --release`
→ `target/release/libptiff_c.*` + `target/ptiff_c.h`). The raw SWIG module is
built as a single `ptiff.oct` loadable module in `lib/` (the
`make -C ../swig octave` "promoted" output dir) -- the dependency-free `test/`
suite is the only versioned source here.

Requires **GNU Octave >= 8** (the SWIG-Octave runtime uses the
`octave::interpreter` API) and, to build, `mkoctfile` (ships with Octave).

## Build

First build + install the C ABI library (see `crates/ptiff-c`), then, with a
pkg-config prefix that ships `libptiff_c.pc` on `PKG_CONFIG_PATH`:

```bash
make -C ../swig octave            # builds octave/lib/ptiff.oct
```

or point `PTIFF_C_LIB_DIR` straight at the Rust build output:

```bash
PTIFF_C_LIB_DIR=$PWD/../../target/release make -C ../swig octave
```

The SWIG Makefile links against `target/release` by default, so for in-tree dev
you only need to build the Rust lib once from the repo root:

```bash
cargo build -p ptiff-c --release
make -C ../swig octave
```

`ptiff.oct` embeds an rpath to the `libptiff_c` dylib so no
`DYLD_LIBRARY_PATH`/`LD_LIBRARY_PATH` is needed at runtime.

## Test

```bash
octave --no-gui --eval "addpath('lib','test'); run_tests_octave();"
```

or as part of the full suite: `make -C ../swig test`.

## Use

```octave
ptiff;                                  % first call loads the module

v = ptiff_runtime_version();            % returns a ptiff_version struct
v.major                                 % => 0

% Struct/object creation is SWIG's new_/delete_ + member syntax:
d = new_ptiff_image_descriptor();
d.width = 10; d.height = 20;
d.pixel_type = ptiff.PTIFF_PIXEL_UINT16;   % constants read as ptiff.<CONST>
d.channel_count = 3;

img = ptiff_image_create(d);
ptiff_image_width(img)                  % => 10
ptiff_image_destroy(img);
delete_ptiff_image_descriptor(d);
```

Out-params and tile buffers follow Octave idioms:

```octave
[r1, r2, r3]   = ptiff_runtime_version_out();    % 3 int out-params
[ok, gsd]      = ptiff_image_gsd(img);           % int ret + double* OUT
[ok, comp]     = ptiff_image_compression(img);   % int ret + int* OUT
[rc, ~]        = ptiff_source_open("a.tif");     % handle + int* err_out

% sink/source tile buffers collapse to uint8 arrays; read data comes BACK as
% an extra uint8 return value (Octave arrays are copy-on-assign):
rc = ptiff_sink_write_tile(sink, 0, 0, uint8(pixels));
[rc, data, nread] = ptiff_source_read_tile(src, 0, 0, uint8(zeros(1, bs)));
```

## Idiomatic classdef layer

The binding also ships idiomatic `classdef` wrapper classes in `lib/`
(`Image`, `Camera`, `Tile`, `Logger`, `Metadata`) over the raw SWIG `.oct`
module — the same structure as the Python `src/ptiff/` package. Prefer these
when you want an object layer; the raw module remains available for
low-level access. Requires loading the module first (`ptiff()`).

```octave
ptiff;                                      % load the SWIG module once

% Read
img = Image.open('a.tif');
tile = img.read_tile(0, 0);
img.close();

% Write (with an optional persisted camera)
cam = Camera('focal_length_x', 700.0, 'principal_x', 64.0);
img = Image.create('out.tif', 32, 32, 'tile_width', 16, 'tile_height', 16, 'camera', cam);
img.write_tile(0, 0, uint8(zeros(1, img.tile_byte_size())));
img.close();

% Metadata (no live handle)
m = Metadata('a.tif');
m.field('ptiff.camera.model')               % => 'pinhole'

% Logger
l = Logger();
l.set_level(l.ERROR);
l.error('boom');
```

## Covered API

Version (struct + out-param), error-code / pixel-type / compression enums,
logging, the backend registry, the `Image` domain type plus its descriptor /
value types, and tile read/write (`ptiff_sink_*` / `ptiff_source_*`) -- see
`test/` for the full ported surface.

## Notes

- The module entry point `ptiff` is a one-shot loader: calling `ptiff()` a
  second time (after it has become the SWIG namespace object) errors. The
  test runner calls it once; each `test_*.m` guards the call with
  `exist('ptiff','var')`.
- SWIG-Octave registers constants both as members of the `ptiff` namespace
  object (available in the base workspace as `ptiff.<CONST>`) and as Octave
  globals. Octave functions can't dereference `ptiff.<CONST>`, so tests read
  them through `ptiff_constants()` (a small `global`-backed helper).
- `ptiff_backend_names()` returns an Octave-owned copy of the C string; do
  **not** call `ptiff_free_string()` on it (same abort bug as the Python/Ruby
  ports -- see `bindings/swig/README.md`).
