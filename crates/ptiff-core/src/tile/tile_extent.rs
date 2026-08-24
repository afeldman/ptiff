//! Pixel dimensions of a tile (or of the rectangle a tile covers).
//!
//! Mirrors `ptiff::io::tile::TileExtent` (see
//! `libptiff/include/ptiff/io/tile/tile_extent.hpp`).

/// Pixel dimensions of a tile (or, reused inside [`crate::tile::TileRegion`],
/// of the rectangle a tile covers).
///
/// This is a plain, cheap value type carrying a pixel width and height. A tile
/// dimension of zero meaningfully flags "untiled" in some consumers (e.g.
/// [`crate::tile::TileLayout`] treats a zero width or height as "no tiling").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TileExtent {
    /// The extent's width in pixels.
    pub width: u32,
    /// The extent's height in pixels.
    pub height: u32,
}

impl TileExtent {
    /// Builds a new extent.
    #[must_use]
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    /// Whether this extent represents "no tiling".
    #[must_use]
    pub const fn is_untiled(self) -> bool {
        self.width == 0 || self.height == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_and_fields() {
        let t = TileExtent::new(16, 32);
        assert_eq!(t.width, 16);
        assert_eq!(t.height, 32);
    }

    #[test]
    fn equality_is_structural() {
        assert_eq!(TileExtent::new(16, 16), TileExtent::new(16, 16));
        assert_ne!(TileExtent::new(16, 16), TileExtent::new(16, 32));
    }

    #[test]
    fn untiled_sentinel() {
        assert!(TileExtent::new(0, 16).is_untiled());
        assert!(TileExtent::new(16, 0).is_untiled());
        assert!(!TileExtent::new(16, 16).is_untiled());
        // Default construction ("no tiling" sentinel).
        assert!(TileExtent::default().is_untiled());
    }
}
