//! Core Model semantic entities (CM-01).
//!
//! The smallest additive semantic layer of PTIFF 2.0. It establishes three
//! distinct entity kinds whose separation is the foundation of the frozen
//! architecture:
//!
//! ```text
//! Observation  = a scientific measurement/acquisition event
//! DataObject   = a concrete scientific data representation
//! Product      = a scientific or derived result
//! ```
//!
//! # Invariants (frozen architecture)
//!
//! * **An Observation is not a DataObject, and a DataObject is not a
//!   Product.** The three kinds are distinct Rust types with distinct typed
//!   ids ([`crate::id::ObservationId`], [`crate::id::DataObjectId`],
//!   [`crate::id::ProductId`]) so accidental interchange is a compile-time
//!   error.
//! * Entities are additive to the 1.x model. They do not rename, replace or
//!   reinterpret [`crate::Image`], [`crate::Camera`], [`crate::Geometry`] or
//!   layers.
//! * The [`crate::Scene`] owns identity allocation: entities never mint their
//!   own ids and never exist outside a Scene. Ids are Scene-scoped (P0-05
//!   contract).
//! * The entities are intentionally minimal: this module establishes identity
//!   and semantic existence only. Geometry, photometry, spectral axes,
//!   relationships, provenance, raster details and all other content belong
//!   to later increments.
//!
//! # Deliberately absent
//!
//! * No provenance graph, no `derived_from`/`source_ids` fields on entities.
//! * No axis model.
//! * **No serialization.** These types carry no serde derives and are never
//!   written to TIFF tags, a manifest, or any storage form. Attaching them to
//!   a Scene does not change one byte of 1.x output. (R5: no accidental
//!   serialization smuggling before G1.)
//!
//! # Relationships (CM-02)
//!
//! Relationships are explicit directed graph edges ([`Relationship`]) owned
//! by the [`crate::Scene`]. Endpoints retain typed identity
//! ([`EntityRef`]). The kind vocabulary is deliberately small and
//! PTIFF-native (ADR-009): [`RelationshipKind::Produces`] links an
//! Observation to the DataObject it produced; [`RelationshipKind::DerivedFrom`]
//! links derivation inputs to outputs — an edge `S → T` of kind
//! `DerivedFrom` reads "T is derived from S" (canonical ADR-002 verb
//! orientation). Provenance, axes, serialization and the full vocabulary
//! remain outside CM-02.

/// A scientific measurement/acquisition event.
///
/// An `Observation` describes *the event in which data was acquired*: a
/// remote-sensing image take, a lidar pass, a laboratory scan. Acquisition
/// metadata, time, instrument context, geometry, photometry and spectral
/// context belong to later increments; CM-01 establishes only the entity and
/// its Scene-scoped identity.
///
/// `Observation` is distinct from [`DataObject`] and [`Product`] by type and
/// by identity: an observation id can never be passed where a data-object id
/// or product id is expected.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Observation {}

impl Observation {
    /// Creates an empty observation (no acquisition fields yet).
    #[must_use]
    pub const fn new() -> Self {
        Self {}
    }
}

/// A concrete scientific data representation.
///
/// A `DataObject` is actual scientific data associated with an observation or
/// otherwise present in the Scene: raster data, masks, uncertainty, spectral
/// arrays, auxiliary scientific arrays. Raster/axis details are later
/// increments; CM-01 establishes only the entity and its Scene-scoped
/// identity.
///
/// `DataObject` is distinct from [`Observation`] and [`Product`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct DataObject {}

impl DataObject {
    /// Creates an empty data object (no data-representation fields yet).
    #[must_use]
    pub const fn new() -> Self {
        Self {}
    }
}

/// A scientific or derived result.
///
/// A `Product` is the output of scientific processing — a DEM, an albedo
/// map, a mosaic, a classified image — and is **not** merely another name for
/// a [`DataObject`]. Products may later reference source data objects,
/// observations and other products through the explicit Relationship model;
/// nothing is connected in CM-01.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Product {}

impl Product {
    /// Creates an empty product (no derived-result fields yet).
    #[must_use]
    pub const fn new() -> Self {
        Self {}
    }
}

/// A typed reference to a Core Model entity, preserving the entity kind.
///
/// Endpoint identity is never reduced to a bare `u64`/string: each variant
/// wraps the Scene-scoped typed id from P0-05/CM-01
/// ([`crate::id::ObservationId`], [`crate::id::DataObjectId`],
/// [`crate::id::ProductId`]).
///
/// An `EntityRef` alone does **not** establish Scene membership — the
/// [`crate::Scene`] validates that any referenced entity belongs to itself
/// when a relationship is created.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum EntityRef {
    /// Reference to an [`Observation`].
    Observation(crate::id::ObservationId),
    /// Reference to a [`DataObject`].
    DataObject(crate::id::DataObjectId),
    /// Reference to a [`Product`].
    Product(crate::id::ProductId),
}

impl EntityRef {
    /// The numeric Scene-local id value shared by every variant (typed access
    /// happens through the variant itself).
    #[must_use]
    pub const fn value(self) -> u64 {
        match self {
            EntityRef::Observation(id) => id.value(),
            EntityRef::DataObject(id) => id.value(),
            EntityRef::Product(id) => id.value(),
        }
    }
}

/// The semantic kind of a [`Relationship`] edge.
///
/// PTIFF-native vocabulary (ADR-009), kept intentionally small for CM-02:
///
/// * [`RelationshipKind::Produces`] — an Observation produced a DataObject
///   (edge `Observation → DataObject`).
/// * [`RelationshipKind::DerivedFrom`] — derivation link; an edge `S → T`
///   reads "T is derived from S". Currently used for
///   `DataObject → Product` and `Product → Product` (derived-product
///   chains). The full edge-type list is finalized with the core schema
///   (deferred vocabulary detail, ADR-009 open item).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RelationshipKind {
    /// Source Observation produced the target DataObject.
    Produces,
    /// Target is derived from source (source → derived target).
    DerivedFrom,
}

impl RelationshipKind {
    /// True when an edge of this kind may connect `source` to `target`.
    ///
    /// CM-02 permits exactly:
    ///
    /// ```text
    /// Observation → DataObject  (Produces)
    /// DataObject   → Product    (DerivedFrom)
    /// Product      → Product    (DerivedFrom)
    /// ```
    ///
    /// Everything else — Observation→Observation, Observation→Product,
    /// DataObject→Observation, Product→Observation — is rejected by
    /// [`crate::Scene::add_relationship`].
    #[must_use]
    pub fn permits(self, source: EntityRef, target: EntityRef) -> bool {
        matches!(
            (self, source, target),
            (
                RelationshipKind::Produces,
                EntityRef::Observation(_),
                EntityRef::DataObject(_)
            ) | (
                RelationshipKind::DerivedFrom,
                EntityRef::DataObject(_),
                EntityRef::Product(_)
            ) | (
                RelationshipKind::DerivedFrom,
                EntityRef::Product(_),
                EntityRef::Product(_)
            )
        )
    }
}

/// An explicit directed semantic relationship between two Core Model
/// entities: `source --kind--> target`.
///
/// Relationships are **not** provenance (CM-03), not ownership, and not
/// hierarchy: the kind carries the meaning. Direction is semantic —
/// `A → B` never implies `B → A`.
///
/// A `Relationship` is a value object. Scene-scoped identity of the entities
/// it connects is preserved through [`EntityRef`]; the edge itself carries no
/// Scene-global id (it is not independently addressable in CM-02). Scene
/// owns the collection and validates membership, kind/domain compatibility
/// and duplicates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relationship {
    source: EntityRef,
    kind: RelationshipKind,
    target: EntityRef,
}

impl Relationship {
    /// Builds a relationship value without Scene validation.
    ///
    /// Prefer [`crate::Scene::add_relationship`], which validates Scene
    /// membership, permitted endpoint domains and duplicate policy. This
    /// constructor exists for read-side reconstruction and tests of the value
    /// type.
    #[must_use]
    pub const fn new(source: EntityRef, kind: RelationshipKind, target: EntityRef) -> Self {
        Self {
            source,
            kind,
            target,
        }
    }

    /// The source endpoint.
    #[must_use]
    pub const fn source(&self) -> EntityRef {
        self.source
    }

    /// The relationship kind.
    #[must_use]
    pub const fn kind(&self) -> RelationshipKind {
        self.kind
    }

    /// The target endpoint.
    #[must_use]
    pub const fn target(&self) -> EntityRef {
        self.target
    }
}
