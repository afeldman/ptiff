//! Read a real ESA/NASA Earth-observation GeoTIFF downloaded by
//! `examples/fetch_esa_nasa_data.py` (network access; large files).
//!
//! `Tiff::open` reads any standard TIFF/BigTIFF — including the real-world
//! granules from ESA/NASA — as a [`Scene`]; this example walks each image and
//! prints its dimensions and pixel type, mirroring what the C++
//! `read_esa_nasa_tiff.cpp` reported.
//!
//! ```sh
//! python3 examples/fetch_esa_nasa_data.py        # downloads into examples/data/
//! cargo run -p ptiff --example read_esa_nasa_tiff -- examples/data/<granule>.tif
//! ```

use ptiff::{Image, Tiff};

fn main() -> ptiff::Result<()> {
    let path = std::env::args()
        .nth(1)
        .expect("usage: read_esa_nasa_tiff <granule.tif>");
    let tiff = Tiff::open(&path)?;
    println!("{path}: {} image(s)", tiff.images().count());
    for image in tiff.images() {
        print_image(image);
    }
    Ok(())
}

fn print_image(image: &Image) {
    println!(
        "  {}x{} px, {} channel(s), {} byte/sample",
        image.width(),
        image.height(),
        image.channel_count(),
        image.pixel_type().bytes_per_sample()
    );
}
