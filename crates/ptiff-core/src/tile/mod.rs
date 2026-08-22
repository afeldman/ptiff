//! Tiling grid and tile-mapping types.
//!
//! Mirrors the C++ `ptiff::io::tile` namespace.

pub mod tile_extent;
pub mod tile_index;
pub mod tile_layout;
pub mod tile_region;

pub use tile_extent::TileExtent;
pub use tile_index::TileIndex;
pub use tile_layout::TileLayout;
pub use tile_region::TileRegion;
