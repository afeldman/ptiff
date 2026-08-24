# Examples — small local samples with the `ptiff` crate

The PTIFF library (and its idiomatic Rust crate `ptiff`) reads and writes the
PTIFF planetary-image format. These examples are **Rust programs** that use the
header-only-for-humans `ptiff` crate directly (same public API the CLI and the
language bindings build on). They generate and inspect **small synthetic PTIFF
files locally**, so they need no network access and no multi-gigabyte data in
the repository.

```
examples/
├── ../crates/ptiff-rust/examples/write_local_sample.rs  # small synthetic PTIFF + camera
├── ../crates/ptiff-rust/examples/read_local_sample.rs    # inspect metadata + camera + pixels
├── ../crates/ptiff-rust/examples/copy_local_sample.rs    # copy PTIFF preserving camera
├── fetch_esa_nasa_data.py               # (optional) downloads real ESA & NASA GeoTIFFs
└── ../crates/ptiff-rust/examples/read_esa_nasa_tiff.rs  # (optional) read the downloaded granules
```

The Rust example sources live in `crates/ptiff-rust/examples/` so they build and
run with the standard Cargo workflow:

```bash
cargo run -p ptiff --example write_local_sample -- ./sample.tif     # write
cargo run -p ptiff --example read_local_sample  -- ./sample.tif     # read + camera
cargo run -p ptiff --example copy_local_sample  -- ./sample.tif ./copy.tif
```

`write_local_sample` embeds a **complete pinhole camera calibration** in the
standard `ptiff.camera.*` extension fields — the same fields the bindings and
the `ptiff metadata` subcommand surface as the `Camera` type (model, focal
length, principal point, rotation quaternion, position, timestamp). It writes a
128×128 UInt8 image tiled 32×32.

`read_local_sample` output (abridged):

```
. . .: 1 image(s)
  image[0]: 128x128 px
    pixel type:  UInt8
    channels:    1
    camera: Some("pinhole")
```

## Optional: real ESA & NASA data (network, large files)

The downloader and reader for **real** public ESA / NASA raster products are an
opt-in path, not required for the local examples.

The `ptiff` crate reads any standard TIFF/BigTIFF — including the real
Earth-observation COG (Cloud-Optimized GeoTIFF) granules from ESA (Sentinel-2)
and NASA (ASTER) available from the
<https://planetarycomputer.microsoft.com> catalog.

### Fetch the data

```bash
# (first time only) install the small Python deps
python3 -m pip install planetary-computer pystac-client

python3 examples/fetch_esa_nasa_data.py                    # both samples
python3 examples/fetch_esa_nasa_data.py --collection sentinel-2
python3 examples/fetch_esa_nasa_data.py --collection aster
```

> Note: the downloaded files are several MB to tens of MB and are git-ignored
> (`examples/data/`). They are only written to disk locally; nothing is
> committed to the repository.

### Run the ESA/NASA reader

```bash
cargo run -p ptiff --example read_esa_nasa_tiff -- examples/data/sentinel2_l2a_visual_rgb.tif
```

For each file the example reports the number of images and each image's size,
channel count and sample depth.
