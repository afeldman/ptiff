//! Copy a PTIFF pixel-for-pixel, preserving metadata and camera calibration.
//!
//! Mirrors `examples/copy_local_sample.cpp` using the idiomatic `ptiff` crate:
//! opens the source, reads each image's metadata + pixels, rebuilds the scene
//! (so the camera/CRS extension domains are preserved) and writes a new PTIFF.
//!
//! Run:
//! ```sh
//! cargo run -p ptiff --example copy_local_sample -- <src.tif> <dst.tif>
//! ```

use ptiff::{ImageDescriptorBuilder, Scene, Tiff, TileInfo};

fn main() -> ptiff::Result<()> {
    let mut args = std::env::args().skip(1);
    let src = args
        .next()
        .expect("usage: copy_local_sample <src.tif> <dst.tif>");
    let dst = args
        .next()
        .expect("usage: copy_local_sample <src.tif> <dst.tif>");

    let tiff = Tiff::open(&src)?;
    let mut scene = Scene::new();
    let mut rasters: Vec<Vec<u8>> = Vec::new();

    for image in tiff.images() {
        let mut desc = ImageDescriptorBuilder::new(image.width(), image.height())
            .pixel_type(image.pixel_type())
            .channel_count(image.channel_count())
            .ground_sample_distance_meters(image.ground_sample_distance_meters())
            .camera(image.camera().cloned())
            .crs(image.crs().cloned());
        if let Some(t) = image.tile_info() {
            desc = desc.tile_info(Some(TileInfo::new(t.tile_width, t.tile_height)));
        }
        scene.add_image(desc.build()).expect("add image");
    }

    // Read every image's pixels, then write the whole scene in one pass so
    // multi-image PTIFFs round-trip correctly.
    for i in 0..scene.image_count() {
        rasters.push(tiff.read_image_pixels(i)?);
    }
    let raster_refs: Vec<&[u8]> = rasters.iter().map(|r| r.as_slice()).collect();
    let bytes = Tiff::to_bytes_with_pixels(&scene, &raster_refs)?;
    std::fs::write(&dst, &bytes)
        .map_err(|e| ptiff::Error::invalid_argument(format!("{dst}: {e}")))?;

    println!("copied {src} -> {dst}");
    Ok(())
}
