//! The read-side document: a parsed PTIFF/TIFF file (`Document`) and a typed
//! view onto one of its images (`Image`). The NumPy-integrated tile API
//! (`Image::read_tile`) returns a real `numpy.ndarray` (Plan §9 Option B),
//! with the sample dtype and a `(tile_height, tile_width, channel_count)`
//! shape straight from the file.
//!
//! `Image` holds a strong `Py<Document>` reference plus an image index, so the
//! owning parser (and its file bytes) stays alive as long as any `Image` view
//! does; this sidesteps Rust lifetime issues across the Python boundary.

use numpy::{Element, PyArray3, PyArrayMethods};
use pyo3::exceptions::PyOSError;
use pyo3::prelude::*;
use pyo3::types::PyAny;

use crate::camera::Camera;
use crate::metadata::Metadata;

/// Parses and owns a PTIFF/TIFF/BigTIFF file.
#[pyclass(module = "ptiff_pyo3", name = "Document")]
pub struct Document {
    tiff: ptiff::Tiff,
}

impl Document {
    /// Opens `path` and validates it as a PTIFF/TIFF/BigTIFF container.
    pub(crate) fn open(path: &str) -> PyResult<Self> {
        let tiff = ptiff::Tiff::open(path)
            .map_err(|e| PyOSError::new_err(format!("cannot open {path:?}: {e}")))?;
        Ok(Self { tiff })
    }
}

/// Opens the PTIFF/TIFF/BigTIFF file at `path` for reading.
///
/// `doc` is a context manager: `with ptiff.open(path) as doc:` releases the
/// parser on exit. Each `doc.image(i)` returns a typed `Image` view that
/// shares the `Document`'s bytes and exposes the NumPy tile API.
#[pyfunction]
pub fn open(path: &str) -> PyResult<Document> {
    Document::open(path)
}

#[pymethods]
impl Document {
    /// Number of images in the file's scene (file order).
    fn image_count(&self) -> usize {
        self.tiff.scene().image_count()
    }

    /// Returns the `index`-th image of the file.
    fn image(slf: &Bound<'_, Self>, index: usize) -> PyResult<Image> {
        let tiff = &slf.borrow().tiff;
        let scene = tiff.scene();
        if index >= scene.image_count() {
            return Err(PyOSError::new_err(format!(
                "image index {index} out of range (count={})",
                scene.image_count()
            )));
        }
        Self::image_view(slf, index)
    }

    /// Yes — `Document` supports `with` for symmetry with `open`.
    fn __enter__(slf: Bound<'_, Self>) -> PyResult<Bound<'_, Self>> {
        Ok(slf)
    }

    /// Exiting the context manager: the parser is dropped on GC.
    fn __exit__(&self, _a: Py<PyAny>, _b: Py<PyAny>, _c: Py<PyAny>) -> PyResult<bool> {
        Ok(false)
    }

    /// Retrieves a `Camera` view for the `index`-th image.
    fn camera(&self, py: Python<'_>, index: usize) -> PyResult<Camera> {
        Camera::from_tiff(py, &self.tiff, index)
    }

    /// Retrieves a read-only `Metadata` view for the whole file.
    fn metadata(&self, py: Python<'_>) -> PyResult<Metadata> {
        Ok(Metadata::from_tiff(py, &self.tiff))
    }

    // Shared: builds an `Image` view bound to this `Document`.
    fn image_view(slf: &Bound<'_, Self>, index: usize) -> PyResult<Image> {
        let tiff = &slf.borrow().tiff;
        let image = tiff
            .image(index)
            .map_err(|e| PyOSError::new_err(format!("{e}")))?;
        let layout = tiff
            .tile_layout(index)
            .map_err(|e| PyOSError::new_err(format!("{e}")))?;
        Ok(Image {
            doc: slf.clone().unbind(),
            index,
            width: image.width(),
            height: image.height(),
            channel_count: image.channel_count(),
            pixel_type: image.pixel_type() as u32,
            tile_width: layout.tile_size.width,
            tile_height: layout.tile_size.height,
        })
    }
}

/// A typed view onto one image of a [`Document`]: dimensions, pixel type and
/// a NumPy tile reader.
#[pyclass(module = "ptiff_pyo3", name = "Image")]
pub struct Image {
    /// Strong reference to the owning `Document` (keeps the parser alive).
    doc: Py<Document>,
    index: usize,
    width: u32,
    height: u32,
    channel_count: u32,
    pixel_type: u32,
    tile_width: u32,
    tile_height: u32,
}

#[pymethods]
impl Image {
    /// Image width in pixels.
    #[getter]
    fn width(&self) -> u32 {
        self.width
    }

    /// Image height in pixels.
    #[getter]
    fn height(&self) -> u32 {
        self.height
    }

    /// Channels per pixel (samples/pixel).
    #[getter]
    fn channel_count(&self) -> u32 {
        self.channel_count
    }

    /// Canonical pixel sample type name (`uint8`, `uint16`, ...).
    #[getter]
    fn pixel_type(&self) -> String {
        ptiff_pixel_type_name(self.pixel_type).to_string()
    }

    /// Numeric pixel type value (PTIFF_PIXEL_* ordering).
    #[getter]
    fn pixel_type_code(&self) -> u32 {
        self.pixel_type
    }

    /// Tile width in pixels (0 when the image is not tiled).
    #[getter]
    fn tile_width(&self) -> u32 {
        self.tile_width
    }

    /// Tile height in pixels (0 when the image is not tiled).
    #[getter]
    fn tile_height(&self) -> u32 {
        self.tile_height
    }

    /// Number of tile columns covering the image.
    #[getter]
    fn tile_columns(&self, py: Python<'_>) -> PyResult<u32> {
        let doc = self.doc.bind(py).borrow();
        let layout = doc
            .tiff
            .tile_layout(self.index)
            .map_err(|e| PyOSError::new_err(format!("{e}")))?;
        Ok(layout.columns(0))
    }

    /// Number of tile rows covering the image.
    #[getter]
    fn tile_rows(&self, py: Python<'_>) -> PyResult<u32> {
        let doc = self.doc.bind(py).borrow();
        let layout = doc
            .tiff
            .tile_layout(self.index)
            .map_err(|e| PyOSError::new_err(format!("{e}")))?;
        Ok(layout.rows(0))
    }

    /// Reads one tile's samples as a `numpy.ndarray` of shape
    /// `(tile_height, tile_width, channel_count)` with the image's sample dtype.
    /// `column`/`row` index the tile grid (0-based).
    #[pyo3(signature = (column, row))]
    fn read_tile<'py>(
        &self,
        py: Python<'py>,
        column: u32,
        row: u32,
    ) -> PyResult<Bound<'py, PyAny>> {
        let doc = self.doc.bind(py).borrow();
        let bytes = doc
            .tiff
            .read_tile(self.index, column, row)
            .map_err(|e| PyOSError::new_err(format!("read_tile({column},{row}): {e}")))?;
        let ch = self.channel_count as usize;
        let h = self.tile_height as usize;
        let w = self.tile_width as usize;
        tile_to_numpy(py, self.pixel_type, &bytes, [h, w, ch])
    }

    /// Returns a `Camera` view for this image (empty if no calibration).
    fn camera(&self, py: Python<'_>) -> PyResult<Camera> {
        let doc = self.doc.bind(py).borrow();
        Camera::from_tiff(py, &doc.tiff, self.index)
    }

    fn __repr__(&self) -> String {
        format!(
            "<ptiff.Image {}x{} px={} ch={}>",
            self.width,
            self.height,
            ptiff_pixel_type_name(self.pixel_type),
            self.channel_count
        )
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }
}

/// Canonical lowercase pixel-type name matching the SWIG `_PIXEL_TYPE_NAMES`
/// table (minus camel-case) and the core's `Display`.
pub(crate) fn ptiff_pixel_type_name(code: u32) -> &'static str {
    match code {
        0 => "uint8",
        1 => "uint16",
        2 => "uint32",
        3 => "float32",
        4 => "float64",
        _ => "unknown",
    }
}

/// Copies `bytes` into a fresh 3-D `numpy.ndarray` of the sample type `code`
/// and shape `dims` (row-major, C-order).
fn tile_to_numpy<'py>(
    py: Python<'py>,
    code: u32,
    bytes: &[u8],
    dims: [usize; 3],
) -> PyResult<Bound<'py, PyAny>> {
    match code {
        0 => copy_ndarray::<u8>(py, bytes, dims),
        1 => copy_ndarray::<u16>(py, bytes, dims),
        2 => copy_ndarray::<u32>(py, bytes, dims),
        3 => copy_ndarray::<f32>(py, bytes, dims),
        4 => copy_ndarray::<f64>(py, bytes, dims),
        other => Err(PyOSError::new_err(format!(
            "unsupported pixel type code {other}"
        ))),
    }
}

/// Generic 3-D NumPy copy for one sample type.
///
/// # Safety
///
/// - `bytes` holds exactly `dims[0]*dims[1]*dims[2]` samples of `T` (guaranteed
///   by the caller's tile-byte-size invariant); casting to `&[T]` and copying
///   `n_samples` elements never reads out of bounds.
/// - `bytes` comes from a heap `Vec<u8>`, whose allocator aligns to at least
///   `max_align` (≥ any `T`'s alignment), so the cast to `*const T` is aligned.
/// - `arr` is freshly created (contiguous, C-order); `as_slice_mut` returns a
///   valid `&mut [T]` of length ≥ `n_samples`.
fn copy_ndarray<'py, T: Element + Copy>(
    py: Python<'py>,
    bytes: &[u8],
    dims: [usize; 3],
) -> PyResult<Bound<'py, PyAny>> {
    let arr = PyArray3::<T>::zeros(py, dims, false);
    let n_samples = dims[0] * dims[1] * dims[2];
    // Safety: see the # Safety section above (length + alignment).
    let src = unsafe { std::slice::from_raw_parts(bytes.as_ptr().cast::<T>(), n_samples) };
    // Safety: `arr` is contiguous C-order of exactly `n_samples` elements.
    let dst = unsafe {
        arr.as_slice_mut()
            .map_err(|_| PyOSError::new_err("tile buffer is not contiguous"))?
    };
    if dst.len() < src.len() {
        return Err(PyOSError::new_err("tile byte size mismatch"));
    }
    dst[..src.len()].copy_from_slice(src);
    Ok(arr.into_any())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ptiff::PixelType;

    #[test]
    fn pixel_type_names_cover_core_order() {
        assert_eq!(ptiff_pixel_type_name(PixelType::UInt8 as u32), "uint8");
        assert_eq!(ptiff_pixel_type_name(PixelType::UInt16 as u32), "uint16");
        assert_eq!(ptiff_pixel_type_name(PixelType::UInt32 as u32), "uint32");
        assert_eq!(ptiff_pixel_type_name(PixelType::Float32 as u32), "float32");
        assert_eq!(ptiff_pixel_type_name(PixelType::Float64 as u32), "float64");
    }
}
