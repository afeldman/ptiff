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
//! * No relationships (Observation→DataObject, DataObject→Product, ...).
//! * No provenance graph, no `derived_from`/`source_ids` fields.
//! * No axis model.
//! * **No serialization.** These types carry no serde derives and are never
//!   written to TIFF tags, a manifest, or any storage form. Attaching them to
//!   a Scene does not change one byte of 1.x output. (R5: no accidental
//!   serialization smuggling before G1.)

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
