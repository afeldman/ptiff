//! Pixel-data-level read abstraction over one image.
//!
//! Mirrors `ptiff::io::ImageSource` (see
//! `libptiff/include/ptiff/io/image_source.hpp`).

use crate::tile::{Tile, TileIndex, TileLayout};
use crate::Result;

/// Pixel-data-level read abstraction over one image.
///
/// `ImageSource` lets a caller read raw pixel bytes one
/// [`Tile`] at a time without knowing the target format's on-disk byte layout.
///
/// **Contract**
///
/// - [`layout`](ImageSource::layout) describes how the image is tiled (tile
///   size, grid, pyramid levels).
/// - [`read_tile`](ImageSource::read_tile) returns the raw pixel bytes for one
///   tile; the returned tile's `data()` is a non-owning view valid only until
///   the *next* call on this source (see the zero-copy note on
///   [`crate::io::TileProvider`]).
///
/// **Thread-safety:** not thread-safe by default, since a concrete source
/// typically wraps a single mutable [`BinaryReader`].
///
/// [`BinaryReader`]: crate::io::BinaryReader
/// [`crate::io::TileProvider`]: crate::io::TileProvider
pub trait ImageSource {
    /// Returns the authoritative tiling of the underlying image.
    ///
    /// The returned layout is valid for the lifetime of this source.
    fn layout(&self) -> &TileLayout;

    /// Reads the raw pixel bytes for one `index`.
    ///
    /// # Errors
    ///
    /// Returns [`crate::ErrorCode::OutOfRange`] if `index` lies outside the
    /// layout's grid, or a backend-specific error if the underlying read fails.
    fn read_tile(&mut self, index: TileIndex) -> Result<Tile<'_>>;
}
