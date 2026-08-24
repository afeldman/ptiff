//! The tiling grid of an image, with tile-mapping queries.
//!
//! Mirrors `ptiff::io::tile::TileLayout` (see
//! `libptiff/include/ptiff/io/tile/tile_layout.hpp`).

use crate::image::ImageDescriptor;
use crate::tile::{TileExtent, TileIndex, TileRegion};
use crate::{Error, Result};

/// The tiling grid of an image.
///
/// Every layout knows its tile dimensions, the total image size at the base
/// resolution, and how many resolution levels exist. Tile coordinates are
/// expressed in row-major order — column is the fastest-moving index — which
/// matches how tile byte ranges are laid out back-to-back in file formats such
/// as TIFF.
///
/// **The "no tiling" sentinel:** a layout whose `tile_size.width` or
/// `tile_size.height` is zero represents "no tiling": [`Self::columns`] /
/// [`Self::rows`] return 0 and the query operations report
/// [`ErrorCode::OutOfRange`]. This is the natural result of default
/// construction and is how non-tiled images are represented internally.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TileLayout {
    /// Tile extent in pixels. A zero width or height indicates "no tiling".
    pub tile_size: TileExtent,
    /// Base-resolution (level 0) image width in pixels.
    pub image_width: u32,
    /// Base-resolution (level 0) image height in pixels.
    pub image_height: u32,
    /// Number of resolution levels in the pyramid (level 0 = base resolution).
    pub level_count: u32,
}

impl Default for TileLayout {
    fn default() -> Self {
        Self {
            tile_size: TileExtent::default(),
            image_width: 0,
            image_height: 0,
            level_count: 1,
        }
    }
}

impl TileLayout {
    /// Builds a new layout.
    #[must_use]
    pub const fn new(
        tile_size: TileExtent,
        image_width: u32,
        image_height: u32,
        level_count: u32,
    ) -> Self {
        Self {
            tile_size,
            image_width,
            image_height,
            level_count,
        }
    }

    /// Whether this layout represents "no tiling".
    #[must_use]
    pub const fn is_untiled(&self) -> bool {
        self.tile_size.is_untiled()
    }

    /// Returns the number of tile columns that cover the image at `level`.
    ///
    /// Level 0 is the base resolution; each higher level `L` down-samples both
    /// dimensions by a factor of `2^L`, matching conventional image-pyramid
    /// downsampling.
    ///
    /// Returns 0 if the layout is non-tiled (`tile_size.width == 0`), matching
    /// the C++ oracle.
    #[must_use]
    pub const fn columns(&self, level: u32) -> u32 {
        if self.tile_size.width == 0 {
            return 0;
        }
        let level_width = self.image_width >> level;
        // Integer division rounded up (`div_ceil`), matching the C++ oracle `(a + b - 1) / b`.
        level_width.div_ceil(self.tile_size.width)
    }

    /// Returns the number of tile rows that cover the image at `level`.
    ///
    /// Returns 0 if the layout is non-tiled (`tile_size.height == 0`), matching
    /// the C++ oracle.
    #[must_use]
    pub const fn rows(&self, level: u32) -> u32 {
        if self.tile_size.height == 0 {
            return 0;
        }
        let level_height = self.image_height >> level;
        level_height.div_ceil(self.tile_size.height)
    }

    /// Returns the pixel rectangle that tile `index` covers within its level.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorCode::OutOfRange`] if `level` is outside this layout's
    /// level count, or if `index` is outside this layout's grid at its level.
    pub fn region_for(&self, index: TileIndex) -> Result<TileRegion> {
        if index.level >= self.level_count {
            return Err(Error::out_of_range(
                "TileLayout::region_for: level out of range",
            ));
        }
        if index.column >= self.columns(index.level) || index.row >= self.rows(index.level) {
            return Err(Error::out_of_range(
                "TileLayout::region_for: index outside grid",
            ));
        }
        Ok(TileRegion::new(
            index.column * self.tile_size.width,
            index.row * self.tile_size.height,
            self.tile_size,
        ))
    }

    /// Maps a pixel coordinate to the index of the tile that contains it.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorCode::OutOfRange`] if `level` is outside this layout's
    /// level count, if `(x, y)` is outside the image at that level, or if the
    /// layout is non-tiled (a tile dimension is zero).
    pub fn index_for(&self, x: u32, y: u32, level: u32) -> Result<TileIndex> {
        if level >= self.level_count {
            return Err(Error::out_of_range(
                "TileLayout::index_for: level out of range",
            ));
        }
        if self.tile_size.width == 0
            || self.tile_size.height == 0
            || x >= (self.image_width >> level)
            || y >= (self.image_height >> level)
        {
            return Err(Error::out_of_range(
                "TileLayout::index_for: pixel outside image",
            ));
        }
        Ok(TileIndex::new(
            x / self.tile_size.width,
            y / self.tile_size.height,
            level,
        ))
    }

    /// Builds a single-level `TileLayout` from an image descriptor.
    ///
    /// The produced layout always has `level_count == 1`. Multi-level (pyramid)
    /// layouts are constructed directly or by higher-level orchestration.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorCode::InvalidArgument`] if `descriptor` has no `tile_info`
    /// (i.e. the image isn't tiled).
    pub fn from_descriptor(descriptor: &ImageDescriptor) -> Result<TileLayout> {
        let Some(tile_info) = descriptor.tile_info else {
            return Err(Error::invalid_argument(
                "TileLayout::from_descriptor: descriptor has no tile_info",
            ));
        };
        Ok(TileLayout::new(
            TileExtent::new(tile_info.tile_width, tile_info.tile_height),
            descriptor.width,
            descriptor.height,
            1,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Matches the C++ doc-examples in tile_layout.hpp / tile_index.hpp.
    fn layout_16() -> TileLayout {
        TileLayout::new(TileExtent::new(16, 16), 64, 32, 1)
    }

    #[test]
    fn columns_and_rows_base_level() {
        let l = layout_16();
        assert_eq!(l.columns(0), 4); // 64 / 16
        assert_eq!(l.rows(0), 2); // 32 / 16
    }

    #[test]
    fn columns_and_rows_ignore_level_when_untiled() {
        // A layout over a 2-level pyramid; level 1 halves both dims.
        let l = TileLayout::new(TileExtent::new(16, 16), 64, 32, 2);
        assert_eq!(l.columns(1), 2); // 32 / 16 (image width >> 1)
        assert_eq!(l.rows(1), 1); // 16 / 16 (image height >> 1)
    }

    #[test]
    fn untiled_layout_has_zero_columns_and_rows() {
        let l = TileLayout::default(); // tile_size = 0x0 sentinel
        assert!(l.is_untiled());
        assert_eq!(l.columns(0), 0);
        assert_eq!(l.rows(0), 0);
    }

    #[test]
    fn region_for_maps_corner() {
        let l = layout_16();
        // Tile at column 1, row 1 → top-left (16, 16).
        let r = l.region_for(TileIndex::new(1, 1, 0)).unwrap();
        assert_eq!(r.x, 16);
        assert_eq!(r.y, 16);
        assert_eq!(r.extent, TileExtent::new(16, 16));
    }

    #[test]
    fn region_for_rejects_outside_grid() {
        let l = layout_16();
        assert!(l.region_for(TileIndex::new(99, 0, 0)).is_err());
        assert!(l.region_for(TileIndex::new(0, 0, 5)).is_err()); // bad level
    }

    #[test]
    fn index_for_maps_pixel_to_tile() {
        let l = layout_16();
        // x=17 → column 1, y=5 → row 0 (matches C++ doc example).
        let i = l.index_for(17, 5, 0).unwrap();
        assert_eq!(i.column, 1);
        assert_eq!(i.row, 0);
        assert_eq!(i.level, 0);

        // x=33 → column 2, y=3 → row 0 (matches C++ doc example).
        let i2 = l.index_for(33, 3, 0).unwrap();
        assert_eq!(i2.column, 2);
        assert_eq!(i2.row, 0);
    }

    #[test]
    fn index_for_rejects_outside_image_and_untiled() {
        let l = layout_16();
        assert!(l.index_for(64, 0, 0).is_err()); // x == image width
        assert!(l.index_for(0, 32, 0).is_err()); // y == image height
        assert!(l.index_for(0, 0, 9).is_err()); // bad level
        assert!(TileLayout::default().index_for(0, 0, 0).is_err()); // untiled
    }

    #[test]
    fn from_descriptor_single_level() {
        let d = ImageDescriptor {
            width: 64,
            height: 32,
            tile_info: Some(crate::image::TileInfo::new(16, 16)),
            ..ImageDescriptor::new(0, 0)
        };
        let l = TileLayout::from_descriptor(&d).unwrap();
        assert_eq!(l.tile_size, TileExtent::new(16, 16));
        assert_eq!(l.image_width, 64);
        assert_eq!(l.image_height, 32);
        assert_eq!(l.level_count, 1);
    }

    #[test]
    fn from_descriptor_rejects_untiled() {
        let d = ImageDescriptor::new(64, 32); // no tile_info
        assert!(TileLayout::from_descriptor(&d).is_err());
    }
}
