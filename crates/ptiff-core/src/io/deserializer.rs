//! Deserializer trait: format-neutral [`StorageModel`] → domain `Scene`.
//!
//! Mirrors `ptiff::io::Deserializer` (see `libptiff/include/ptiff/io/deserializer.hpp`).

use crate::scene::Scene;
use crate::{Result, StorageModel};

/// Converts a format-neutral [`StorageModel`] into a domain-model [`Scene`].
///
/// `Deserializer` is the inverse of [`Serializer`]: it maps the structured,
/// format-neutral nodes and fields of a `StorageModel` back into the domain
/// model's [`Scene`] -- images, cameras, layers, annotations. As with its
/// mirror, no binary data crosses this boundary; turning bytes into a
/// `StorageModel` is a [`crate::io::StorageBackend`] concern.
///
/// **Thread-safety:** mirrors [`Serializer`]: thread-compatible if stateless
/// (the expected case); a caching implementation must document otherwise.
///
/// [`Serializer`]: crate::io::Serializer
/// [`crate::io::StorageBackend`]: crate::io::StorageBackend
pub trait Deserializer {
    /// Converts `model` into a domain-model scene.
    ///
    /// # Errors
    ///
    /// Returns a deserializer-specific error (typically
    /// [`crate::ErrorCode::InvalidArgument`]) if `model` is missing fields or
    /// contradicts the domain model's constraints.
    fn deserialize(&self, model: &StorageModel) -> Result<Scene>;
}
