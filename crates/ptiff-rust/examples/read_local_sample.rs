//! Open a PTIFF and print its metadata, camera calibration and first tile.
//!
//! Mirrors `examples/read_local_sample.cpp` using the idiomatic `ptiff` crate:
//! it reads the on-disk metadata + camera/CRS extensions and the first tile of
//! pixel data — the same surface a binding (Python/Rust/Go) or CLI tool uses.
//!
//! Run:
//! ```sh
//! cargo run -p ptiff --example read_local_sample -- <path-to-ptiff.tif>
//! ```

use ptiff::{Image, PixelType, Tiff};

fn main() -> ptiff::Result<()> {
    let path = std::env::args()
        .nth(1)
        .expect("usage: read_local_sample <path-to-ptiff.tif>");
    let tiff = Tiff::open(&path)?;
    println!("{}: {} image(s)", path, tiff.images().count());

    for (i, image) in tiff.images().enumerate() {
        print_image(i, image);
    }
    Ok(())
}

fn print_image(i: usize, image: &Image) {
    println!("  image[{i}]: {}x{} px", image.width(), image.height());
    println!("    pixel type:  {}", pixel_name(image.pixel_type()));
    println!("    channels:    {}", image.channel_count());
    println!("    tiling:      {:?}", image.tile_info());
    println!("    compression: {:?}", image.compression());
    print_camera(image);
}

fn print_camera(image: &Image) {
    println!("    camera: {:?}", image.camera().map(|c| c.model_name()));
}

fn pixel_name(p: PixelType) -> &'static str {
    match p {
        PixelType::UInt8 => "UInt8",
        PixelType::UInt16 => "UInt16",
        PixelType::UInt32 => "UInt32",
        PixelType::Float32 => "Float32",
        PixelType::Float64 => "Float64",
        _ => "other",
    }
}
