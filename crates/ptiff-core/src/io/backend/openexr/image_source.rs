//! Pixel-data-level read for the OpenEXR backend.
//!
//! Mirrors `ptiff::io::backend::openexr::OpenExrImageSource` (see
//! `libptiff/src/io/backend/openexr/openexr_image_source.cpp`): the whole image
//! is always one tile, so the first `read_tile` decodes the full `.exr` into a
//! cached, interleaved, native-endian byte buffer that every subsequent
//! `read_tile` returns.

use crate::id::TileId;
use crate::io::BinaryReader;
use crate::tile::{Tile, TileExtent, TileIndex, TileLayout};
use crate::Result;

use super::document::{decode_image, OpenExrImageInfo};

/// A tile-based reader over one OpenEXR image (single whole-image tile).
pub struct OpenExrImageSource<'a> {
    /// The borrowed byte transport.
    reader: &'a mut dyn BinaryReader,
    /// The decoded image geometry.
    info: OpenExrImageInfo,
    /// The single-tile layout.
    layout: TileLayout,
    /// Cached decoded interleaved pixel bytes (filled on first `read_tile`).
    loaded: Option<Vec<u8>>,
}

impl<'a> OpenExrImageSource<'a> {
    /// Builds a source over `reader` for the document described by `info`.
    pub fn new(reader: &'a mut dyn BinaryReader, info: OpenExrImageInfo) -> Self {
        let layout = TileLayout::new(
            TileExtent::new(info.width, info.height),
            info.width,
            info.height,
            1,
        );
        Self {
            reader,
            info,
            layout,
            loaded: None,
        }
    }

    /// Decodes the whole image once, caching the resulting bytes.
    fn ensure_loaded(&mut self) -> Result<()> {
        if self.loaded.is_some() {
            return Ok(());
        }
        let bytes = decode_image(self.reader, &self.info)?;
        self.loaded = Some(bytes);
        Ok(())
    }
}

impl crate::io::ImageSource for OpenExrImageSource<'_> {
    fn layout(&self) -> &TileLayout {
        &self.layout
    }

    fn read_tile(&mut self, index: TileIndex) -> Result<Tile<'_>> {
        let region = self.layout.region_for(index)?;
        self.ensure_loaded()?;
        let data = self.loaded.as_deref().expect("loaded above");
        Ok(Tile::new(TileId::new(0), index, region, data))
    }
}
