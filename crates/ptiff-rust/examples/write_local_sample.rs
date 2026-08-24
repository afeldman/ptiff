//! Write a small synthetic PTIFF (128×128 UInt8, tiled 32×32) with a pinhole
//! camera calibration.
//!
//! This mirrors `examples/write_local_sample.cpp` in the idiomatic `ptiff`
//! crate. It builds a [`Scene`] with one image, fills the tiled pixel raster,
//! and writes a valid PTIFF document that any standard TIFF reader can open.
//!
//! Run (from the crate or workspace root):
//! ```sh
//! cargo run -p ptiff --example write_local_sample -- <output-path.tif>
//! ```

use ptiff::{
    Camera, Error, Extrinsics, ImageDescriptorBuilder, Intrinsics, PixelType, Quaternion, Scene,
    Tiff, TileInfo, Vec3,
};

const SIZE: u32 = 128;
const TILE: u32 = 32;

fn tile_pixels(col: u32, row: u32) -> Vec<u8> {
    let mut pixels = Vec::with_capacity((TILE * TILE) as usize);
    for y in 0..TILE {
        for x in 0..TILE {
            let gx = col * TILE + x;
            let gy = row * TILE + y;
            // A simple checkerboard gradient so tiles are visually distinct.
            let v = (((gx + gy) * 8) % 256) as u8;
            pixels.push(v);
        }
    }
    pixels
}

fn main() -> ptiff::Result<()> {
    let out = std::env::args()
        .nth(1)
        .expect("usage: write_local_sample <output-path.tif>");

    // One 128×128 UInt8 image, tiled 32×32, with a pinhole camera.
    let mut scene = Scene::new();
    let camera = Camera::from_model(
        "pinhole",
        Intrinsics::new(500.0, 500.0, 64.0, 64.0),
        Extrinsics::new(Quaternion::IDENTITY, Vec3::new(0.0, 0.0, 1000.0)),
        "2026-08-24T00:00:00Z",
    );
    let descriptor = ImageDescriptorBuilder::new(SIZE, SIZE)
        .pixel_type(PixelType::UInt8)
        .channel_count(1)
        .ground_sample_distance_meters(Some(1.0))
        .tile_info(Some(TileInfo::new(TILE, TILE)))
        .camera(Some(camera))
        .build();
    scene.add_image(descriptor).expect("add image");

    // The raster is expected in read_image_pixels order: tile-row-major, then
    // tile-column, with each tile's pixels row-major inside. Concatenate the
    // tiles in that exact layout.
    let columns = SIZE / TILE;
    let rows = SIZE / TILE;
    let mut raster = Vec::with_capacity((SIZE * SIZE) as usize);
    for row in 0..rows {
        for col in 0..columns {
            raster.extend_from_slice(&tile_pixels(col, row));
        }
    }

    let bytes = Tiff::to_bytes_with_pixels(&scene, &[raster.as_slice()])?;
    std::fs::write(&out, &bytes).map_err(|e| Error::invalid_argument(format!("{out}: {e}")))?;
    println!("wrote {out} ({SIZE}x{SIZE}, {columns}x{rows} tiles, pinhole camera)");
    Ok(())
}
