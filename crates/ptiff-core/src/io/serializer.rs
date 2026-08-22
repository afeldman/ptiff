//! Serializer trait: domain `Scene` → format-neutral [`StorageModel`].
//!
//! Mirrors `ptiff::io::Serializer` (see `libptiff/include/ptiff/io/serializer.hpp`).

use crate::scene::Scene;
use crate::{Result, StorageModel};

/// Converts a [`Scene`] into a format-neutral [`StorageModel`].
///
/// A `Serializer` is the boundary between the **domain model** (a [`Scene`],
/// above this layer) and the **storage layer** (a [`StorageModel`], below). It
/// maps the scene's semantic content -- images, cameras, layers, annotations --
/// into the structured, format-neutral nodes and fields of a `StorageModel`,
/// without touching any bytes.
///
/// **No bytes cross this boundary.** Everything below `StorageModel` is a
/// [`crate::io::StorageBackend`]'s problem; everything above it is the domain
/// model's. Keeping the two decoupled is what lets the same scene serialize to
/// any format.
///
/// **Thread-safety:** thread-compatible if stateless, which is the expected
/// case (a pure function of its argument). A caching implementation must
/// document otherwise.
///
/// [`crate::io::StorageBackend`]: crate::io::StorageBackend
pub trait Serializer {
    /// Converts `scene` into a format-neutral storage model.
    ///
    /// # Errors
    ///
    /// Returns a serializer-specific error (typically
    /// [`crate::ErrorCode::InvalidArgument`]) if `scene` cannot be represented
    /// in the format-neutral model.
    fn serialize(&self, scene: &Scene) -> Result<StorageModel>;
}
