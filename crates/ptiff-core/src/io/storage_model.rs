//! Format-neutral intermediate representation of a document to store.
//!
//! Mirrors `ptiff::io::StorageModel` (see
//! `libptiff/include/ptiff/io/storage_model.hpp`).

use std::collections::BTreeMap;

use crate::{Error, Result};

/// Format-neutral intermediate representation of a document to store.
///
/// `StorageModel` is the in-memory, binary-free payload a `Serializer` produces
/// from a `Scene` and a `StorageBackend` later turns into (or back out of)
/// bytes. It holds only structured fields (a key/value table of strings) and
/// nested child nodes — one child per image / camera / layer / ... . No binary
/// data lives here.
///
/// **Value semantics:** a plain owned value type (cheap to move around a scene
/// tree). The children form an owning tree (each child is itself a
/// `StorageModel`).
///
/// **Thread-safety:** thread-compatible — safe to read concurrently, not safe
/// to mutate concurrently with any other access.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StorageModel {
    fields: BTreeMap<String, String>,
    children: Vec<StorageModel>,
}

impl StorageModel {
    /// Default-constructs an empty storage model (no fields, no children).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets (or overwrites) a scalar string field.
    ///
    /// @example `m.set_field("model", "HiRISE"); m.set_field("model", "CTX")`
    ///          overwrites, leaving `field("model") == "CTX"`.
    pub fn set_field(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.fields.insert(key.into(), value.into());
    }

    /// Reads a scalar string field.
    ///
    /// # Errors
    ///
    /// Returns [`crate::ErrorCode::NotFound`] if `key` was never set.
    pub fn field(&self, key: &str) -> Result<&str> {
        self.fields
            .get(key)
            .map(String::as_str)
            .ok_or_else(|| Error::not_found(format!("StorageModel: no field '{key}'")))
    }

    /// Invokes `f` for every `(key, value)` field pair, in ascending key order.
    ///
    /// Enumerates the node's scalar string fields exactly once each, ordered
    /// lexicographically by key (the map's stable ordering). This is the general
    /// way to persist a `StorageModel` tree without naming the fields a priori
    /// (e.g. a serialization codec).
    pub fn for_each_field(&self, mut f: impl FnMut(&str, &str)) {
        for (k, v) in &self.fields {
            f(k, v);
        }
    }

    /// Returns the number of scalar fields in this node.
    #[must_use]
    pub fn field_count(&self) -> usize {
        self.fields.len()
    }

    /// Appends a nested child node.
    ///
    /// One child typically corresponds to one image / camera / layer the
    /// serializer emits.
    pub fn add_child(&mut self, child: StorageModel) {
        self.children.push(child);
    }

    /// Returns a slice over this model's child nodes.
    ///
    /// Valid for this model's lifetime and invalidated by `add_child`.
    #[must_use]
    pub fn children(&self) -> &[StorageModel] {
        &self.children
    }

    /// Returns the number of children in this node.
    #[must_use]
    pub fn child_count(&self) -> usize {
        self.children.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ErrorCode;

    #[test]
    fn set_and_read_field() {
        let mut m = StorageModel::new();
        m.set_field("width", "64");
        assert_eq!(m.field("width").unwrap(), "64");
        assert_eq!(m.field_count(), 1);
    }

    #[test]
    fn set_field_overwrites() {
        let mut m = StorageModel::new();
        m.set_field("model", "HiRISE");
        m.set_field("model", "CTX");
        assert_eq!(m.field("model").unwrap(), "CTX");
        assert_eq!(m.field_count(), 1);
    }

    #[test]
    fn missing_field_is_not_found() {
        let m = StorageModel::new();
        let err = m.field("missing").unwrap_err();
        assert_eq!(err.code(), ErrorCode::NotFound);
    }

    #[test]
    fn for_each_field_is_ascending_key_order() {
        let mut m = StorageModel::new();
        m.set_field("b", "2");
        m.set_field("a", "1");
        m.set_field("c", "3");
        let mut order = Vec::new();
        m.for_each_field(|k, v| order.push((k.to_string(), v.to_string())));
        // Insertion was b, a, c — but iteration must be lexicographic a, b, c.
        assert_eq!(
            order,
            vec![
                ("a".to_string(), "1".to_string()),
                ("b".to_string(), "2".to_string()),
                ("c".to_string(), "3".to_string()),
            ]
        );
    }

    #[test]
    fn children_form_an_owning_tree() {
        let mut scene = StorageModel::new();
        let mut image = StorageModel::new();
        image.set_field("width", "64");
        let mut camera = StorageModel::new();
        camera.set_field("name", "left");

        scene.add_child(image);
        scene.add_child(camera);

        assert_eq!(scene.child_count(), 2);
        assert_eq!(scene.children()[0].field("width").unwrap(), "64");
        assert_eq!(scene.children()[1].field("name").unwrap(), "left");
    }
}
