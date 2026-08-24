//! Read-only metadata of the file (dimensions, pixel type, tile layout and the
//! flattened `ptiff.<domain>.<key>` extension fields). Primarily the generic
//! extension domains (SPICE/scientific-layers/provenance, i.e. the "PDS layer")
//! reachable via `Image::metadata`, matching what the SWIG surface exposes
//! through `ptiff_open_path_fields`.

use pyo3::prelude::*;
use pyo3::types::PyDict;

/// Read-only metadata view over the file's primary image.
#[pyclass(module = "ptiff_pyo3", name = "Metadata", skip_from_py_object)]
#[derive(Clone, Default)]
pub struct Metadata {
    width: u32,
    height: u32,
    channel_count: u32,
    pixel_type_name: String,
    pixel_type_code: u32,
    tile_width: u32,
    tile_height: u32,
    fields: std::collections::BTreeMap<String, String>,
}

impl Metadata {
    /// Builds a `Metadata` view from a parsed `Tiff`.
    pub(crate) fn from_tiff(py: Python<'_>, tiff: &ptiff::Tiff) -> Metadata {
        let scene = tiff.scene();
        let mut m = Metadata::default();
        if let Ok(image) = scene.image_at(0) {
            m.width = image.width();
            m.height = image.height();
            m.channel_count = image.channel_count();
            m.pixel_type_code = image.pixel_type() as u32;
            m.pixel_type_name =
                crate::document::ptiff_pixel_type_name(m.pixel_type_code).to_string();
            if let Ok(layout) = tiff.tile_layout(0) {
                m.tile_width = layout.tile_size.width;
                m.tile_height = layout.tile_size.height;
            }
            // The generic `ptiff.<domain>.<key>` extension fields (SPICE,
            // scientific layers, provenance — the PDS layer). Camera/CRS are
            // structured domains exposed via `Image.camera`, not flattened here.
            m.fields = image.metadata().clone();
        }
        let _ = py;
        m
    }

    /// The flattened extension fields as an ordered `key -> value` dict.
    pub(crate) fn fields_pydict<'py>(&self, py: Python<'py>) -> Bound<'py, PyDict> {
        let d = PyDict::new(py);
        for (k, v) in &self.fields {
            let _ = d.set_item(k, v);
        }
        d
    }
}

#[pymethods]
impl Metadata {
    #[getter]
    fn width(&self) -> u32 {
        self.width
    }
    #[getter]
    fn height(&self) -> u32 {
        self.height
    }
    #[getter]
    fn channel_count(&self) -> u32 {
        self.channel_count
    }
    #[getter]
    fn pixel_type(&self) -> u32 {
        self.pixel_type_code
    }
    #[getter]
    fn pixel_type_name(&self) -> String {
        self.pixel_type_name.clone()
    }
    #[getter]
    fn tile_width(&self) -> u32 {
        self.tile_width
    }
    #[getter]
    fn tile_height(&self) -> u32 {
        self.tile_height
    }

    /// The flattened `ptiff.<domain>.<key>` extension fields.
    #[getter]
    fn fields<'py>(&self, py: Python<'py>) -> Bound<'py, PyDict> {
        self.fields_pydict(py)
    }

    /// Returns a single extension field value, or `None` when absent.
    #[pyo3(signature = (key, default = None))]
    fn field(&self, key: &str, default: Option<String>) -> Option<String> {
        self.fields.get(key).cloned().or(default)
    }
}
