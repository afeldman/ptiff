//! Tiling grid and tile-mapping types, plus the [`Tile`] data unit.
//!
//! Mirrors the C++ `ptiff::io::tile` namespace.

mod tile_extent;
mod tile_index;
mod tile_layout;
mod tile_region;

pub use tile_extent::TileExtent;
pub use tile_index::TileIndex;
pub use tile_layout::TileLayout;
pub use tile_region::TileRegion;

// --- Tile (defined directly in this module to avoid `tile::tile` inception) ---

use crate::id::TileId;

/// One tile's identity, position, and pixel data.
///
/// A `Tile` bundles everything a storage layer needs to reason about a single
/// unit of transfer: a unique [`TileId`], its position in the grid
/// ([`TileIndex`]), the pixel rectangle it covers ([`TileRegion`]), and a slice
/// over the raw pixel bytes.
///
/// **Zero-copy by design:** `data()` is a **non-owning** view (`&[u8]`): the
/// tile itself never allocates or copies pixel bytes. The view is only valid
/// while whatever produced it is still alive — an `ImageSource`, a `TileCache`
/// entry, or a caller-owned buffer. Do not retain a `Tile` (or its `data`)
/// beyond the lifetime of the object that handed it to you.
#[derive(Debug, Clone, Copy)]
pub struct Tile<'a> {
    id: TileId,
    index: TileIndex,
    region: TileRegion,
    data: &'a [u8],
}

impl<'a> Tile<'a> {
    /// Builds a tile from its identity, grid position, covered region and a
    /// non-owning data view.
    ///
    /// No copies are made of `data`; the caller retains ownership and must keep
    /// it alive for at least the lifetime of this `Tile`.
    #[must_use]
    pub const fn new(id: TileId, index: TileIndex, region: TileRegion, data: &'a [u8]) -> Self {
        Self {
            id,
            index,
            region,
            data,
        }
    }

    /// Returns the tile's unique identifier.
    #[must_use]
    pub const fn id(&self) -> TileId {
        self.id
    }

    /// Returns the tile's position within its grid.
    #[must_use]
    pub const fn index(&self) -> TileIndex {
        self.index
    }

    /// Returns the pixel-space rectangle the tile covers.
    #[must_use]
    pub const fn region(&self) -> TileRegion {
        self.region
    }

    /// Returns a non-owning slice over the tile's raw pixel bytes.
    #[must_use]
    pub const fn data(&self) -> &'a [u8] {
        self.data
    }

    /// Returns the number of raw pixel bytes in this tile.
    #[must_use]
    pub const fn data_len(&self) -> usize {
        self.data.len()
    }
}

#[cfg(test)]
mod tests {
    use super::Tile;
    use crate::id::TileId;
    use crate::tile::TileExtent;
    use crate::tile::{TileIndex, TileRegion};

    #[test]
    fn tile_bundles_identity_position_region_and_data() {
        // 16x16 UInt8 "tile" fully inlined into 256 bytes.
        let pixels = [0x41u8; 256];
        let t = Tile::new(
            TileId::new(7),
            TileIndex::new(2, 1, 0),
            TileRegion::new(32, 16, TileExtent::new(16, 16)),
            &pixels,
        );

        assert_eq!(t.id().value(), 7);
        assert_eq!(t.index().column, 2);
        assert_eq!(t.index().row, 1);
        assert_eq!(t.region().extent.width, 16);
        assert_eq!(t.data_len(), 256);
        assert_eq!(t.data()[0], 0x41);
    }

    // Zero-copy: the tile views the same buffer, not a copy.
    #[test]
    fn tile_views_same_buffer_zero_copy() {
        let pixels = [0x41u8; 256];
        let t = Tile::new(
            TileId::new(1),
            TileIndex::default(),
            TileRegion::default(),
            &pixels,
        );
        assert_eq!(t.data().as_ptr(), pixels.as_ptr());
        assert_eq!(t.data_len(), pixels.len());
    }
}
