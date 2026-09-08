//! MF-03A — Typed Manifest Serialization Representation Layer
//!
//! This module establishes the typed intermediate representation between the
//! semantic `Manifest` (MF-02) and the future canonical JSON/JCS encoder (MF-03B).
//!
//! It is a **mapping layer**, not an encoder. No JSON bytes are produced here.

use crate::semantic::{DataObject, Observation, ProcessRecord, Product, Relationship};
use crate::ManifestMetadata;

/// A typed serialization representation of a Manifest.
#[derive(Debug, Clone)]
pub struct SerializationManifest {
    pub metadata: Option<SerializationMetadata>,
    pub observations: Vec<SerializedObservation>,
    pub data_objects: Vec<SerializedDataObject>,
    pub products: Vec<SerializedProduct>,
    pub relationships: Vec<SerializedRelationship>,
    pub process_records: Vec<SerializedProcessRecord>,
    pub provenance_relations: Vec<SerializedProvenanceRelation>,
}

/// Manifest metadata for serialization.
#[derive(Debug, Clone)]
pub struct SerializationMetadata {
    pub version: u64,
}

/// A serialized observation entity.
#[derive(Debug, Clone)]
pub struct SerializedObservation {}

/// A serialized data object with axes declaration (per ADR-003).
#[derive(Debug, Clone)]
pub struct SerializedDataObject {
    pub axes: Vec<SerializedAxisDescriptor>,
}

/// An axis descriptor for serialization representation (per ADR-003).
#[derive(Debug, Clone)]
pub struct SerializedAxisDescriptor {
    pub kind: String,
    pub extent: u64,
}

/// A serialized product entity.
#[derive(Debug, Clone)]
pub struct SerializedProduct {}

/// A serialized process record for provenance (CM-03).
#[derive(Debug, Clone)]
pub struct SerializedProcessRecord {}

/// A serialized relationship with endpoints and kind.
#[derive(Debug, Clone)]
pub struct SerializedRelationship {
    pub source: u64,
    pub kind: String,
    pub target: u64,
}

/// A serialized provenance relation with kind and endpoints.
#[derive(Debug, Clone)]
pub struct SerializedProvenanceRelation {
    pub kind: String,
    pub source: u64,
    pub target: u64,
}

impl SerializationManifest {
    /// Converts a `Manifest` (MF-02) into a typed serialization representation.
    pub fn from_manifest(manifest: &crate::mf02::Manifest) -> Self {
        let metadata = Some(SerializationMetadata {
            version: manifest.metadata.version(),
        });

        let observations: Vec<SerializedObservation> = manifest
            .observations
            .iter()
            .map(|_| SerializedObservation {})
            .collect();
        let data_objects: Vec<SerializedDataObject> = manifest
            .data_objects
            .iter()
            .enumerate()
            .map(|(i, dobj)| {
                let axes: Vec<SerializedAxisDescriptor> = dobj
                    .axes()
                    .iter()
                    .map(|ax| SerializedAxisDescriptor {
                        kind: format!("{:?}", ax.kind()),
                        extent: ax.extent(),
                    })
                    .collect();
                SerializedDataObject { axes }
            })
            .collect();

        let products: Vec<SerializedProduct> = manifest
            .products
            .iter()
            .map(|_| SerializedProduct {})
            .collect();
        let process_records: Vec<SerializedProcessRecord> = manifest
            .process_records
            .iter()
            .map(|_| SerializedProcessRecord {})
            .collect();

        let relationships: Vec<SerializedRelationship> = manifest
            .relationships
            .iter()
            .enumerate()
            .map(|(rel_index, rel)| {
                let kind = format!("{:?}", rel.kind());
                SerializedRelationship {
                    source: 0u64,
                    kind,
                    target: 0u64,
                }
            })
            .collect();

        let provenance_relations: Vec<SerializedProvenanceRelation> = manifest
            .provenance_relations
            .iter()
            .enumerate()
            .map(|(rel_index, rel)| SerializedProvenanceRelation {
                kind: format!("{:?}", rel.kind()),
                source: 0u64,
                target: 0u64,
            })
            .collect();

        Self {
            metadata,
            observations,
            data_objects,
            products,
            relationships,
            process_records,
            provenance_relations,
        }
    }
}
