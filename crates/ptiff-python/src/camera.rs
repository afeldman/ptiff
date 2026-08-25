//! Structured camera calibration / pose (`Camera`), read from a parsed image's
//! PTIFF extension domain (tag 65002). Mirrors the SWIG `ptiff.camera.Camera`
//! attribute surface: pinhole intrinsics, quaternion extrinsics + translation,
//! an ISO-8601 timestamp, the derived K / [R|t] / P matrices, and a model name.

use pyo3::prelude::*;

/// A structured camera calibration/pose view for one image.
/// A structured camera calibration/pose view for one image.
#[pyclass(module = "ptiff_pyo3", name = "Camera", from_py_object)]
#[derive(Clone)]
pub struct Camera {
    #[pyo3(get, set)]
    model: String,
    #[pyo3(get, set)]
    has_intrinsics: bool,
    #[pyo3(get, set)]
    focal_length_x: f64,
    #[pyo3(get, set)]
    focal_length_y: f64,
    #[pyo3(get, set)]
    principal_x: f64,
    #[pyo3(get, set)]
    principal_y: f64,
    #[pyo3(get, set)]
    rotation_w: f64,
    #[pyo3(get, set)]
    rotation_x: f64,
    #[pyo3(get, set)]
    rotation_y: f64,
    #[pyo3(get, set)]
    rotation_z: f64,
    #[pyo3(get, set)]
    position_x: f64,
    #[pyo3(get, set)]
    position_y: f64,
    #[pyo3(get, set)]
    position_z: f64,
    #[pyo3(get, set)]
    timestamp: String,
    // Lens (distortion) model: canonical kind + named double parameters.
    #[pyo3(get, set)]
    lens_kind: String,
    lens_parameters: std::collections::BTreeMap<String, f64>,
    // Read-derived matrices (computed, not settable directly).
    intrinsics: [f64; 9],
    extrinsics: [f64; 12],
    projection: [f64; 12],
    has_extrinsics: bool,
}

impl Camera {
    /// Builds a `Camera` from the `index`-th image of `tiff`.
    ///
    /// When the image carries no PTIFF camera calibration, a default (empty)
    /// `Camera` is returned with all `has_*` flags false — uniform with the
    /// SWIG surface.
    pub(crate) fn from_tiff(_py: Python<'_>, tiff: &ptiff::Tiff, index: usize) -> PyResult<Camera> {
        let image = tiff
            .image(index)
            .map_err(|e| pyo3::exceptions::PyOSError::new_err(format!("{e}")))?;
        let Some(cam) = image.camera() else {
            return Ok(Camera::default());
        };
        Ok(Camera::from_core(cam))
    }

    /// Converts a core `ptiff::Camera` into the Python view.
    fn from_core(cam: &ptiff::Camera) -> Camera {
        let intrin = cam.intrinsics();
        let extrin = cam.extrinsics();
        let rot = extrin.rotation;
        let pos = extrin.translation;
        let has_intrinsics = intrin != ptiff::Intrinsics::ZERO;
        let has_extrinsics = extrin != ptiff::Extrinsics::IDENTITY;
        let lens = cam.lens_model();
        let lens_parameters = lens.iter().map(|(k, v)| (k.to_string(), v)).collect();
        Camera {
            model: cam.model_name().to_string(),
            has_intrinsics,
            focal_length_x: intrin.focal_length_pixels_x,
            focal_length_y: intrin.focal_length_pixels_y,
            principal_x: intrin.principal_point_x,
            principal_y: intrin.principal_point_y,
            intrinsics: cam.intrinsics_matrix(),
            has_extrinsics,
            rotation_w: rot.w,
            rotation_x: rot.x,
            rotation_y: rot.y,
            rotation_z: rot.z,
            position_x: pos.x,
            position_y: pos.y,
            position_z: pos.z,
            extrinsics: cam.extrinsics_matrix(),
            projection: cam.projection_matrix(),
            timestamp: cam.timestamp().to_string(),
            lens_kind: ptiff::lens_model_kind_str(lens.kind()).to_string(),
            lens_parameters,
        }
    }
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            model: String::new(),
            has_intrinsics: false,
            focal_length_x: 0.0,
            focal_length_y: 0.0,
            principal_x: 0.0,
            principal_y: 0.0,
            intrinsics: [0.0; 9],
            has_extrinsics: false,
            rotation_w: 0.0,
            rotation_x: 0.0,
            rotation_y: 0.0,
            rotation_z: 0.0,
            position_x: 0.0,
            position_y: 0.0,
            position_z: 0.0,
            extrinsics: [0.0; 12],
            projection: [0.0; 12],
            timestamp: String::new(),
            lens_kind: "pinhole".to_string(),
            lens_parameters: std::collections::BTreeMap::new(),
        }
    }
}

#[pymethods]
impl Camera {
    /// Constructs a (possibly default) `Camera`.
    ///
    /// All parameters are keyword arguments; each defaults to the "unset"
    /// value. Set `focal_length_x`, `focal_length_y`, `principal_x`,
    /// `principal_y`, `rotation_*`, `position_*` and/or `timestamp` to build a
    /// calibration to persist, or call `doc.camera(i)` to read one back.
    #[new]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (*, model="pinhole".to_string(), focal_length_x=0.0, focal_length_y=0.0, principal_x=0.0, principal_y=0.0, rotation_w=1.0, rotation_x=0.0, rotation_y=0.0, rotation_z=0.0, position_x=0.0, position_y=0.0, position_z=0.0, timestamp="".to_string()))]
    fn new(
        model: String,
        focal_length_x: f64,
        focal_length_y: f64,
        principal_x: f64,
        principal_y: f64,
        rotation_w: f64,
        rotation_x: f64,
        rotation_y: f64,
        rotation_z: f64,
        position_x: f64,
        position_y: f64,
        position_z: f64,
        timestamp: String,
    ) -> PyResult<Camera> {
        let has_intrinsics = focal_length_x != 0.0 || focal_length_y != 0.0;
        let has_extrinsics = rotation_x != 0.0
            || rotation_y != 0.0
            || rotation_z != 0.0
            || rotation_w != 1.0
            || position_x != 0.0
            || position_y != 0.0
            || position_z != 0.0;
        Ok(Camera {
            model,
            has_intrinsics,
            has_extrinsics,
            focal_length_x,
            focal_length_y,
            principal_x,
            principal_y,
            rotation_w,
            rotation_x,
            rotation_y,
            rotation_z,
            position_x,
            position_y,
            position_z,
            intrinsics: [0.0; 9],
            extrinsics: [0.0; 12],
            projection: [0.0; 12],
            timestamp,
            lens_kind: "pinhole".to_string(),
            lens_parameters: std::collections::BTreeMap::new(),
        })
    }

    /// Whether pinhole intrinsics are present (derived from the file).
    #[getter]
    fn has_intrinsics(&self) -> bool {
        self.has_intrinsics
    }

    /// Whether extrinsic pose (rotation + position) is present.
    #[getter]
    fn has_extrinsics(&self) -> bool {
        self.has_extrinsics
    }

    /// The 3×3 intrinsic matrix `K` (row-major).
    fn intrinsics_matrix(&self) -> Vec<f64> {
        self.intrinsics.to_vec()
    }

    /// The 3×4 extrinsic matrix `[R|t]` (row-major).
    fn extrinsics_matrix(&self) -> Vec<f64> {
        self.extrinsics.to_vec()
    }

    /// The 3×4 projection matrix `P = K·[R|t]` (row-major).
    fn projection_matrix(&self) -> Vec<f64> {
        self.projection.to_vec()
    }

    /// The name of the projection kind for the attached model
    /// (`"pinhole"`, `"fisheye"`, `"pushbroom"`).
    #[getter]
    fn lens_kind(&self) -> String {
        self.lens_kind.clone()
    }

    /// The named double parameters of the lens (distortion) model (e.g.
    /// `k1`/`k2`/`p1`), as an insertion-ordered `{key: value}` mapping.
    fn lens_parameters(&self) -> std::collections::BTreeMap<String, f64> {
        self.lens_parameters.clone()
    }

    /// Sets (or replaces) one named lens (distortion) parameter.
    fn set_lens_parameter(&mut self, key: String, value: f64) {
        self.lens_parameters.insert(key, value);
    }

    fn __repr__(&self) -> String {
        format!(
            "<ptiff.Camera model={} intrinsics={} extrinsics={} lens={}>",
            self.model, self.has_intrinsics, self.has_extrinsics, self.lens_kind
        )
    }
}

impl Camera {
    /// Marshals this Python view into a core `ptiff::Camera` for writing.
    pub(crate) fn into_core(self) -> ptiff::Camera {
        let intrinsics = ptiff::Intrinsics::new(
            self.focal_length_x,
            self.focal_length_y,
            self.principal_x,
            self.principal_y,
        );
        // A "no rotation" (identity) pose is `w=1, (x,y,z)=0`; if the caller
        // only set x/y/z or nothing, normalise to a valid unit quaternion.
        let has_rotation = self.rotation_x != 0.0
            || self.rotation_y != 0.0
            || self.rotation_z != 0.0
            || self.rotation_w != 0.0;
        let rotation = ptiff::Quaternion::new(
            if has_rotation { self.rotation_w } else { 1.0 },
            self.rotation_x,
            self.rotation_y,
            self.rotation_z,
        );
        let translation = ptiff::Vec3::new(self.position_x, self.position_y, self.position_z);
        let extrinsics = ptiff::Extrinsics::new(rotation, translation);
        let mut lens = ptiff::LensModel::new(ptiff::lens_model_kind_from_str(&self.lens_kind));
        for (k, v) in &self.lens_parameters {
            lens.set_parameter(k.clone(), *v);
        }
        ptiff::Camera::from_model_with_lens(
            if self.model.is_empty() {
                "pinhole"
            } else {
                &self.model
            },
            intrinsics,
            extrinsics,
            self.timestamp.clone(),
            lens,
        )
    }
}
