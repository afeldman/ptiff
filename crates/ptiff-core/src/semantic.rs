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
//!
//! # Provenance (CM-03)
//!
//! Provenance is a **directed process graph** (I-7, D14): per-producing-step
//! [`ProcessRecord`] nodes connected to CM-01 entities through
//! [`ProvenanceRelation`] edges of kind [`ProvenanceRelationKind::Used`] or
//! [`ProvenanceRelationKind::Generated`]. Provenance is deliberately **not**
//! the CM-02 Relationship layer and not `DerivedFrom` in disguise: an
//! ordinary semantic edge ("B is derived from A") coexists with explicit
//! process semantics ("processing record X used A and generated B") without
//! automatic conversion or inference. Records are immutable and
//! append-only in this increment (no mutation, no removal, no rewrite).
//! Provenance remains in-memory and serialization-agnostic.
//!
//! # Axes (AX-01)
//!
//! A [`DataObject`] declares its scientific dimensions as ordered semantic
//! [`AxisDescriptor`]s ([`AxisKind`]) — never as bare array shape. An axis is
//! semantic meaning, not a TIFF width/height, not storage order, not CRS.
//! Coordinate values, units, time systems, spectral calibration and
//! serialization syntax are all deferred (ADR-003).

use crate::{Error, Result};

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
/// identity, and AX-01 adds the ordered semantic axis declaration.
///
/// `DataObject` is distinct from [`Observation`] and [`Product`].
///
/// Axes are intrinsic semantic descriptors of the data object (ordered,
/// deterministic, immutable-by-convention), **not** identity: two data
/// objects with identical axes are distinct entities when their ids differ.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct DataObject {
    axes: Vec<AxisDescriptor>,
}

impl DataObject {
    /// Creates an empty data object (no data-representation fields yet, no
    /// axes declared).
    ///
    /// An empty axis collection means "not yet populated" in this increment;
    /// the I-5 requirement that conformant data objects declare explicit
    /// axes is enforced at validation time (VAL-01), not at construction.
    #[must_use]
    pub fn new() -> Self {
        Self { axes: Vec::new() }
    }

    /// Declares an axis for this data object.
    ///
    /// Axes are appended in semantic order: their position in the collection
    /// is their semantic position (deterministic; no separate ordinal field
    /// that could drift from the collection).
    ///
    /// # Errors
    ///
    /// [`crate::ErrorCode::InvalidArgument`] if the data object already
    /// declares an axis of the same [`AxisKind`] (one axis kind per data
    /// object), or if `axis` failed its own structural validation.
    pub fn add_axis(&mut self, axis: AxisDescriptor) -> Result<()> {
        // Validate the descriptor itself (extent > 0) even though it was
        // constructed through `AxisDescriptor::new`; this keeps validation
        // at the single Scene/entity boundary as well.
        axis.validate()?;
        if self.axes.iter().any(|a| a.kind() == axis.kind()) {
            return Err(Error::invalid_argument(
                "DataObject::add_axis: an axis of this kind is already declared",
            ));
        }
        self.axes.push(axis);
        Ok(())
    }

    /// The declared axes in deterministic semantic order.
    #[must_use]
    pub fn axes(&self) -> &[AxisDescriptor] {
        &self.axes
    }

    /// Number of declared axes.
    #[must_use]
    pub fn axis_count(&self) -> usize {
        self.axes.len()
    }

    /// The declared axis of `kind`, if any.
    #[must_use]
    pub fn axis(&self, kind: AxisKind) -> Option<&AxisDescriptor> {
        self.axes.iter().find(|a| a.kind() == kind)
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

/// The semantic role of a [`DataObject`] axis (AX-01).
///
/// Canonical role vocabulary from the frozen architecture (ADR-003: X, Y,
/// Band, Time, Polarization, …). These roles name scientific meaning, never
/// storage layout:
///
/// * [`AxisKind::X`] — the spatial sample/column axis.
/// * [`AxisKind::Y`] — the spatial line/row axis.
/// * [`AxisKind::Band`] — the band axis (D7 spectral band model; band
///   coordinates such as wavelength/bandwidth/units are a later concern).
/// * [`AxisKind::Time`] — the time axis (no time system is implied).
/// * [`AxisKind::Polarization`] — the polarization/stokes axis.
///
/// The role registry is open (ADR-003 open item); this enum is additive and
/// `#[non_exhaustive]`. X/Y are spatial *semantics* only — longitude/latitude,
/// projection and CRS are separate (never merged into axes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AxisKind {
    /// Spatial sample/column axis (semantic; no CRS implied).
    X,
    /// Spatial line/row axis (semantic; no CRS implied).
    Y,
    /// Band axis (spectral semantics per D7; coordinates deferred).
    Band,
    /// Time axis (no time system implied).
    Time,
    /// Polarization/stokes axis.
    Polarization,
}

/// A semantic axis descriptor of a [`DataObject`] (AX-01).
///
/// ```text
/// AxisDescriptor
///     ├── kind    — semantic role (AxisKind)
///     ├── extent  — logical cardinality along this axis (> 0)
///     └── order   — position in the DataObject's ordered axis collection
/// ```
///
/// An axis descriptor is a value object: it carries no Scene identity, no
/// coordinates, no units, no CRS, no calibration. The semantic extent must
/// not be confused with TIFF width/height or storage dimensions — physical
/// mapping is a later serialization/storage concern (SR-01).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AxisDescriptor {
    kind: AxisKind,
    extent: u64,
}

impl AxisDescriptor {
    /// Builds an axis descriptor for `kind` with logical extent `extent`.
    ///
    /// # Errors
    ///
    /// [`crate::ErrorCode::InvalidArgument`] if `extent` is zero (a declared
    /// semantic axis must have cardinality ≥ 1). Negative extents are
    /// unrepresentable by construction (`u64`).
    pub fn new(kind: AxisKind, extent: u64) -> Result<Self> {
        let axis = Self { kind, extent };
        axis.validate()?;
        Ok(axis)
    }

    /// Structural validation: extent must be ≥ 1.
    pub(crate) fn validate(&self) -> Result<()> {
        if self.extent == 0 {
            return Err(Error::invalid_argument(
                "AxisDescriptor: extent must be at least 1",
            ));
        }
        Ok(())
    }

    /// The semantic role of this axis.
    #[must_use]
    pub const fn kind(&self) -> AxisKind {
        self.kind
    }

    /// The logical cardinality along this axis.
    #[must_use]
    pub const fn extent(&self) -> u64 {
        self.extent
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

/// A provenance process step (D14 "ProcessRecord", the Activity/Process
/// concept of the Core Model).
///
/// A `ProcessRecord` represents one scientific or processing step that acted
/// upon entities — calibration, photometric correction, geometric processing,
/// surface reconstruction, derivation, ... It is semantically distinct from
/// an [`Observation`] (a measurement event), a [`DataObject`], a [`Product`]
/// and a [`Relationship`]. No domain-specific processing types are encoded in
/// CM-03.
///
/// Records are immutable and append-only: every producing step adds a new
/// record; history is chained by later records and never rewritten
/// (in-memory policy; validator ordering is a later concern). The record's
/// content fields (software, parameters, actor, time, environment — D14) are
/// intentionally absent here; CM-03 establishes the graph semantics and the
/// Scene-scoped identity.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ProcessRecord {}

impl ProcessRecord {
    /// Creates an empty process record (no content fields yet).
    #[must_use]
    pub const fn new() -> Self {
        Self {}
    }
}

/// The kind of a [`ProvenanceRelation`] edge.
///
/// Canonical orientation (CM-03): both kinds originate at the
/// [`ProcessRecord`] and point at the CM-01 entity.
///
/// ```text
/// ProcessRecord ──Used──>     entity (consumed/input by the process)
/// ProcessRecord ──Generated──> entity (produced/output by the process)
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ProvenanceRelationKind {
    /// The process used the entity as an input.
    Used,
    /// The process generated the entity as an output.
    Generated,
}

/// An explicit directed provenance edge:
/// `process --kind--> entity`.
///
/// Distinct from [`Relationship`]: provenance relations always involve a
/// [`ProcessRecord`] endpoint and describe how entities were produced or
/// consumed through process activity. Direction is preserved — `process
/// --Used--> entity` is not equivalent to the reverse — and no inverse edges
/// are created automatically. The provenance graph holds only explicitly
/// declared relations (no inference).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvenanceRelation {
    process: crate::id::ProcessRecordId,
    kind: ProvenanceRelationKind,
    entity: EntityRef,
}

impl ProvenanceRelation {
    /// Builds a provenance relation value without Scene validation.
    ///
    /// Prefer [`crate::Scene::add_provenance_relation`], which validates Scene
    /// membership and duplicates.
    #[must_use]
    pub const fn new(
        process: crate::id::ProcessRecordId,
        kind: ProvenanceRelationKind,
        entity: EntityRef,
    ) -> Self {
        Self {
            process,
            kind,
            entity,
        }
    }

    /// The process-record endpoint (source).
    #[must_use]
    pub const fn process(&self) -> crate::id::ProcessRecordId {
        self.process
    }

    /// The provenance kind.
    #[must_use]
    pub const fn kind(&self) -> ProvenanceRelationKind {
        self.kind
    }

    /// The entity endpoint (target).
    #[must_use]
    pub const fn entity(&self) -> EntityRef {
        self.entity
    }
}
