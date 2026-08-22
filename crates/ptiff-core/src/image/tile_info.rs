//! Optional tiling layout hint carried on an image descriptor.
//!
//! Mirrors `ptiff::TileInfo` (see `libptiff/include/ptiff/image/tile_info.hpp`).

/// Optional tiling layout hint in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TileInfo {
    /// Tile width in pixels.
    pub tile_width: u32,
    /// Tile height in pixels.
    pub tile_height: u32,
}

impl TileInfo {
    /// Builds a new tile hint.
    #[must_use]
    pub const fn new(tile_width: u32, tile_height: u32) -> Self {
        Self {
            tile_width,
            tile_height,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_and_fields() {
        let t = TileInfo::new(256, 256);
        assert_eq!(t.tile_width, 256);
        assert_eq!(t.tile_height, 256);
    }

    #[test]
    fn equality_is_structural() {
        assert_eq!(TileInfo::new(16, 16), TileInfo::new(16, 16));
        assert_ne!(TileInfo::new(16, 16), TileInfo::new(16, 32));
    }
}
