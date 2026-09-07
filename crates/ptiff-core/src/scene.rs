//! A scene: the composition of multiple related images.
//!
//! Mirrors `ptiff::Scene` (see `libptiff/include/ptiff/scene.hpp`).

use crate::geometry::{Camera, Geometry};
use crate::id::{
    CameraId, DataObjectId, GeometryId, ImageId, ObservationId, ProcessRecordId, ProductId,
};
use crate::identity::ExternalId;
use crate::image::Image;
use crate::image::ImageDescriptor;
use crate::semantic::{
    DataObject, EntityRef, Observation, ProcessRecord, Product, ProvenanceRelation,
    ProvenanceRelationKind, Relationship, RelationshipKind,
};
use crate::{Error, Result};

/// A scene: the composition of multiple related images (e.g. a stereo pair, a
/// mosaic).
///
/// An owning container of [`Image`] objects. Each image is given a
/// monotonically increasing [`ImageId`] when added (0, 1, 2, ...). Copy is
/// intentionally not implemented; a `Scene` is cheap to move.
///
/// **Identity semantics (P0-05):** the minted ids are PTIFF-local handles.
/// They are unique within this Scene, stable for the lifetime of the Scene
/// (adding unrelated images never renumbers earlier ids), and never derived
/// from TIFF layout. Lookup today is implemented positionally (`ImageId` 0..n
/// doubles as the backing index), which is an implementation detail of the
/// 1.x container — physical position is **not** scientific identity. A
/// Scene-level `ExternalId` may be attached per image in memory (see
/// [`Scene::attach_external_id`]); its on-disk representation is deferred.
/// The full separation of local / external / physical identity is specified
/// in [`crate::identity`].
///
/// A `Scene` also owns optional scene-level [`Camera`] and [`Geometry`] objects
/// (added via [`Scene::add_camera`] / [`Scene::add_geometry`]), distinct from
/// the per-image camera/CRS fields. Their ids are scoped to the scene, like
/// image ids.
///
/// **Round-trip:** a `Scene` round-trips through `io::StorageModel`, including
/// its scene-level cameras/geometries (format-neutral via `SceneSerializer` /
/// `SceneDeserializer`). Not every field survives a *TIFF* round-trip:
/// `tile_info`, `ground_sample_distance_meters` and the scene-level
/// cameras/geometries are not representable in the per-image TIFF tag
/// schema and are not preserved there (see GEOMETRY-FOUNDATION.md §8 Phase IV).
///
/// The serde derives (feature `serde`) cover the image set but **not** the
/// scene-level cameras/geometries: those are serialized via the `io::Serializer`
/// trait (format-neutral `StorageModel`), mirroring how `ImageDescriptor.metadata`
/// is also excluded from serde.
#[derive(Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Scene {
    images: Vec<Image>,
    #[cfg_attr(feature = "serde", serde(skip))]
    cameras: Vec<Camera>,
    #[cfg_attr(feature = "serde", serde(skip))]
    geometries: Vec<Geometry>,
    /// External identifiers attached to images, in deterministic insertion
    /// order as `(image_id, external_id)` pairs.
    ///
    /// **In-memory only:** no serialization exists for these until the
    /// manifest encoding/location decisions (ADR-011/ADR-012) — attaching an
    /// external id therefore never changes 1.x TIFF output and never
    /// survives a storage round-trip (documented deferral, see the `identity`
    /// module).
    #[cfg_attr(feature = "serde", serde(skip))]
    external_ids: Vec<(u64, ExternalId)>,
    /// Core Model entities (CM-01). Purely additive and **never serialized**:
    /// observations/data objects/products exist only inside the live Scene
    /// until the manifest layer is designed (see the `semantic` module).
    #[cfg_attr(feature = "serde", serde(skip))]
    observations: Vec<Observation>,
    #[cfg_attr(feature = "serde", serde(skip))]
    data_objects: Vec<DataObject>,
    #[cfg_attr(feature = "serde", serde(skip))]
    products: Vec<Product>,
    /// Explicit directed semantic relationships (CM-02), in deterministic
    /// insertion order. In-memory only — never serialized (see `semantic`).
    #[cfg_attr(feature = "serde", serde(skip))]
    relationships: Vec<Relationship>,
    /// Provenance process records (CM-03), in deterministic insertion order.
    /// Immutable/append-only, in-memory only (see `semantic`).
    #[cfg_attr(feature = "serde", serde(skip))]
    process_records: Vec<ProcessRecord>,
    /// Explicit directed provenance edges (CM-03), in deterministic insertion
    /// order. In-memory only.
    #[cfg_attr(feature = "serde", serde(skip))]
    provenance_relations: Vec<ProvenanceRelation>,
    next_id: u64,
    next_camera_id: u64,
    next_geometry_id: u64,
    next_observation_id: u64,
    next_data_object_id: u64,
    next_product_id: u64,
    next_process_record_id: u64,
}

impl Scene {
    /// Constructs an empty scene (no images).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends an image built from `descriptor` and returns its new [`ImageId`].
    ///
    /// The returned id is the next value of the per-Scene counter starting at
    /// 0.
    pub fn add_image(&mut self, descriptor: ImageDescriptor) -> Result<ImageId> {
        // Matches the C++ oracle: images are appended unconditionally (no
        // degenerate-dimension validation here).
        let id = self.next_id;
        self.next_id += 1;
        self.images.push(Image::new(descriptor));
        Ok(ImageId::new(id))
    }

    /// Number of images in the scene.
    #[must_use]
    pub fn image_count(&self) -> usize {
        self.images.len()
    }

    /// Looks up an image by its [`ImageId`].
    ///
    /// # Errors
    ///
    /// Returns [`crate::ErrorCode::NotFound`] if `id` was never returned by
    /// `add_image`.
    pub fn image(&self, id: ImageId) -> Result<&Image> {
        self.images
            .get(id.value() as usize)
            .ok_or_else(|| Error::not_found("Scene::image: no image with this id"))
    }

    /// Returns the image at `index` in scene order (the order they were added).
    ///
    /// # Errors
    ///
    /// Returns [`crate::ErrorCode::OutOfRange`] if `index` is not in `[0, image_count())`.
    pub fn image_at(&self, index: usize) -> Result<&Image> {
        self.images
            .get(index)
            .ok_or_else(|| Error::out_of_range("Scene::image_at: index out of range"))
    }

    /// Attaches an [`ExternalId`] to the image identified by `image`.
    ///
    /// An external identifier is a reference into an external system (PDS4,
    /// ISIS, mission archive, source dataset); it never replaces the
    /// PTIFF-local [`ImageId`]. One image may carry several external ids, at
    /// most one per namespace.
    ///
    /// This is **in-memory only**: external ids have no on-disk
    /// representation until the manifest encoding/location decisions
    /// (ADR-011/ADR-012) — see [`crate::identity`].
    ///
    /// # Errors
    ///
    /// * [`crate::ErrorCode::NotFound`] if `image` was never returned by
    ///   [`Scene::add_image`].
    /// * [`crate::ErrorCode::InvalidArgument`] if the image already carries an
    ///   external id in the same namespace (conflicting duplicate).
    pub fn attach_external_id(&mut self, image: ImageId, external: ExternalId) -> Result<()> {
        let image_index = self.validate_image_id(image)?;
        if self
            .external_ids
            .iter()
            .any(|(id, e)| *id == image_index && e.namespace() == external.namespace())
        {
            return Err(Error::invalid_argument(
                "Scene::attach_external_id: image already has an external id in this namespace",
            ));
        }
        self.external_ids.push((image_index, external));
        Ok(())
    }

    /// Returns the external id attached to `image` under `namespace`, if any.
    ///
    /// # Errors
    ///
    /// [`crate::ErrorCode::NotFound`] if `image` was never returned by
    /// [`Scene::add_image`].
    pub fn external_id(&self, image: ImageId, namespace: &str) -> Result<Option<&ExternalId>> {
        let image_index = self.validate_image_id(image)?;
        Ok(self
            .external_ids
            .iter()
            .find(|(id, e)| *id == image_index && e.namespace() == namespace)
            .map(|(_, e)| e))
    }

    /// Returns every external id attached to `image`, in attachment order
    /// (deterministic; never hash-order dependent).
    ///
    /// # Errors
    ///
    /// [`crate::ErrorCode::NotFound`] if `image` was never returned by
    /// [`Scene::add_image`].
    pub fn external_ids_for(&self, image: ImageId) -> Result<Vec<&ExternalId>> {
        let image_index = self.validate_image_id(image)?;
        Ok(self
            .external_ids
            .iter()
            .filter(|(id, _)| *id == image_index)
            .map(|(_, e)| e)
            .collect())
    }

    /// Validates that `image` is an id minted by this scene and returns its
    /// backing index.
    fn validate_image_id(&self, image: ImageId) -> Result<u64> {
        let index = image.value();
        if index >= self.images.len() as u64 {
            return Err(Error::not_found("Scene: no image with this id"));
        }
        Ok(index)
    }

    /// Appends a scene-level camera and returns its new [`CameraId`].
    ///
    /// The returned id is scoped to this scene (starting at 0, monotonic), like
    /// image ids. Scene-level cameras are distinct from the per-image `camera`
    /// field and are serialized format-neutrally via `io::Serializer` (they are
    /// not representable in the per-image TIFF tag schema).
    pub fn add_camera(&mut self, camera: Camera) -> CameraId {
        let id = self.next_camera_id;
        self.next_camera_id += 1;
        self.cameras.push(camera);
        CameraId::new(id)
    }

    /// Number of scene-level cameras in the scene.
    #[must_use]
    pub fn camera_count(&self) -> usize {
        self.cameras.len()
    }

    /// Looks up a scene-level camera by its [`CameraId`].
    ///
    /// # Errors
    ///
    /// Returns [`crate::ErrorCode::NotFound`] if `id` was never returned by
    /// `add_camera`.
    pub fn camera(&self, id: CameraId) -> Result<&Camera> {
        self.cameras
            .get(id.value() as usize)
            .ok_or_else(|| Error::not_found("Scene::camera: no camera with this id"))
    }

    /// Returns the camera at `index` in scene order (the order they were added).
    ///
    /// # Errors
    ///
    /// Returns [`crate::ErrorCode::OutOfRange`] if `index` is not in `[0, camera_count())`.
    pub fn camera_at(&self, index: usize) -> Result<&Camera> {
        self.cameras
            .get(index)
            .ok_or_else(|| Error::out_of_range("Scene::camera_at: index out of range"))
    }

    /// Appends a scene-level geometry and returns its new [`GeometryId`].
    ///
    /// The returned id is scoped to this scene (starting at 0, monotonic), like
    /// image ids. Scene-level geometries are distinct from the per-image CRS and
    /// are serialized format-neutrally via `io::Serializer` (they are not
    /// representable in the per-image TIFF tag schema).
    pub fn add_geometry(&mut self, geometry: Geometry) -> GeometryId {
        let id = self.next_geometry_id;
        self.next_geometry_id += 1;
        self.geometries.push(geometry);
        GeometryId::new(id)
    }

    /// Number of scene-level geometries in the scene.
    #[must_use]
    pub fn geometry_count(&self) -> usize {
        self.geometries.len()
    }

    /// Looks up a scene-level geometry by its [`GeometryId`].
    ///
    /// # Errors
    ///
    /// Returns [`crate::ErrorCode::NotFound`] if `id` was never returned by
    /// `add_geometry`.
    pub fn geometry(&self, id: GeometryId) -> Result<&Geometry> {
        self.geometries
            .get(id.value() as usize)
            .ok_or_else(|| Error::not_found("Scene::geometry: no geometry with this id"))
    }

    /// Returns the geometry at `index` in scene order (the order they were added).
    ///
    /// # Errors
    ///
    /// Returns [`crate::ErrorCode::OutOfRange`] if `index` is not in `[0, geometry_count())`.
    pub fn geometry_at(&self, index: usize) -> Result<&Geometry> {
        self.geometries
            .get(index)
            .ok_or_else(|| Error::out_of_range("Scene::geometry_at: index out of range"))
    }

    /// Appends a Core Model [`Observation`] and returns its new
    /// [`ObservationId`].
    ///
    /// The id is scoped to this scene (starting at 0, monotonic), exactly like
    /// image/camera/geometry ids, and independent of every other id family:
    /// an observation id can never be confused with a data-object or product
    /// id (distinct [`crate::id::Id`] tags), and allocating Core Model entities never
    /// renumbers existing 1.x ids. See the `semantic` module for the entity
    /// contract.
    ///
    /// # Example
    ///
    /// ```
    /// use ptiff_core::{Scene, Observation};
    ///
    /// let mut scene = Scene::new();
    /// let id = scene.add_observation(Observation::new());
    /// assert_eq!(scene.observation(id).unwrap().clone(), Observation::new());
    /// ```
    pub fn add_observation(&mut self, observation: Observation) -> ObservationId {
        let id = self.next_observation_id;
        self.next_observation_id += 1;
        self.observations.push(observation);
        ObservationId::new(id)
    }

    /// Number of Core Model observations in the scene.
    #[must_use]
    pub fn observation_count(&self) -> usize {
        self.observations.len()
    }

    /// Looks up a Core Model [`Observation`] by its [`ObservationId`].
    ///
    /// # Errors
    ///
    /// Returns [`crate::ErrorCode::NotFound`] if `id` was never returned by
    /// [`Scene::add_observation`] (including ids minted by a different Scene).
    pub fn observation(&self, id: ObservationId) -> Result<&Observation> {
        self.observations
            .get(id.value() as usize)
            .ok_or_else(|| Error::not_found("Scene::observation: no observation with this id"))
    }

    /// Appends a Core Model [`DataObject`] and returns its new
    /// [`DataObjectId`]. Independent of every other id family (see
    /// [`Scene::add_observation`]).
    ///
    /// # Example
    ///
    /// ```
    /// use ptiff_core::{Scene, DataObject};
    ///
    /// let mut scene = Scene::new();
    /// let id = scene.add_data_object(DataObject::new());
    /// assert_eq!(scene.data_object(id).unwrap().clone(), DataObject::new());
    /// ```
    pub fn add_data_object(&mut self, data_object: DataObject) -> DataObjectId {
        let id = self.next_data_object_id;
        self.next_data_object_id += 1;
        self.data_objects.push(data_object);
        DataObjectId::new(id)
    }

    /// Number of Core Model data objects in the scene.
    #[must_use]
    pub fn data_object_count(&self) -> usize {
        self.data_objects.len()
    }

    /// Looks up a Core Model [`DataObject`] by its [`DataObjectId`].
    ///
    /// # Errors
    ///
    /// Returns [`crate::ErrorCode::NotFound`] if `id` was never returned by
    /// [`Scene::add_data_object`].
    pub fn data_object(&self, id: DataObjectId) -> Result<&DataObject> {
        self.data_objects
            .get(id.value() as usize)
            .ok_or_else(|| Error::not_found("Scene::data_object: no data object with this id"))
    }

    /// Appends a Core Model [`Product`] and returns its new [`ProductId`].
    /// Independent of every other id family (see [`Scene::add_observation`]).
    ///
    /// # Example
    ///
    /// ```
    /// use ptiff_core::{Scene, Product};
    ///
    /// let mut scene = Scene::new();
    /// let id = scene.add_product(Product::new());
    /// assert_eq!(scene.product(id).unwrap().clone(), Product::new());
    /// ```
    pub fn add_product(&mut self, product: Product) -> ProductId {
        let id = self.next_product_id;
        self.next_product_id += 1;
        self.products.push(product);
        ProductId::new(id)
    }

    /// Number of Core Model products in the scene.
    #[must_use]
    pub fn product_count(&self) -> usize {
        self.products.len()
    }

    /// Looks up a Core Model [`Product`] by its [`ProductId`].
    ///
    /// # Errors
    ///
    /// Returns [`crate::ErrorCode::NotFound`] if `id` was never returned by
    /// [`Scene::add_product`].
    pub fn product(&self, id: ProductId) -> Result<&Product> {
        self.products
            .get(id.value() as usize)
            .ok_or_else(|| Error::not_found("Scene::product: no product with this id"))
    }

    /// Adds an explicit directed relationship `source --kind--> target`.
    ///
    /// The Scene validates that:
    ///
    /// 1. **Membership** — each endpoint lies within this Scene's entity
    ///    domain ([`crate::ErrorCode::NotFound`] otherwise). Handles carry no
    ///    minting provenance (P0-05 `Id<Tag>` design): a foreign handle whose
    ///    numeric value falls inside the local domain is structurally equal
    ///    to the local id and denotes the local entity — cross-Scene id
    ///    comparison is meaningless by design, and the graph stays internally
    ///    coherent. Ids outside the local domain are always rejected.
    /// 2. **Domain** — the kind permits the source/target combination
    ///    ([`crate::ErrorCode::InvalidArgument`] otherwise; see
    ///    [`RelationshipKind::permits`]).
    /// 3. **Duplicates** — an identical edge (same source, kind and target)
    ///    is rejected ([`crate::ErrorCode::InvalidArgument`]); different
    ///    kinds between the same entities are independent edges.
    ///
    /// Relationships are in-memory semantic model objects (no serialization,
    /// no TIFF/manifest impact). See the `semantic` module.
    ///
    /// # Errors
    ///
    /// [`crate::ErrorCode::NotFound`] for foreign endpoints,
    /// [`crate::ErrorCode::InvalidArgument`] for disallowed kinds or exact
    /// duplicates.
    pub fn add_relationship(
        &mut self,
        source: EntityRef,
        kind: RelationshipKind,
        target: EntityRef,
    ) -> Result<()> {
        self.validate_entity_ref(source)?;
        self.validate_entity_ref(target)?;
        if !kind.permits(source, target) {
            return Err(Error::invalid_argument(
                "Scene::add_relationship: relationship kind does not permit these endpoints",
            ));
        }
        if self
            .relationships
            .iter()
            .any(|r| r.source() == source && r.kind() == kind && r.target() == target)
        {
            return Err(Error::invalid_argument(
                "Scene::add_relationship: duplicate relationship (same source, kind and target)",
            ));
        }
        self.relationships
            .push(Relationship::new(source, kind, target));
        Ok(())
    }

    /// Number of relationships in the scene.
    #[must_use]
    pub fn relationship_count(&self) -> usize {
        self.relationships.len()
    }

    /// All relationships in deterministic insertion order.
    #[must_use]
    pub fn relationships(&self) -> &[Relationship] {
        &self.relationships
    }

    /// Relationships whose source equals `source`, in insertion order.
    #[must_use]
    pub fn relationships_from(&self, source: EntityRef) -> Vec<&Relationship> {
        self.relationships
            .iter()
            .filter(|r| r.source() == source)
            .collect()
    }

    /// Relationships whose target equals `target`, in insertion order.
    #[must_use]
    pub fn relationships_to(&self, target: EntityRef) -> Vec<&Relationship> {
        self.relationships
            .iter()
            .filter(|r| r.target() == target)
            .collect()
    }

    /// Validates that `entity` refers to an entity minted by this Scene.
    fn validate_entity_ref(&self, entity: EntityRef) -> Result<()> {
        match entity {
            EntityRef::Observation(id) => {
                if id.value() < self.observations.len() as u64 {
                    Ok(())
                } else {
                    Err(Error::not_found(
                        "Scene::add_relationship: no observation with this id in this scene",
                    ))
                }
            }
            EntityRef::DataObject(id) => {
                if id.value() < self.data_objects.len() as u64 {
                    Ok(())
                } else {
                    Err(Error::not_found(
                        "Scene::add_relationship: no data object with this id in this scene",
                    ))
                }
            }
            EntityRef::Product(id) => {
                if id.value() < self.products.len() as u64 {
                    Ok(())
                } else {
                    Err(Error::not_found(
                        "Scene::add_relationship: no product with this id in this scene",
                    ))
                }
            }
        }
    }

    /// Appends a provenance [`ProcessRecord`] and returns its new
    /// [`ProcessRecordId`].
    ///
    /// Records are immutable and append-only: each producing step adds a new
    /// record; nothing mutates or removes an existing record. The id is
    /// scoped to this scene (starting at 0, monotonic) and independent of
    /// every other id family. See the `semantic` module.
    pub fn add_process_record(&mut self, record: ProcessRecord) -> ProcessRecordId {
        let id = self.next_process_record_id;
        self.next_process_record_id += 1;
        self.process_records.push(record);
        ProcessRecordId::new(id)
    }

    /// Number of provenance process records in the scene.
    #[must_use]
    pub fn process_record_count(&self) -> usize {
        self.process_records.len()
    }

    /// Looks up a provenance [`ProcessRecord`] by its [`ProcessRecordId`].
    ///
    /// # Errors
    ///
    /// Returns [`crate::ErrorCode::NotFound`] if `id` was never returned by
    /// [`Scene::add_process_record`].
    pub fn process_record(&self, id: ProcessRecordId) -> Result<&ProcessRecord> {
        self.process_records
            .get(id.value() as usize)
            .ok_or_else(|| {
                Error::not_found("Scene::process_record: no process record with this id")
            })
    }

    /// Adds an explicit directed provenance relation
    /// `process --kind--> entity`.
    ///
    /// Provenance is a separate semantic layer from CM-02 Relationships:
    /// nothing here reads, writes, infers or converts ordinary relationships,
    /// and ordinary relationships never create provenance automatically.
    /// Relations are stored in deterministic insertion order; only explicitly
    /// declared edges exist.
    ///
    /// Validation:
    ///
    /// 1. **Membership** — `process` was minted by this Scene and `entity`
    ///    lies within this Scene's entity domain
    ///    ([`crate::ErrorCode::NotFound`] otherwise; same documented
    ///    numeric-handle limitation as relationships — `Id<Tag>` carries no
    ///    minting provenance).
    /// 2. **Duplicates** — an identical relation (same process, kind and
    ///    entity) is rejected ([`crate::ErrorCode::InvalidArgument`]).
    ///
    /// # Errors
    ///
    /// [`crate::ErrorCode::NotFound`] for foreign endpoints,
    /// [`crate::ErrorCode::InvalidArgument`] for exact duplicates.
    pub fn add_provenance_relation(
        &mut self,
        process: ProcessRecordId,
        kind: ProvenanceRelationKind,
        entity: EntityRef,
    ) -> Result<()> {
        self.validate_process_record(process)?;
        self.validate_entity_ref(entity)?;
        if self
            .provenance_relations
            .iter()
            .any(|r| r.process() == process && r.kind() == kind && r.entity() == entity)
        {
            return Err(Error::invalid_argument(
                "Scene::add_provenance_relation: duplicate relation (same process, kind and entity)",
            ));
        }
        self.provenance_relations
            .push(ProvenanceRelation::new(process, kind, entity));
        Ok(())
    }

    /// Number of provenance relations in the scene.
    #[must_use]
    pub fn provenance_relation_count(&self) -> usize {
        self.provenance_relations.len()
    }

    /// All provenance relations in deterministic insertion order.
    #[must_use]
    pub fn provenance_relations(&self) -> &[ProvenanceRelation] {
        &self.provenance_relations
    }

    /// Provenance relations of one process record, in insertion order.
    #[must_use]
    pub fn provenance_relations_of_process(
        &self,
        process: ProcessRecordId,
    ) -> Vec<&ProvenanceRelation> {
        self.provenance_relations
            .iter()
            .filter(|r| r.process() == process)
            .collect()
    }

    /// Provenance relations whose entity endpoint equals `entity`, in
    /// insertion order.
    #[must_use]
    pub fn provenance_relations_of_entity(&self, entity: EntityRef) -> Vec<&ProvenanceRelation> {
        self.provenance_relations
            .iter()
            .filter(|r| r.entity() == entity)
            .collect()
    }

    /// Validates that `process` was minted by this Scene.
    fn validate_process_record(&self, process: ProcessRecordId) -> Result<()> {
        if process.value() < self.process_records.len() as u64 {
            Ok(())
        } else {
            Err(Error::not_found(
                "Scene::add_provenance_relation: no process record with this id in this scene",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ErrorCode;

    fn desc(width: u32, height: u32) -> ImageDescriptor {
        ImageDescriptor::new(width, height)
    }

    #[test]
    fn add_image_assigns_monotonic_ids_from_zero() {
        let mut scene = Scene::new();
        assert_eq!(scene.add_image(desc(64, 32)).unwrap(), ImageId::new(0));
        assert_eq!(scene.add_image(desc(128, 64)).unwrap(), ImageId::new(1));
        assert_eq!(scene.image_count(), 2);
    }

    #[test]
    fn image_lookup_by_id() {
        let mut scene = Scene::new();
        let id = scene.add_image(desc(64, 32)).unwrap();
        let img = scene.image(id).unwrap();
        assert_eq!(img.width(), 64);
        assert_eq!(img.height(), 32);
    }

    #[test]
    fn image_by_missing_id_is_not_found() {
        let scene = Scene::new();
        let err = scene.image(ImageId::new(42)).unwrap_err();
        assert_eq!(err.code(), ErrorCode::NotFound);
    }

    #[test]
    fn image_at_in_scene_order() {
        let mut scene = Scene::new();
        scene.add_image(desc(64, 32)).unwrap();
        scene.add_image(desc(128, 64)).unwrap();
        assert_eq!(scene.image_at(0).unwrap().width(), 64);
        assert_eq!(scene.image_at(1).unwrap().width(), 128);
    }

    #[test]
    fn image_at_out_of_range() {
        let scene = Scene::new();
        let err = scene.image_at(0).unwrap_err();
        assert_eq!(err.code(), ErrorCode::OutOfRange);
    }

    #[test]
    fn add_camera_assigns_monotonic_ids_from_zero() {
        let mut scene = Scene::new();
        use crate::geometry::{Extrinsics, Intrinsics};
        use crate::geometry::{Quaternion, Vec3};
        let c0 = Camera::from_model(
            "pinhole",
            Intrinsics::new(100.0, 100.0, 8.0, 8.0),
            Extrinsics::new(Quaternion::IDENTITY, Vec3::ZERO),
            "2026-08-21T00:00:00Z",
        );
        let c1 = Camera::new();
        assert_eq!(scene.add_camera(c0), CameraId::new(0));
        assert_eq!(scene.add_camera(c1), CameraId::new(1));
        assert_eq!(scene.camera_count(), 2);
        assert_eq!(
            scene.camera(CameraId::new(0)).unwrap().model_name(),
            "pinhole"
        );
        assert_eq!(
            scene.camera(CameraId::new(1)).unwrap().model_name(),
            "pinhole"
        );
        assert_eq!(scene.camera_at(0).unwrap().model_name(), "pinhole");
    }

    #[test]
    fn camera_by_missing_id_is_not_found() {
        let scene = Scene::new();
        let err = scene.camera(CameraId::new(5)).unwrap_err();
        assert_eq!(err.code(), ErrorCode::NotFound);
    }

    #[test]
    fn camera_at_out_of_range() {
        let scene = Scene::new();
        let err = scene.camera_at(0).unwrap_err();
        assert_eq!(err.code(), ErrorCode::OutOfRange);
    }

    #[test]
    fn add_geometry_assigns_monotonic_ids_from_zero() {
        let mut scene = Scene::new();
        use crate::geometry::GeometryKind;
        assert_eq!(
            scene.add_geometry(Geometry::new(GeometryKind::Unspecified)),
            GeometryId::new(0)
        );
        assert_eq!(
            scene.add_geometry(Geometry::new(GeometryKind::Unspecified)),
            GeometryId::new(1)
        );
        assert_eq!(scene.geometry_count(), 2);
        assert!(scene.geometry(GeometryId::new(0)).is_ok());
        assert_eq!(
            scene.geometry_at(1).unwrap().kind(),
            GeometryKind::Unspecified
        );
    }

    #[test]
    fn geometry_by_missing_id_is_not_found() {
        let scene = Scene::new();
        let err = scene.geometry(GeometryId::new(3)).unwrap_err();
        assert_eq!(err.code(), ErrorCode::NotFound);
    }

    #[test]
    fn geometry_at_out_of_range() {
        let scene = Scene::new();
        let err = scene.geometry_at(0).unwrap_err();
        assert_eq!(err.code(), ErrorCode::OutOfRange);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn scene_round_trips_through_serde() {
        let mut scene = Scene::new();
        scene.add_image(desc(64, 32)).unwrap();
        scene.add_image(desc(128, 64)).unwrap();

        let json = serde_json::to_string(&scene).expect("serialize scene");
        let mut back: Scene = serde_json::from_str(&json).expect("deserialize scene");

        assert_eq!(back.image_count(), 2);
        assert_eq!(back.image_at(0).unwrap().width(), 64);
        assert_eq!(back.image_at(1).unwrap().width(), 128);

        // A fresh image wins the next id, proving the counter was round-tripped.
        let next = back.add_image(desc(10, 10)).unwrap();
        assert_eq!(next, ImageId::new(2));
    }
}
