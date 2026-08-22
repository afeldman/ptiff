//! The pixel-space rectangle one tile covers within its level.
//!
//! Mirrors `ptiff::io::tile::TileRegion` (see
//! `libptiff/include/ptiff/io/tile/tile_region.hpp`).

use crate::tile::TileExtent;

/// The pixel-space rectangle one tile covers within its level.
///
/// A thin value type combining a top-left corner (`x`, `y`), expressed in image
/// pixels at the tile's level, with a pixel [`TileExtent`]. It is the concrete,
/// geometry-resolved output of [`crate::tile::TileLayout::region_for`].
///
/// **Edge tiles:** the `extent` of an edge tile is always the **full** tile
/// size, even when the tile hangs over the right or bottom edge of the image
/// (that is, `x + extent.width` may exceed the image width). Consumers treat
/// overhanging pixels as padding/clipped, matching how storage layers pad edge
/// tiles out to the full tile size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TileRegion {
    /// The rectangle's left edge, in image pixels at its level.
    pub x: u32,
    /// The rectangle's top edge, in image pixels at its level.
    pub y: u32,
    /// The rectangle's pixel extent (width and height).
    pub extent: TileExtent,
}

impl TileRegion {
    /// Builds a new region.
    #[must_use]
    pub const fn new(x: u32, y: u32, extent: TileExtent) -> Self {
        Self { x, y, extent }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_and_fields() {
        let extent = TileExtent::new(16, 16);
        let r = TileRegion::new(32, 16, extent);
        assert_eq!(r.x, 32);
        assert_eq!(r.y, 16);
        assert_eq!(r.extent, extent);
    }

    #[test]
    fn edge_tile_may_exceed_image() {
        // A tile starting at x=48 with width 16 reaches x=64, which may exceed
        // a narrower image; the caller decides how to clip.
        let edge = TileRegion::new(48, 0, TileExtent::new(16, 16));
        assert_eq!(edge.x + edge.extent.width, 64);
    }
}
