//! Pixel-data-level write source over one image.
//!
//! Mirrors `ptiff::io::TileProvider` (see
//! `libptiff/include/ptiff/io/tile_provider.hpp`).

use crate::tile::{Tile, TileIndex, TileLayout};
use crate::Result;

/// Pixel-data-level write source over one image.
///
/// A `TileProvider` knows how to produce the raw pixel bytes for one
/// strip/tile of the image being written, but is metadata-free and stateless
/// from the `StorageBackend`'s perspective: the `Writer` derives the
/// authoritative tiling from the backend's `ImageSink::layout()`, then asks the
/// provider for each tile in turn and hands it to `ImageSink::write_tile`.
///
/// **Metadata lives in the Scene, not the provider.** The image's geometry and
/// storage metadata come from the `Scene` being written. The provider supplies
/// only pixels: raw, uncompressed, row-major tile bytes. The caller is
/// responsible for producing tile data whose byte count matches the target
/// layout.
///
/// **Zero-copy by design.** Tiles a provider returns are **non-owning** views.
/// The `Writer` consumes each tile synchronously — `provide_tile` is immediately
/// followed by `ImageSink::write_tile` — so a provider's pixel buffer only needs
/// to live until that single `write_tile` completes. The provider may reuse or
/// overwrite its buffer for the next tile.
pub trait TileProvider {
    /// Returns the tiling this provider can supply tiles for.
    ///
    /// Must agree with the layout the `StorageBackend` derives from the `Scene`
    /// being written; the `Writer` validates agreement before writing any
    /// pixels.
    fn layout(&self) -> &TileLayout;

    /// Returns the raw pixel bytes for one strip/tile.
    ///
    /// # Errors
    ///
    /// Returns [`crate::ErrorCode::OutOfRange`] if `index` lies outside this
    /// provider's layout.
    ///
    /// The returned tile's `data()` is a non-owning view valid only until the
    /// writer's next `write_tile` (see the zero-copy note above).
    fn provide_tile(&mut self, index: TileIndex) -> Result<Tile<'_>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::TileId;
    use crate::tile::TileExtent;

    // A simple concrete provider over one caller-owned buffer, as in the C++
    // doc example.
    struct SimpleProvider<'a> {
        layout: TileLayout,
        bytes: &'a [u8],
        next_id: u64,
    }

    impl TileProvider for SimpleProvider<'_> {
        fn layout(&self) -> &TileLayout {
            &self.layout
        }

        fn provide_tile(&mut self, index: TileIndex) -> Result<Tile<'_>> {
            let region = self.layout.region_for(index)?;
            self.next_id += 1;
            Ok(Tile::new(
                TileId::new(self.next_id),
                index,
                region,
                self.bytes,
            ))
        }
    }

    #[test]
    fn provider_serves_tiles_over_its_layout() {
        let layout = TileLayout::new(TileExtent::new(16, 16), 64, 32, 1);
        let bytes = vec![0x41u8; 256];
        let mut provider = SimpleProvider {
            layout,
            bytes: &bytes,
            next_id: 0,
        };

        let tile = provider.provide_tile(TileIndex::new(1, 1, 0)).unwrap();
        assert_eq!(tile.id().value(), 1);
        assert_eq!(tile.region().x, 16);
        assert_eq!(tile.region().y, 16);
        assert_eq!(tile.data_len(), 256);

        // Out-of-grid index is an error.
        assert!(provider.provide_tile(TileIndex::new(99, 0, 0)).is_err());
    }
}
