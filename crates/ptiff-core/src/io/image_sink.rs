//! Pixel-data-level write abstraction over one image.
//!
//! Mirrors `ptiff::io::ImageSink` (see
//! `libptiff/include/ptiff/io/image_sink.hpp`).

use crate::tile::{Tile, TileLayout};
use crate::Result;

/// Pixel-data-level write abstraction over one image.
///
/// `ImageSink` is the write-side counterpart to [`ImageSource`]: it consumes an
/// image one [`Tile`] at a time, so a caller can stream pixels into a file
/// without knowing the target format's on-disk byte layout. Edge tiles may be
/// padded to the full tile size, matching the contract of
/// [`TileLayout`].
///
/// **Contract**
///
/// - [`layout`](ImageSink::layout) describes how the image is tiled; tiles
///   handed to [`write_tile`](ImageSink::write_tile) must agree with this
///   layout.
/// - [`write_tile`](ImageSink::write_tile) persists one tile's pixel bytes,
///   returning the outcome.
///
/// **Thread-safety:** mirrors [`ImageSource`]: not thread-safe by default,
/// since a concrete sink typically wraps a single mutable
/// [`BinaryWriter`].
///
/// [`ImageSource`]: crate::io::ImageSource
/// [`BinaryWriter`]: crate::io::BinaryWriter
/// [`TileLayout`]: crate::tile::TileLayout
pub trait ImageSink {
    /// Returns the authoritative tiling of the target image.
    ///
    /// The returned layout is valid for the lifetime of this sink.
    fn layout(&self) -> &TileLayout;

    /// Persists one `tile`'s pixel bytes.
    ///
    /// # Errors
    ///
    /// Returns [`crate::ErrorCode::OutOfRange`] if `tile`'s index lies outside
    /// the layout's grid, or a backend-specific error if the underlying write
    /// fails.
    fn write_tile(&mut self, tile: &Tile<'_>) -> Result<()>;
}
