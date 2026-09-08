//! Strongly-typed opaque handles.
//!
//! Mirrors `ptiff::Id<Tag>` and the concrete aliases (see
//! `libptiff/include/ptiff/core/id.hpp`).

use std::marker::PhantomData;

/// Strongly-typed opaque handle.
///
/// `Id<Tag>` wraps a `u64` value but keeps it **distinct from every other `Id`
/// type** at compile time. The `Tag` type parameter exists only for that
/// purpose: it has no data of its own and is never instantiated by itself -- its
/// sole job is to stop, say, [`ImageId`] and [`CameraId`] from being interchanged
/// by accident.
///
/// **Scoping:** `Id` values carry **no meaning outside the `Scene` that issued
/// them**. Two ids from different scenes are not comparable in a meaningful way,
/// and the same numeric value may identify different objects in different
/// scenes. Ids are minted by `Scene::add_image` and friends, not hand-rolled by
/// callers.
///
/// **Identity contract (P0-05):** an `Id` is the PTIFF-local identity of one
/// entity inside its issuing Scene. It is *not*:
///
/// * an external/persistent identifier (see [`crate::identity::ExternalId`]),
/// * a physical TIFF property — IFD index, byte offset, tile layout and file
///   name are container mechanics, never scientific identity,
/// * a content hash — two identical descriptors are two distinct entities with
///   distinct ids,
/// * a global identifier — the same numeric `Id` in two different Scenes names
///   two different entities.
///
/// Uniqueness is guaranteed by the issuing Scene (monotonic minting; there is
/// no removal), and ids are stable when unrelated entities are added. Because
/// `Scene` has no clone, an in-memory whole-scene "copy" is not available; a
/// scientific duplication is expressed by adding a new entity, which mints a
/// new, distinct id. Copying a PTIFF *file* is byte-identical and re-reads
/// with the same file-order ids — the 1.x reader mints ids from IFD order, so
/// reading never fabricates scientific identity (migration tooling owns that
/// mapping later).
///
/// Ids are deliberately constrained: no arithmetic (`+`, `-`, ...) and no
/// implicit conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Id<Tag> {
    value: u64,
    _marker: PhantomData<Tag>,
}

impl<Tag> Id<Tag> {
    /// Wraps a raw 64-bit value into this `Id` type. The conversion is explicit.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self {
            value,
            _marker: PhantomData,
        }
    }

    /// Returns the underlying numeric value.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.value
    }
}

// Marker tags (mirrors the C++ `detail::*Tag` types). Each is an empty enum so
// it can never be instantiated; the only purpose is to disambiguate `Id` types.
macro_rules! declare_id_tag {
    ($(#[doc=$doc:literal] $name:ident),* $(,)?) => {
        $(
            #[doc=$doc]
            #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
            pub enum $name {}
        )*
    };
}

declare_id_tag! {
    #[doc="Tag for [`ImageId`]."] ImageIdTag,
    #[doc="Tag for [`CameraId`]."] CameraIdTag,
    #[doc="Tag for [`LayerId`]."] LayerIdTag,
    #[doc="Tag for [`AnnotationId`]."] AnnotationIdTag,
    #[doc="Tag for [`GeometryId`]."] GeometryIdTag,
    #[doc="Tag for [`TileId`]."] TileIdTag,
    #[doc="Tag for [`ObservationId`]."] ObservationIdTag,
    #[doc="Tag for [`DataObjectId`]."] DataObjectIdTag,
    #[doc="Tag for [`ProductId`]."] ProductIdTag,
    #[doc="Tag for [`ProcessRecordId`]."] ProcessRecordIdTag,
}

/// Unique id of an image within its scene.
pub type ImageId = Id<ImageIdTag>;
/// Unique id of a camera within its scene.
pub type CameraId = Id<CameraIdTag>;
/// Unique id of a scientific layer within its scene.
pub type LayerId = Id<LayerIdTag>;
/// Unique id of an annotation within its scene.
pub type AnnotationId = Id<AnnotationIdTag>;
/// Unique id of a geometry within its scene.
pub type GeometryId = Id<GeometryIdTag>;
/// Unique id of a tile within a backend.
pub type TileId = Id<TileIdTag>;
/// Unique id of an observation within its scene (Core Model, CM-01).
pub type ObservationId = Id<ObservationIdTag>;
/// Unique id of a data object within its scene (Core Model, CM-01).
pub type DataObjectId = Id<DataObjectIdTag>;
/// Unique id of a product within its scene (Core Model, CM-01).
pub type ProductId = Id<ProductIdTag>;
/// Unique id of a process record within its scene (Core Model, CM-03).
pub type ProcessRecordId = Id<ProcessRecordIdTag>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_roundtrip() {
        let id = TileId::new(7);
        assert_eq!(id.value(), 7);
        assert_eq!(id, TileId::new(7));
        assert_ne!(id, TileId::new(8));
    }

    // Distinct tags produce distinct types sharing the same numeric payload.
    // (Comparing an ImageId to a CameraId does not typecheck — a documented
    // compile-time guarantee.)
    #[test]
    fn distinct_tags_are_distinct_types() {
        let image: ImageId = ImageId::new(42);
        let camera: CameraId = CameraId::new(42);
        assert_eq!(image.value(), camera.value());
    }
}
