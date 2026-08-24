//! Write a small PTIFF fixture, with or without the PTIFF private-tag metadata.
//!
//! Mirrors the old C++ `ptiff_example_write_sample` used by the paper's
//! interop/overhead study: it writes a 64×64 UInt8, tiled 8×8 TIFF and, in
//! "with-tags" mode, embeds the PTIFF extension fields (camera + CRS), which
//! are persisted as the private TIFF tags 65002/65003. Standard TIFF readers
//! must still open the primary image whether or not those tags are present.
//!
//! Run:
//! ```sh
//! cargo run -p ptiff --example write_ptiff_fixture -- <path> with-tags
//! cargo run -p ptiff --example write_ptiff_fixture -- <path> plain
//! ```

use ptiff::{Camera, Extrinsics, ImageDescriptorBuilder, Intrinsics, Scene, Tiff, TileInfo, Vec3};

const WIDTH: u32 = 64;
const HEIGHT: u32 = 64;
const TILE: u32 = 16;

fn main() -> ptiff::Result<()> {
    let path = std::env::args()
        .nth(1)
        .expect("usage: write_ptiff_fixture <path> <with-tags|plain>");
    let mode = std::env::args()
        .nth(2)
        .expect("usage: write_ptiff_fixture <path> <with-tags|plain>");

    let mut scene = Scene::new();
    let mut builder = ImageDescriptorBuilder::new(WIDTH, HEIGHT)
        .pixel_type(ptiff::PixelType::UInt8)
        .channel_count(1)
        .tile_info(Some(TileInfo::new(TILE, TILE)));
    if mode == "with-tags" {
        // Embed the PTIFF camera + CRS extension domains (private tags).
        let camera = Camera::from_model(
            "pinhole",
            Intrinsics::new(700.0, 700.0, 32.0, 32.0),
            Extrinsics::new(ptiff::Quaternion::IDENTITY, Vec3::new(0.0, 0.0, 500.0)),
            "2026-08-24T00:00:00Z",
        );
        builder = builder
            .camera(Some(camera))
            .ground_sample_distance_meters(Some(1.0));
    }
    scene.add_image(builder.build()).expect("add image");

    // Fill the tiled raster (tile-row-major, then tile-column, tiles row-major).
    let columns = WIDTH / TILE;
    let rows = HEIGHT / TILE;
    let mut raster = Vec::with_capacity((WIDTH * HEIGHT) as usize);
    for row in 0..rows {
        for col in 0..columns {
            for y in 0..TILE {
                for x in 0..TILE {
                    raster.push(((col * TILE + x + row * TILE + y) as u8).wrapping_mul(1));
                }
            }
        }
    }

    let bytes = Tiff::to_bytes_with_pixels(&scene, &[raster.as_slice()])?;
    std::fs::write(&path, &bytes)
        .map_err(|e| ptiff::Error::invalid_argument(format!("{path}: {e}")))?;
    println!("wrote {path} (mode={mode}, {WIDTH}x{HEIGHT})");
    Ok(())
}
