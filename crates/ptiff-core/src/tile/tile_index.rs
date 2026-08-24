//! Position of one tile within its image's tiling grid.
//!
//! Mirrors `ptiff::io::tile::TileIndex` (see
//! `libptiff/include/ptiff/io/tile/tile_index.hpp`).

/// Position of one tile within its image's tiling grid.
///
/// A [`TileIndex`] identifies exactly one tile of a [`crate::tile::TileLayout`]
/// through its `column` and `row` within a given pyramid `level`. Tiling is
/// iterated in row-major order: the column advances fastest, which matches how
/// tile byte ranges are laid out back-to-back in file formats such as TIFF.
///
/// `level` is 0 for the base resolution and increases for coarser
/// pyramid/mip levels a backend may expose later. A tile's level is always
/// strictly smaller than the layout's `level_count`.
///
/// A [`TileIndex`] is validated against a layout by [`crate::tile::TileLayout`]'s
/// query methods (`region_for`, `index_for`); the index itself performs no
/// validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TileIndex {
    /// Tile column index (horizontal position) within the level's grid.
    pub column: u32,
    /// Tile row index (vertical position) within the level's grid.
    pub row: u32,
    /// Pyramid level; 0 = base resolution.
    pub level: u32,
}

impl TileIndex {
    /// Builds a new index.
    #[must_use]
    pub const fn new(column: u32, row: u32, level: u32) -> Self {
        Self { column, row, level }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_and_fields() {
        let idx = TileIndex::new(2, 1, 0);
        assert_eq!(idx.column, 2);
        assert_eq!(idx.row, 1);
        assert_eq!(idx.level, 0);
    }

    #[test]
    fn equality_is_structural() {
        assert_eq!(TileIndex::new(2, 1, 0), TileIndex::new(2, 1, 0));
        assert_ne!(TileIndex::new(2, 1, 0), TileIndex::new(1, 1, 0));
        assert_ne!(TileIndex::new(2, 1, 0), TileIndex::new(2, 1, 1));
    }
}
