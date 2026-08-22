# Examples — small local samples with libptiff

This directory contains **runnable examples** that exercise libptiff's
`TiffBackend` — the same public API that the Python / Go / Rust bindings and
tools use. By default they generate and inspect **small synthetic PTIFF files
locally**, so they need no network access, no downloads and no multi-gigabyte
data in the repository.

```
examples/
├── write_local_sample.cpp   # generate a small synthetic PTIFF + camera calibration
├── read_local_sample.cpp    # inspect metadata + camera + first tile
├── copy_local_sample.cpp    # copy a PTIFF, preserving the camera calibration
├── fetch_esa_nasa_data.py   # (optional) downloads real ESA & NASA GeoTIFFs
├── read_esa_nasa_tiff.cpp   # (optional) read the downloaded real granules
└── CMakeLists.txt           # builds all of the above (PTIFF_BUILD_EXAMPLES=ON)
```

## 1. Local synthetic samples (no network, no big data)

The three `*_local_sample` targets are fully self-contained: they create small
in-memory images on the fly (128×128, UInt8, tiled 32×32) and write them to
disk. Nothing is downloaded and nothing large is stored in the repo.

The write example also embeds a **complete pinhole camera calibration** in the
standard `ptiff.camera.*` extension fields — the same fields the Rust / Python
bindings surface as their `Camera` type (model, focal length, principal point,
rotation quaternion, position, timestamp).

### Build

From a configured build tree (the examples are added when
`PTIFF_BUILD_EXAMPLES=ON`):

```bash
cmake -B build -S . -DPTIFF_BUILD_EXAMPLES=ON
cmake --build build --target ptiff_example_write_local_sample \
      ptiff_example_read_local_sample ptiff_example_copy_local_sample
```

### Run

```bash
# 1) generate a small PTIFF with a pinhole camera calibration
./build/examples/ptiff_example_write_local_sample /tmp/sample.tif

# 2) inspect it: file metadata + camera calibration + first tile pixel stats
./build/examples/ptiff_example_read_local_sample /tmp/sample.tif

# 3) copy it pixel-for-pixel, preserving the camera
./build/examples/ptiff_example_copy_local_sample /tmp/sample.tif /tmp/sample_copy.tif
```

Example `read_local_sample` output (abridged):

```
PTIFF local sample reader (libptiff)
  file: /tmp/sample.tif

[file]
    imageWidth = 128
    imageHeight = 128
    pixelType = UInt8
    samplesPerPixel = 1

[camera]
      ptiff.camera.model = pinhole
      ptiff.camera.focal_length_x = 700.0
      ptiff.camera.focal_length_y = 700.0
      ptiff.camera.principal_x = 64.0
      ptiff.camera.principal_y = 64.0
      ...

[pixels]
  first tile bytes: 1024
  tile min: 0  max: 255  sum: 0
```

## 2. Optional: real ESA & NASA data (network, large files)

The downloader and reader for **real** public ESA / NASA raster products are
kept as an optional, opt-in path. They are not required for the local examples
above.

### Why real ESA & NASA data?

libptiff is a planetary-TIFF library, but its `TiffBackend` is a general
TIFF/BigTIFF reader, so Earth-observation GeoTIFFs work out of the box:

| Agency | Product | Format | What it is |
|--------|---------|--------|------------|
| **ESA** | Sentinel-2 L2A “visual” | Cloud-Optimized GeoTIFF | 10 m true-colour RGB surface reflectance, the iconic Copernicus programme product (ESA/EU). |
| **NASA** | ASTER L1T VNIR | Cloud-Optimized GeoTIFF | Orthorectified VNIR reflectance of an **ASTER** granule from **NASA's Terra** satellite. |

These granules are distributed through the <https://planetarycomputer.microsoft.com>
catalog as COG (Cloud-Optimized GeoTIFF) — a tiled BigTIFF, exactly the format
`libptiff` reads out of the box.

> Note: The downloaded files are several MB to tens of MB and are git-ignored
> (`examples/data/`). They are only written to disk locally; nothing is
> committed to the repository.

### Fetch the data

```bash
# (first time only) install the small Python deps
python3 -m pip install planetary-computer pystac-client

# download both the ESA (Sentinel-2) and NASA (ASTER) samples
python3 examples/fetch_esa_nasa_data.py

# or select just one
python3 examples/fetch_esa_nasa_data.py --collection sentinel-2
python3 examples/fetch_esa_nasa_data.py --collection aster
```

If the STAC API or a granule is momentarily unavailable, re-run the script
later — it is safe to re-run (already-downloaded files are skipped unless you
pass `--force`).

### Build & run the NASA/ESA reader

```bash
cmake --build build --target ptiff_example_read_esa_nasa_tiff
./build/examples/ptiff_example_read_esa_nasa_tiff examples/data/sentinel2_l2a_visual_rgb.tif
```

For each file the example prints the real `file size`, the **metadata**
(key/value pairs on the format-neutral `StorageModel`, one block per IFD chain
/ COG pyramid level), the decoded **image layout** (image size, tile size,
grid) and the first few **tiles** of raw pixel data.
