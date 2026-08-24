//! Write side: a `Sink` that accumulates tiles into a raster and flushes a
//! complete TIFF/BigTIFF through `ptiff::Tiff::to_bytes_with_pixels` on close.
//! Mirrors the C ABI's `ptiff_sink_*` contract (tiled images only; tile I/O
//! in any order; file not valid until `close`).

use pyo3::exceptions::{PyOSError, PyValueError};
use pyo3::prelude::*;

use crate::camera::Camera;

/// An open write handle for one image.
#[pyclass(module = "ptiff_pyo3", name = "Sink")]
pub struct Sink {
    path: std::path::PathBuf,
    scene: ptiff::Scene,
    columns: usize,
    rows: usize,
    tile_bytes: usize,
    raster: Vec<u8>,
    written: usize,
    closed: bool,
}

/// Creates a `Sink` for a new TIFF/BigTIFF at `path` describing one image.
///
/// `pixel_type` is a `PTIFF_PIXEL_*` code, `compression` a
/// `PTIFF_COMPRESSION_*` code (0 = none). The image must be tiled
/// (`tile_width`/`tile_height` > 0). `camera` is an optional `Camera` whose
/// calibration is persisted into the file.
#[pyfunction]
#[allow(clippy::too_many_arguments)]
#[pyo3(signature = (path, width, height, pixel_type=0, channel_count=1, tile_width=16, tile_height=16, compression=0, camera=None))]
pub fn create_image(
    path: &str,
    width: u32,
    height: u32,
    pixel_type: u32,
    channel_count: u32,
    tile_width: u32,
    tile_height: u32,
    compression: u32,
    camera: Option<Camera>,
) -> PyResult<Sink> {
    if tile_width == 0 || tile_height == 0 {
        return Err(PyValueError::new_err(
            "create_image requires a tiled layout (tile_width/tile_height > 0)",
        ));
    }
    if !(1..=3).contains(&channel_count) {
        return Err(PyValueError::new_err(
            "channel_count must be 1 or 3 (PTIFF tile sample layout)",
        ));
    }

    let mut descriptor = ptiff::ImageDescriptor::new(width, height);
    descriptor.pixel_type = normalize_pixel_type(pixel_type)?;
    descriptor.channel_count = channel_count;
    descriptor.tile_info = Some(ptiff::TileInfo::new(tile_width, tile_height));
    descriptor.compression = match compression {
        0 => Some(ptiff::CompressionKind::None),
        1 => Some(ptiff::CompressionKind::Lzw),
        2 => Some(ptiff::CompressionKind::Deflate),
        3 => Some(ptiff::CompressionKind::Jpeg),
        other => {
            return Err(PyValueError::new_err(format!(
                "unknown compression code {other}"
            )))
        }
    };
    if let Some(cam) = camera {
        descriptor.camera = Some(cam.into_core());
    }

    let bps = descriptor.pixel_type.bytes_per_sample();
    let tile_bytes =
        (tile_width as usize) * (tile_height as usize) * (channel_count as usize) * bps;

    let layout = ptiff::TileLayout::new(
        ptiff::TileExtent::new(tile_width, tile_height),
        width,
        height,
        1,
    );
    let columns = layout.columns(0) as usize;
    let rows = layout.rows(0) as usize;
    if columns == 0 || rows == 0 {
        return Err(PyValueError::new_err("bad tile/image dimensions"));
    }

    let mut scene = ptiff::Scene::new();
    scene
        .add_image(descriptor)
        .map_err(|e| PyOSError::new_err(format!("{e}")))?;

    Ok(Sink {
        path: std::path::PathBuf::from(path),
        scene,
        columns,
        rows,
        tile_bytes,
        raster: vec![0u8; columns * rows * tile_bytes],
        written: 0,
        closed: false,
    })
}

fn normalize_pixel_type(code: u32) -> PyResult<ptiff::PixelType> {
    match code {
        0 => Ok(ptiff::PixelType::UInt8),
        1 => Ok(ptiff::PixelType::UInt16),
        2 => Ok(ptiff::PixelType::UInt32),
        3 => Ok(ptiff::PixelType::Float32),
        4 => Ok(ptiff::PixelType::Float64),
        other => Err(PyValueError::new_err(format!(
            "unknown pixel type code {other}"
        ))),
    }
}

#[pymethods]
impl Sink {
    /// Number of tile columns covering the image grid.
    #[getter]
    fn tile_columns(&self) -> usize {
        self.columns
    }

    /// Number of tile rows covering the image grid.
    #[getter]
    fn tile_rows(&self) -> usize {
        self.rows
    }

    /// Byte size of one (uniform) tile.
    #[getter]
    fn tile_byte_size(&self) -> usize {
        self.tile_bytes
    }

    /// How many tiles have been written so far.
    #[getter]
    fn tiles_written(&self) -> usize {
        self.written
    }

    /// Writes one tile's raw sample bytes (exactly `tile_byte_size` bytes) at
    /// grid position `(column, row)`. May be called in any order.
    fn write_tile(&mut self, column: u32, row: u32, buffer: &[u8]) -> PyResult<()> {
        if self.closed {
            return Err(PyValueError::new_err("sink already closed"));
        }
        if buffer.len() != self.tile_bytes {
            return Err(PyValueError::new_err(format!(
                "tile buffer must be exactly {0} bytes (got {1})",
                self.tile_bytes,
                buffer.len()
            )));
        }
        if column as usize >= self.columns || row as usize >= self.rows {
            return Err(PyValueError::new_err("tile index out of range"));
        }
        let offset = (row as usize * self.columns + column as usize) * self.tile_bytes;
        self.raster[offset..offset + self.tile_bytes].copy_from_slice(buffer);
        self.written += 1;
        Ok(())
    }

    /// Flushes the file (making it complete and readable) and closes the
    /// sink. Idempotent; must be called to finalise.
    fn close(&mut self) -> PyResult<()> {
        if self.closed {
            return Ok(());
        }
        let bytes = ptiff::Tiff::to_bytes_with_pixels(&self.scene, &[&self.raster[..]])
            .map_err(|e| PyOSError::new_err(format!("cannot serialise image: {e}")))?;
        std::fs::write(&self.path, bytes)
            .map_err(|e| PyOSError::new_err(format!("cannot write {:?}: {}", self.path, e)))?;
        self.closed = true;
        Ok(())
    }
}

impl Drop for Sink {
    fn drop(&mut self) {
        // A sink that was never `close`d does not emit a partially-written
        // file (matching the C ABI "must write every tile" contract).
    }
}
