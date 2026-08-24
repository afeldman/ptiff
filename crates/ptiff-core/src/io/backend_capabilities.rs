//! Capability flags advertised by a [`StorageBackend`].
//!
//! Mirrors `ptiff::io::BackendCapabilities` (see
//! `libptiff/include/ptiff/io/backend_capabilities.hpp`).

/// Describes what a [`StorageBackend`] can do.
///
/// A plain flag struct so callers -- [`crate::io::BackendFactory`] users, future
/// `Reader`/`Writer` implementations -- can pick or reject a backend based on
/// its advertised abilities without hardcoding per-format knowledge. Each
/// backend advertises its true support for the flags below; callers should not
/// assume a flag is set unless the backend reports it.
///
/// [`crate::io::BackendFactory`]: crate::io::BackendFactory
/// [`StorageBackend`]: crate::io::StorageBackend
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BackendCapabilities {
    /// Set if the backend stores images as tiled data (and can serve/sink
    /// tile-by-tile).
    pub supports_tiling: bool,
    /// Set if the backend can stream data incrementally (rather than only
    /// whole-file at once).
    pub supports_streaming: bool,
    /// Set if the backend supports arbitrary positional reads/writes (seek).
    pub supports_random_access: bool,
    /// Set if this backend's format is safe to read over a slow, small-read
    /// heavy random-access transport (e.g. HTTP range-reads) -- i.e. its
    /// header/directory layout does not require large sequential scans to open.
    /// This does not mean the backend itself does any networking; it is a
    /// statement about the format, not the transport.
    pub supports_cloud_streaming: bool,
}
