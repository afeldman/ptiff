//! Pixel-data-level write for the OpenEXR backend.
//!
//! Mirrors `ptiff::io::backend::openexr::OpenExrImageSink` (see
//! `libptiff/src/io/backend/openexr/openexr_image_sink.cpp`): the whole image
//! is always one tile, so a single `write_tile` writes the complete `.exr`
//! document.
//!
//! Because `exr` requires a seekable writer with a deterministic layout, we
//! write in one pass whenever `write_tile` is called with the full image.

use crate::io::BinaryWriter;
use crate::tile::{Tile, TileExtent, TileLayout};
use crate::{Error, Result};

use super::document::{encode_image, expected_pixel_bytes, OpenExrImageInfo};

/// A single-tile writer over one OpenEXR image.
pub struct OpenExrImageSink<'a> {
    /// The borrowed byte transport.
    writer: &'a mut dyn BinaryWriter,
    /// The decoded image geometry.
    info: OpenExrImageInfo,
    /// The single-tile layout.
    layout: TileLayout,
}

impl<'a> OpenExrImageSink<'a> {
    /// Builds a sink over `writer` for the document described by `info`.
    pub fn new(writer: &'a mut dyn BinaryWriter, info: OpenExrImageInfo) -> Self {
        let layout = TileLayout::new(
            TileExtent::new(info.width, info.height),
            info.width,
            info.height,
            1,
        );
        Self {
            writer,
            info,
            layout,
        }
    }
}

impl crate::io::ImageSink for OpenExrImageSink<'_> {
    fn layout(&self) -> &TileLayout {
        &self.layout
    }

    fn write_tile(&mut self, tile: &Tile<'_>) -> Result<()> {
        let region = self.layout.region_for(tile.index())?;
        // Region must cover the whole image (single tile). Verify the data is
        // the full image rather than an edge-padded tile.
        let _ = region;
        let expected = expected_pixel_bytes(&self.info)?;
        if tile.data().len() != expected {
            return Err(Error::invalid_argument(
                "openexr: tile data size does not match image geometry",
            ));
        }
        encode_image(self.writer, &self.info, tile.data())
    }
}
