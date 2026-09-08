//! MF-03B — Canonical JSON / RFC 8785 JCS Encoding Layer
//!
//! This module implements the canonical JSON encoding layer on top of the
//! corrected MF-03A typed serialization representation
//! ([`crate::mf03a::SerializationManifest`]).
//!
//! ```text
//! Manifest (semantic projection)
//!     ↓
//! MF-03A typed serialization representation
//!     ↓
//! MF-03B canonical encoder (this module)
//!     ↓
//! RFC 8785 / JCS canonical UTF-8 JSON bytes
//! ```
//!
//! # ADR-011 implementation
//!
//! RFC 8785 (JCS) canonicalization is implemented in the private `jcs`
//! submodule and applied to a typed JSON value tree produced from the
//! MF-03A representation. The encoder never bypasses MF-03A: there is no
//! direct `Manifest` → JSON mapping here.
//!
//! ADR-011 numeric policy as applied by this layer:
//!
//! * Integer values within the exact IEEE-754 integer range
//!   (`<= 2^53`) are emitted as ordinary JSON numbers.
//! * Integer values beyond `2^53` are emitted as **typed lexical decimal
//!   strings** (exact decimal digits; the integer64 type attribution is
//!   positional — every affected position is an ADR-011 integer64 position:
//!   axis `extent`, manifest-reference `id`, metadata `version`). The
//!   encoder deliberately introduces **no** wrapper-object vocabulary:
//!   exact schema spelling remains the schema phase's authority (ADR-011
//!   consequences; ADR-003/ADR-010 reference the same integer64 policy).
//! * The MF-03A representation contains no floating-point values, so no
//!   non-finite value can reach the encoder. The architecture preserves
//!   that possibility for the schema phase: a non-finite double at the
//!   value level is rejected with an explicit error (`jcs::JcsError::NonFiniteNumber`)
//!   instead of being silently degraded; the schema-governed non-finite
//!   representation is not invented here.
//!
//! # Boundaries
//!
//! * No TIFF/BigTIFF, IFD, offset, private-tag, overlay or digest concepts
//!   appear anywhere in this layer; output is identical regardless of
//!   physical placement.
//! * The JSON member names used here are the current internal names of the
//!   MF-03A representation. They are **not** presented as the ratified
//!   PTIFF 2.0 JSON Schema (exact field vocabulary remains schema-phase
//!   work per ADR-003/ADR-010/ADR-011 open items).
//! * No JSON Schema, `$schema`, `$id`, extension, revision or CBOR
//!   semantics are introduced.
//! * Decoding (canonical JSON → representation) is out of scope for this
//!   increment.
//!
//! # Canonical object member ordering
//!
//! The mapping layer emits object members in semantic (field) order;
//! RFC 8785 canonicalization determines the canonical byte order of object
//! members. Semantic array order (entities, relationships, provenance
//! relations, axes) is never reordered.

use crate::mf03a::{
    SerializationManifest, SerializationMetadata, SerializedAxisDescriptor, SerializedDataObject,
    SerializedExternalId, SerializedManifestRef, SerializedProvenanceRelation,
    SerializedRelationship,
};
use crate::Error;

mod jcs;

use jcs::{JsonNumber, JsonValue};

/// Largest integer exactly representable as an IEEE-754 double (`2^53`).
///
/// ADR-011 permits ordinary JSON numbers for integer values inside the
/// exact IEEE-754 range; values beyond this limit use the typed lexical
/// decimal-string representation.
const MAX_EXACT_IEEE754_INTEGER: u64 = 1u64 << 53;

impl SerializationManifest {
    /// Encodes this typed serialization representation as canonical JSON
    /// bytes per RFC 8785 (JCS), the PTIFF 2.0 normative encoding
    /// (ADR-011).
    ///
    /// The returned bytes are deterministic UTF-8: equal representations
    /// encode to byte-for-byte equal output, independent of object member
    /// construction order and of process-local state. Arrays (entities,
    /// relationships, provenance relations, axes) keep the semantic order
    /// established by MF-03A.
    ///
    /// Object member names in the output use the current MF-03A internal
    /// field names; they are not the ratified JSON Schema vocabulary.
    ///
    /// # Errors
    ///
    /// The current MF-03A representation contains no floating-point values,
    /// so this method cannot fail today. The `Result` follows the crate
    /// error conventions and reserves the single failure mode defined by
    /// ADR-011: a non-finite floating-point value would be reported as
    /// [`ErrorCode::NotImplemented`] because its schema-governed
    /// representation is deferred to the schema phase (no silent
    /// degradation is ever performed).
    pub fn to_canonical_json(&self) -> crate::Result<Vec<u8>> {
        encode_value(&manifest_to_json(self))
    }

    /// Encodes this typed serialization representation as a canonical JSON
    /// string (UTF-8), equivalent to
    /// [`SerializationManifest::to_canonical_json`].
    pub fn to_canonical_json_string(&self) -> crate::Result<String> {
        Ok(String::from_utf8(self.to_canonical_json()?)
            .expect("the canonical encoder only emits valid UTF-8"))
    }
}

impl SerializedExternalId {
    /// Encodes this external identifier as canonical JSON bytes per
    /// RFC 8785 (JCS), using the ADR-010 grammar
    /// `{namespace, value, version?}`.
    ///
    /// When no version is present the `version` member is omitted, as
    /// required by ADR-010; `namespace` and `value` are serialized as
    /// opaque strings and never reinterpreted.
    ///
    /// The current MF-03A [`SerializationManifest`] carries no external-id
    /// slots (semantic entities do not yet attach external identifiers), so
    /// this method encodes the identifier value object itself.
    ///
    /// # Errors
    ///
    /// Cannot fail for a well-formed [`SerializedExternalId`]; see
    /// [`SerializationManifest::to_canonical_json`] for the reserved
    /// non-finite failure mode.
    pub fn to_canonical_json(&self) -> crate::Result<Vec<u8>> {
        encode_value(&external_id_to_json(self))
    }
}

/// Encodes a JSON value through the private RFC 8785 writer.
fn encode_value(value: &JsonValue) -> crate::Result<Vec<u8>> {
    let mut out = String::new();
    jcs::write_canonical(value, &mut out).map_err(|error| match error {
        jcs::JcsError::NonFiniteNumber => Error::not_implemented(
            "canonical JSON encoding of non-finite floating-point values \
                is deferred to the schema phase (ADR-011)",
        ),
    })?;
    Ok(out.into_bytes())
}

/// Projects an integer64 value onto its ADR-011 JSON representation.
///
/// Values within the exact IEEE-754 integer range become ordinary JSON
/// numbers; larger values become exact decimal strings (typed lexical
/// representation). No information is ever lost by narrowing.
fn integer64_to_json(value: u64) -> JsonValue {
    if value <= MAX_EXACT_IEEE754_INTEGER {
        JsonValue::Number(JsonNumber::U64(value))
    } else {
        JsonValue::String(value.to_string())
    }
}

/// Projects manifest metadata.
fn metadata_to_json(metadata: &SerializationMetadata) -> JsonValue {
    JsonValue::Object(vec![(
        "version".to_owned(),
        integer64_to_json(metadata.version),
    )])
}

/// Projects a typed manifest reference `{entity, id}` (ADR-010).
fn manifest_ref_to_json(reference: &SerializedManifestRef) -> JsonValue {
    JsonValue::Object(vec![
        (
            "entity".to_owned(),
            JsonValue::String(reference.entity.as_token().to_owned()),
        ),
        ("id".to_owned(), integer64_to_json(reference.id)),
    ])
}

/// Projects an axis descriptor `{kind, extent}` (ADR-003).
fn axis_descriptor_to_json(axis: &SerializedAxisDescriptor) -> JsonValue {
    JsonValue::Object(vec![
        (
            "kind".to_owned(),
            JsonValue::String(axis.kind.as_token().to_owned()),
        ),
        ("extent".to_owned(), integer64_to_json(axis.extent)),
    ])
}

/// Projects a data object with its ordered axis declaration.
fn data_object_to_json(data_object: &SerializedDataObject) -> JsonValue {
    JsonValue::Object(vec![(
        "axes".to_owned(),
        JsonValue::Array(
            data_object
                .axes
                .iter()
                .map(axis_descriptor_to_json)
                .collect(),
        ),
    )])
}

/// Projects a typed relationship edge (ADR-009 vocabulary tokens).
fn relationship_to_json(relationship: &SerializedRelationship) -> JsonValue {
    JsonValue::Object(vec![
        (
            "source".to_owned(),
            manifest_ref_to_json(&relationship.source),
        ),
        (
            "kind".to_owned(),
            JsonValue::String(relationship.kind.as_token().to_owned()),
        ),
        (
            "target".to_owned(),
            manifest_ref_to_json(&relationship.target),
        ),
    ])
}

/// Projects a provenance relation `process → entity` (CM-03 / ADR-009).
fn provenance_relation_to_json(relation: &SerializedProvenanceRelation) -> JsonValue {
    JsonValue::Object(vec![
        (
            "process".to_owned(),
            manifest_ref_to_json(&relation.process),
        ),
        (
            "kind".to_owned(),
            JsonValue::String(relation.kind.as_token().to_owned()),
        ),
        ("entity".to_owned(), manifest_ref_to_json(&relation.entity)),
    ])
}

/// Projects an external identifier value object `{namespace, value,
/// version?}` (ADR-010 grammar).
fn external_id_to_json(external_id: &SerializedExternalId) -> JsonValue {
    let mut members = vec![
        (
            "namespace".to_owned(),
            JsonValue::String(external_id.namespace.clone()),
        ),
        (
            "value".to_owned(),
            JsonValue::String(external_id.value.clone()),
        ),
    ];
    if let Some(version) = &external_id.version {
        members.push(("version".to_owned(), JsonValue::String(version.clone())));
    }
    JsonValue::Object(members)
}

/// Projects a whole serialization manifest.
///
/// Every top-level member is always present (the collections preserve
/// count and insertion order); there is no invented optional/null rule.
fn manifest_to_json(manifest: &SerializationManifest) -> JsonValue {
    JsonValue::Object(vec![
        ("metadata".to_owned(), metadata_to_json(&manifest.metadata)),
        (
            "observations".to_owned(),
            JsonValue::Array(
                manifest
                    .observations
                    .iter()
                    // MF-03A observations are empty markers: their
                    // presence and count are the serialized information.
                    .map(|_| JsonValue::Object(Vec::new()))
                    .collect(),
            ),
        ),
        (
            "data_objects".to_owned(),
            JsonValue::Array(
                manifest
                    .data_objects
                    .iter()
                    .map(data_object_to_json)
                    .collect(),
            ),
        ),
        (
            "products".to_owned(),
            JsonValue::Array(
                manifest
                    .products
                    .iter()
                    .map(|_| JsonValue::Object(Vec::new()))
                    .collect(),
            ),
        ),
        (
            "process_records".to_owned(),
            JsonValue::Array(
                manifest
                    .process_records
                    .iter()
                    .map(|_| JsonValue::Object(Vec::new()))
                    .collect(),
            ),
        ),
        (
            "relationships".to_owned(),
            JsonValue::Array(
                manifest
                    .relationships
                    .iter()
                    .map(relationship_to_json)
                    .collect(),
            ),
        ),
        (
            "provenance_relations".to_owned(),
            JsonValue::Array(
                manifest
                    .provenance_relations
                    .iter()
                    .map(provenance_relation_to_json)
                    .collect(),
            ),
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mf03a::{SerializedEntityKind, SerializedRelationshipKind};
    use crate::scene::Scene;
    use crate::semantic::{
        AxisDescriptor, AxisKind, DataObject, EntityRef, Observation, ProcessRecord, Product,
        ProvenanceRelationKind, RelationshipKind,
    };
    use crate::ExternalId;

    /// Builds an empty serialization manifest with a given version.
    fn empty_manifest(version: u64) -> SerializationManifest {
        SerializationManifest::from_manifest(&crate::mf02::Manifest::new_empty(Some(
            crate::ManifestMetadata::new(version),
        )))
    }

    /// Canonical bytes of an empty manifest with version 0.
    fn empty_manifest_bytes() -> Vec<u8> {
        empty_manifest(0).to_canonical_json().unwrap()
    }

    /// Golden fixture: empty manifest (fixture 1).
    #[test]
    fn golden_empty_manifest() {
        let expected = "{\"data_objects\":[],\"metadata\":{\"version\":0},\
                        \"observations\":[],\"process_records\":[],\
                        \"products\":[],\"provenance_relations\":[],\
                        \"relationships\":[]}";
        assert_eq!(empty_manifest_bytes(), expected.as_bytes());
    }

    /// Golden fixture: one observation (fixture 2).
    #[test]
    fn golden_one_observation() {
        let mut scene = Scene::new();
        scene.add_observation(Observation::new());
        let serialized =
            SerializationManifest::from_manifest(&crate::mf02::Manifest::from_scene(scene));
        let expected = "{\"data_objects\":[],\"metadata\":{\"version\":0},\
                        \"observations\":[{}],\"process_records\":[],\
                        \"products\":[],\"provenance_relations\":[],\
                        \"relationships\":[]}";
        assert_eq!(serialized.to_canonical_json().unwrap(), expected.as_bytes());
    }

    /// Golden fixture: one data object with ordered axes (fixture 3).
    #[test]
    fn golden_data_object_with_axes() {
        let mut scene = Scene::new();
        let mut data_object = DataObject::new();
        data_object
            .add_axis(AxisDescriptor::new(AxisKind::X, 640).unwrap())
            .unwrap();
        data_object
            .add_axis(AxisDescriptor::new(AxisKind::Y, 480).unwrap())
            .unwrap();
        data_object
            .add_axis(AxisDescriptor::new(AxisKind::Band, 4).unwrap())
            .unwrap();
        scene.add_data_object(data_object);
        let serialized =
            SerializationManifest::from_manifest(&crate::mf02::Manifest::from_scene(scene));
        let expected = "{\"data_objects\":[{\"axes\":[{\"extent\":640,\
                        \"kind\":\"x\"},{\"extent\":480,\"kind\":\"y\"},\
                        {\"extent\":4,\"kind\":\"band\"}]}],\
                        \"metadata\":{\"version\":0},\"observations\":[],\
                        \"process_records\":[],\"products\":[],\
                        \"provenance_relations\":[],\"relationships\":[]}";
        assert_eq!(serialized.to_canonical_json().unwrap(), expected.as_bytes());
    }

    /// Golden fixture: one product (fixture 4).
    #[test]
    fn golden_one_product() {
        let mut scene = Scene::new();
        scene.add_product(Product::new());
        let serialized =
            SerializationManifest::from_manifest(&crate::mf02::Manifest::from_scene(scene));
        let expected = "{\"data_objects\":[],\"metadata\":{\"version\":0},\
                        \"observations\":[],\"process_records\":[],\
                        \"products\":[{}],\"provenance_relations\":[],\
                        \"relationships\":[]}";
        assert_eq!(serialized.to_canonical_json().unwrap(), expected.as_bytes());
    }

    /// Golden fixture: typed identity references stay domain-scoped
    /// (fixture 5) — `observation/1` and `data_object/1` remain
    /// distinguishable in the canonical bytes.
    #[test]
    fn golden_typed_identity_references() {
        let mut scene = Scene::new();
        let _obs0 = scene.add_observation(Observation::new());
        let obs1 = scene.add_observation(Observation::new());
        let _dobj0 = scene.add_data_object(DataObject::new());
        let dobj1 = scene.add_data_object(DataObject::new());
        scene
            .add_relationship(
                EntityRef::Observation(obs1),
                RelationshipKind::Produces,
                EntityRef::DataObject(dobj1),
            )
            .unwrap();
        assert_eq!(obs1.value(), 1);
        assert_eq!(dobj1.value(), 1);

        let serialized =
            SerializationManifest::from_manifest(&crate::mf02::Manifest::from_scene(scene));
        let expected = "{\"data_objects\":[{\"axes\":[]},{\"axes\":[]}],\
                        \"metadata\":{\"version\":0},\
                        \"observations\":[{},{}],\"process_records\":[],\
                        \"products\":[],\"provenance_relations\":[],\
                        \"relationships\":[{\"kind\":\"produces\",\
                        \"source\":{\"entity\":\"observation\",\"id\":1},\
                        \"target\":{\"entity\":\"data_object\",\"id\":1}}]}";
        assert_eq!(serialized.to_canonical_json().unwrap(), expected.as_bytes());
        let bytes = serialized.to_canonical_json().unwrap();
        let text = String::from_utf8(bytes).unwrap();
        // The two domain tokens are present with the same numeric id, and
        // the canonical bytes distinguish them by their entity member.
        assert!(text.contains("\"entity\":\"observation\",\"id\":1"));
        assert!(text.contains("\"entity\":\"data_object\",\"id\":1"));
    }

    /// Golden fixture: relationship kinds use the deterministic PTIFF
    /// vocabulary (fixture 6) — `produces` and `derived-from`.
    #[test]
    fn golden_relationship_vocabulary() {
        let mut scene = Scene::new();
        let dobj = scene.add_data_object(DataObject::new());
        let prod = scene.add_product(Product::new());
        scene
            .add_relationship(
                EntityRef::DataObject(dobj),
                RelationshipKind::DerivedFrom,
                EntityRef::Product(prod),
            )
            .unwrap();
        let serialized =
            SerializationManifest::from_manifest(&crate::mf02::Manifest::from_scene(scene));
        let expected = "{\"data_objects\":[{\"axes\":[]}],\"metadata\":{\"version\":0},\
                        \"observations\":[],\"process_records\":[],\
                        \"products\":[{}],\"provenance_relations\":[],\
                        \"relationships\":[{\"kind\":\"derived-from\",\
                        \"source\":{\"entity\":\"data_object\",\"id\":0},\
                        \"target\":{\"entity\":\"product\",\"id\":0}}]}";
        assert_eq!(serialized.to_canonical_json().unwrap(), expected.as_bytes());
    }

    /// Golden fixture: provenance preserves `process`/`kind`/`entity` with
    /// `used` and `generated` (fixture 7).
    #[test]
    fn golden_provenance_relations() {
        let mut scene = Scene::new();
        let dobj = scene.add_data_object(DataObject::new());
        let prod = scene.add_product(Product::new());
        let proc = scene.add_process_record(ProcessRecord::new());
        scene
            .add_provenance_relation(
                proc,
                ProvenanceRelationKind::Used,
                EntityRef::DataObject(dobj),
            )
            .unwrap();
        scene
            .add_provenance_relation(
                proc,
                ProvenanceRelationKind::Generated,
                EntityRef::Product(prod),
            )
            .unwrap();
        let serialized =
            SerializationManifest::from_manifest(&crate::mf02::Manifest::from_scene(scene));
        let expected = "{\"data_objects\":[{\"axes\":[]}],\"metadata\":{\"version\":0},\
                        \"observations\":[],\"process_records\":[{}],\
                        \"products\":[{}],\
                        \"provenance_relations\":[{\"entity\":\
                        {\"entity\":\"data_object\",\"id\":0},\
                        \"kind\":\"used\",\"process\":\
                        {\"entity\":\"process_record\",\"id\":0}},\
                        {\"entity\":{\"entity\":\"product\",\"id\":0},\
                        \"kind\":\"generated\",\"process\":\
                        {\"entity\":\"process_record\",\"id\":0}}],\
                        \"relationships\":[]}";
        assert_eq!(serialized.to_canonical_json().unwrap(), expected.as_bytes());
    }

    /// Golden fixture: external identifier with the ADR-010 grammar
    /// (fixture 8); absent versions are omitted, present versions are
    /// serialized.
    #[test]
    fn golden_external_id() {
        let plain = ExternalId::new("pds4", "urn:nasa:pds:orex:data").unwrap();
        let plain_serialized = SerializedExternalId::from(&plain);
        let expected_plain = "{\"namespace\":\"pds4\",\"value\":\"urn:nasa:pds:orex:data\"}";
        assert_eq!(
            plain_serialized.to_canonical_json().unwrap(),
            expected_plain.as_bytes()
        );

        let versioned = ExternalId::new("pds4", "urn:nasa:pds:orex:data")
            .unwrap()
            .with_version("1.0")
            .unwrap();
        let versioned_serialized = SerializedExternalId::from(&versioned);
        let expected_versioned =
            "{\"namespace\":\"pds4\",\"value\":\"urn:nasa:pds:orex:data\",\"version\":\"1.0\"}";
        assert_eq!(
            versioned_serialized.to_canonical_json().unwrap(),
            expected_versioned.as_bytes()
        );
    }

    /// Golden fixture: integers beyond 2^53 use the typed lexical decimal
    /// representation instead of a lossy JSON number (fixture 9).
    #[test]
    fn golden_large_integer_lexical_representation() {
        let huge: u64 = 9_007_199_254_740_993; // 2^53 + 1
        let mut scene = Scene::new();
        let mut data_object = DataObject::new();
        data_object
            .add_axis(AxisDescriptor::new(AxisKind::X, huge).unwrap())
            .unwrap();
        scene.add_data_object(data_object);
        let serialized =
            SerializationManifest::from_manifest(&crate::mf02::Manifest::from_scene(scene));
        let expected = "{\"data_objects\":[{\"axes\":[{\"extent\":\
                        \"9007199254740993\",\"kind\":\"x\"}]}],\
                        \"metadata\":{\"version\":0},\"observations\":[],\
                        \"process_records\":[],\"products\":[],\
                        \"provenance_relations\":[],\"relationships\":[]}";
        assert_eq!(serialized.to_canonical_json().unwrap(), expected.as_bytes());
    }

    /// The 2^53 boundary itself is exactly representable and stays an
    /// ordinary JSON number; only values beyond it become lexical.
    #[test]
    fn integer_boundary_at_2_power_53() {
        let boundary: u64 = 9_007_199_254_740_992; // 2^53
        let beyond: u64 = 9_007_199_254_740_993; // 2^53 + 1
        let mut scene = Scene::new();
        let mut boundary_object = DataObject::new();
        boundary_object
            .add_axis(AxisDescriptor::new(AxisKind::X, boundary).unwrap())
            .unwrap();
        scene.add_data_object(boundary_object);
        let mut beyond_object = DataObject::new();
        beyond_object
            .add_axis(AxisDescriptor::new(AxisKind::Band, beyond).unwrap())
            .unwrap();
        scene.add_data_object(beyond_object);
        let serialized =
            SerializationManifest::from_manifest(&crate::mf02::Manifest::from_scene(scene));
        let bytes = serialized.to_canonical_json().unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.contains("\"extent\":9007199254740992"));
        assert!(text.contains("\"extent\":\"9007199254740993\""));
    }

    /// Golden fixture: combined representative manifest (fixture 10) with
    /// metadata version, one observation, two data objects (one with
    /// axes), one product, one process record, both relationship kinds and
    /// both provenance kinds.
    #[test]
    fn golden_combined_representative_manifest() {
        let mut scene = Scene::new();
        let obs = scene.add_observation(Observation::new());
        let mut data_object = DataObject::new();
        data_object
            .add_axis(AxisDescriptor::new(AxisKind::X, 640).unwrap())
            .unwrap();
        data_object
            .add_axis(AxisDescriptor::new(AxisKind::Y, 480).unwrap())
            .unwrap();
        data_object
            .add_axis(AxisDescriptor::new(AxisKind::Band, 4).unwrap())
            .unwrap();
        let dobj = scene.add_data_object(data_object);
        let _empty_dobj = scene.add_data_object(DataObject::new());
        let prod = scene.add_product(Product::new());
        let proc = scene.add_process_record(ProcessRecord::new());
        scene
            .add_relationship(
                EntityRef::Observation(obs),
                RelationshipKind::Produces,
                EntityRef::DataObject(dobj),
            )
            .unwrap();
        scene
            .add_relationship(
                EntityRef::DataObject(dobj),
                RelationshipKind::DerivedFrom,
                EntityRef::Product(prod),
            )
            .unwrap();
        scene
            .add_provenance_relation(
                proc,
                ProvenanceRelationKind::Used,
                EntityRef::DataObject(dobj),
            )
            .unwrap();
        scene
            .add_provenance_relation(
                proc,
                ProvenanceRelationKind::Generated,
                EntityRef::Product(prod),
            )
            .unwrap();

        let manifest = crate::mf02::Manifest::from_scene(scene);
        let serialized = SerializationManifest::from_manifest(&manifest);
        let expected = "{\"data_objects\":[{\"axes\":[{\"extent\":640,\
                        \"kind\":\"x\"},{\"extent\":480,\"kind\":\"y\"},\
                        {\"extent\":4,\"kind\":\"band\"}]},\
                        {\"axes\":[]}],\
                        \"metadata\":{\"version\":0},\"observations\":[{}],\
                        \"process_records\":[{}],\"products\":[{}],\
                        \"provenance_relations\":[{\"entity\":\
                        {\"entity\":\"data_object\",\"id\":0},\
                        \"kind\":\"used\",\"process\":\
                        {\"entity\":\"process_record\",\"id\":0}},\
                        {\"entity\":{\"entity\":\"product\",\"id\":0},\
                        \"kind\":\"generated\",\"process\":\
                        {\"entity\":\"process_record\",\"id\":0}}],\
                        \"relationships\":[{\"kind\":\"produces\",\
                        \"source\":{\"entity\":\"observation\",\"id\":0},\
                        \"target\":{\"entity\":\"data_object\",\"id\":0}},\
                        {\"kind\":\"derived-from\",\
                        \"source\":{\"entity\":\"data_object\",\"id\":0},\
                        \"target\":{\"entity\":\"product\",\"id\":0}}]}";
        assert_eq!(serialized.to_canonical_json().unwrap(), expected.as_bytes());
    }

    /// Metadata version is serialized faithfully (never turned into a JSON
    /// Schema version); large versions use the typed lexical form.
    #[test]
    fn metadata_version_is_serialized_faithfully() {
        let text_small = String::from_utf8(empty_manifest(7).to_canonical_json().unwrap()).unwrap();
        assert!(text_small.contains("\"version\":7"));
        assert!(!text_small.contains("$schema"));

        let huge_version = 9_007_199_254_740_993u64;
        let manifest = empty_manifest(huge_version);
        let bytes = manifest.to_canonical_json().unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.contains("\"version\":\"9007199254740993\""));
    }

    /// Every integer64 position (metadata version, reference id, axis
    /// extent) obeys the typed lexical policy for values beyond 2^53.
    #[test]
    fn all_integer64_positions_use_lexical_form_beyond_2_power_53() {
        let huge = 9_007_199_254_740_993u64;
        let manifest = SerializationManifest {
            metadata: SerializationMetadata { version: huge },
            observations: Vec::new(),
            data_objects: Vec::new(),
            products: Vec::new(),
            relationships: vec![SerializedRelationship {
                source: SerializedManifestRef {
                    entity: SerializedEntityKind::DataObject,
                    id: huge,
                },
                kind: SerializedRelationshipKind::DerivedFrom,
                target: SerializedManifestRef {
                    entity: SerializedEntityKind::Product,
                    id: 0,
                },
            }],
            process_records: Vec::new(),
            provenance_relations: Vec::new(),
        };
        let bytes = manifest.to_canonical_json().unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.contains("\"version\":\"9007199254740993\""));
        assert!(text.contains("\"id\":\"9007199254740993\""));
        assert!(!text.contains("9007199254740993,"));
    }

    /// Canonical byte determinism (fixture part of RFC 8785 §7 of the
    /// directive): encode(input) == encode(input), and equivalent semantic
    /// construction paths produce identical bytes.
    #[test]
    fn canonical_bytes_are_deterministic() {
        // Same representation twice.
        let manifest = empty_manifest(3);
        assert_eq!(
            manifest.to_canonical_json().unwrap(),
            manifest.to_canonical_json().unwrap()
        );

        // Equivalent construction paths: two independently built scenes
        // with the same semantic content produce identical bytes.
        let mut scene_a = Scene::new();
        scene_a.add_observation(Observation::new());
        let mut data_object = DataObject::new();
        data_object
            .add_axis(AxisDescriptor::new(AxisKind::Y, 512).unwrap())
            .unwrap();
        scene_a.add_data_object(data_object);
        scene_a.add_product(Product::new());

        let mut scene_b = Scene::new();
        scene_b.add_observation(Observation::new());
        let mut data_object_b = DataObject::new();
        data_object_b
            .add_axis(AxisDescriptor::new(AxisKind::Y, 512).unwrap())
            .unwrap();
        scene_b.add_data_object(data_object_b);
        scene_b.add_product(Product::new());

        let manifest_a =
            SerializationManifest::from_manifest(&crate::mf02::Manifest::from_scene(scene_a));
        let manifest_b =
            SerializationManifest::from_manifest(&crate::mf02::Manifest::from_scene(scene_b));
        assert_eq!(
            manifest_a.to_canonical_json().unwrap(),
            manifest_b.to_canonical_json().unwrap()
        );
    }

    /// The encoder output is valid UTF-8 and contains no JSON whitespace.
    #[test]
    fn output_is_compact_utf8() {
        let bytes = empty_manifest(1).to_canonical_json().unwrap();
        assert!(std::str::from_utf8(&bytes).is_ok());
        assert!(!bytes.contains(&b' '));
        assert!(!bytes.contains(&b'\n'));
        assert!(!bytes.contains(&b'\t'));
    }

    /// Object member names of the mapping use the MF-03A internal names and
    /// never introduce JSON Schema vocabulary.
    #[test]
    fn mapping_introduces_no_schema_vocabulary() {
        let text = String::from_utf8(empty_manifest(0).to_canonical_json().unwrap()).unwrap();
        assert!(!text.contains("$schema"));
        assert!(!text.contains("$id"));
        assert!(!text.contains("ptiff."));
    }
}
