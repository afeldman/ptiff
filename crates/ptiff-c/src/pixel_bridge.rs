//! `ptiff_source_*` / `ptiff_sink_*` handle surface
//! (`bindings/c/ptiff_pixel_bridge.h`).
//!
//! Two opaque handles cross the ABI here:
//!
//! - **`ptiff_source`** — a read-only, level-0 tile reader over one image of a
//!   TIFF/BigTIFF file. Built on the idiomatic [`ptiff::Tiff`]: the file bytes
//!   are retained, the image index is fixed at open time (the C ABI's singular
//!   `ptiff_source_open` reads image 0), and every grid/tile query goes
//!   through the core's [`ptiff::Tiff::tile_layout`] / [`ptiff::Tiff::read_tile`].
//! - **`ptiff_sink`** — a tiled image writer that buffers the tile grid in
//!   memory and flushes it to a file on close via
//!   [`ptiff::Tiff::to_bytes_with_pixels`].
//!
//! The surface follows the C++ `libptiff_c` contract: every fallible op returns
//! a negative `ptiff_error_code` and `0` on success; opaque handles are
//! heap-owned and destroyed with their `_close` counterpart (`NULL` = no-op).

use crate::error::to_c_error;
use crate::types::{apply_layout_tile_info, image_to_c, ptiff_image_descriptor};
use ptiff::{Error, ErrorCode, Scene, Tiff};
use std::os::raw::c_char;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Like the C++ `libptiff_c` `ptiff_source_*` accessors: the C header documents
/// that a `NULL`-untiled/single-strip image is exposed as a 1×1 grid (one
/// spanning strip), whereas the core's `TileLayout` reports a non-tiled image
/// with a zero tile extent. Normalise the core grid to the C-ABI view.
fn grid_dims(layout: &ptiff::TileLayout, level: u32) -> (u32, u32) {
    if layout.is_untiled() {
        (1, 1)
    } else {
        (layout.columns(level), layout.rows(level))
    }
}

/// Bytes of one (uniform) tile — grid tiles and edge tiles are padded to the
/// same size, so a single invariant holds for every tile in the image.
fn tile_byte_size(layout: &ptiff::TileLayout, channels: u32, bps: usize) -> usize {
    if layout.is_untiled() {
        // Single spanning strip: full image area.
        (layout.image_width as usize) * (layout.image_height as usize) * (channels as usize) * bps
    } else {
        (layout.tile_size.width as usize)
            * (layout.tile_size.height as usize)
            * (channels as usize)
            * bps
    }
}

// ---------------------------------------------------------------------------
// Read side: ptiff_source
// ---------------------------------------------------------------------------

/// Opaque, heap-owned source handle.
///
/// Retains the file bytes and the image index index it was opened at; each
/// read re-opens the core source over the retained buffer (the core's tile
/// views are short-lived), so the handle stays independent of any iterator
/// lifetime.
pub struct PtiffSource {
    /// Owned container bytes, kept so the source can be re-opened on demand.
    #[allow(dead_code)]
    bytes: Vec<u8>,
    /// The image index this source was opened at (always 0 in the C ABI).
    #[allow(dead_code)]
    image_index: usize,
    /// Cached descriptor (unowned, cheap to hold).
    descriptor: ptiff_image_descriptor,
    /// Cached layout (for grid + byte-size queries without re-opening).
    layout: ptiff::TileLayout,
    /// Tile byte size for the cached layout.
    tile_bytes: usize,
}

impl PtiffSource {
    /// Opens the first image of the file at `path`.
    fn open(path: &str) -> Result<PtiffSource, ptiff::Error> {
        let tiff = Tiff::open(path)?;
        let scene = tiff.scene();
        if scene.image_count() == 0 {
            return Err(Error::not_found("ptiff_source_open: file has no images"));
        }
        let image = scene.image_at(0)?;
        let layout = tiff.tile_layout(0)?;
        let mut descriptor = image_to_c(image);
        apply_layout_tile_info(&layout, &mut descriptor);
        let tile_bytes = tile_byte_size(
            &layout,
            image.channel_count(),
            image.pixel_type().bytes_per_sample(),
        );
        Ok(PtiffSource {
            bytes: tiff.as_bytes().to_vec(),
            image_index: 0,
            descriptor,
            layout,
            tile_bytes,
        })
    }
}

/// Creates a read-only source over the first image of the TIFF/BigTIFF file at
/// `path`. On failure returns `NULL` and, when `err_out` is non-null, sets
/// `*err_out` to a negative ptiff error code.
///
/// # Safety
///
/// `path` must be a valid NUL-terminated C string; `err_out` must be null or
/// point to a writable `int` for the duration of the call.
#[no_mangle]
pub extern "C" fn ptiff_source_open(
    path: *const std::os::raw::c_char,
    err_out: *mut i32,
) -> *mut PtiffSource {
    let result = (|| -> Result<*mut PtiffSource, ptiff::Error> {
        let path =
            c_str(path).ok_or_else(|| Error::invalid_argument("ptiff_source_open: null path"))?;
        let source = PtiffSource::open(&path)?;
        Ok(Box::into_raw(Box::new(source)))
    })();
    match result {
        Ok(ptr) => ptr,
        Err(e) => {
            if !err_out.is_null() {
                // Safety: non-null err_out points at a writable int.
                unsafe { *err_out = to_c_error(&e) };
            }
            std::ptr::null_mut()
        }
    }
}

/// Releases a handle returned by [`ptiff_source_open`]. `NULL` is a no-op.
///
/// # Safety
///
/// `source` must be null or a pointer previously returned by
/// [`ptiff_source_open`] that has not already been closed.
#[no_mangle]
pub extern "C" fn ptiff_source_close(source: *mut PtiffSource) {
    if !source.is_null() {
        // Safety: a non-null handle is unique-owned; dropping frees the box.
        drop(unsafe { Box::from_raw(source) });
    }
}

/// Fills `desc` with the opened image's metadata. Returns 0 on success, a
/// negative error code on a null source/desc.
///
/// # Safety
///
/// `source` must be null or a valid live handle; `desc` must be null or point
/// to writable storage of at least `sizeof(ptiff_image_descriptor)`.
#[no_mangle]
pub extern "C" fn ptiff_source_descriptor(
    source: *const PtiffSource,
    desc: *mut ptiff_image_descriptor,
) -> i32 {
    let Some(src) = src(source) else {
        return -(ErrorCode::InvalidArgument as i32);
    };
    if desc.is_null() {
        return -(ErrorCode::InvalidArgument as i32);
    }
    // Safety: both pointers validated above; we write a full Copy-able struct.
    unsafe { *desc = src.descriptor };
    0
}

/// Tile grid columns at level 0. `NULL` source -> 0.
#[no_mangle]
pub extern "C" fn ptiff_source_tile_columns(source: *const PtiffSource) -> u32 {
    let Some(src) = src(source) else { return 0 };
    grid_dims(&src.layout, 0).0
}

/// Tile grid rows at level 0. `NULL` source -> 0.
#[no_mangle]
pub extern "C" fn ptiff_source_tile_rows(source: *const PtiffSource) -> u32 {
    let Some(src) = src(source) else { return 0 };
    grid_dims(&src.layout, 0).1
}

/// Byte size of one decoded tile's pixel data (every tile incl. edge tiles is
/// this many bytes). `NULL` source -> 0.
#[no_mangle]
pub extern "C" fn ptiff_source_tile_byte_size(source: *const PtiffSource) -> usize {
    let Some(src) = src(source) else { return 0 };
    src.tile_bytes
}

/// Reads tile `(column, row)` into `buffer` (at least
/// [`ptiff_source_tile_byte_size`] bytes; `buffer_size` is checked). On success
/// returns 0 and sets `*bytes_read` to the number of bytes written.
///
/// # Safety
///
/// `source`, `buffer` and `bytes_read` must be valid live pointers; `buffer`
/// must point to at least `buffer_size` writable bytes.
#[no_mangle]
pub extern "C" fn ptiff_source_read_tile(
    source: *const PtiffSource,
    column: u32,
    row: u32,
    buffer: *mut u8,
    buffer_size: usize,
    bytes_read: *mut usize,
) -> i32 {
    let Some(src) = src(source) else {
        return -(ErrorCode::InvalidArgument as i32);
    };
    if buffer.is_null() || bytes_read.is_null() {
        return -(ErrorCode::InvalidArgument as i32);
    }
    if buffer_size < src.tile_bytes {
        return -(ErrorCode::InvalidArgument as i32);
    }
    // Re-open the source over the retained bytes and read the requested tile.
    let tiff = match Tiff::from_bytes(&src.bytes).map_err(|e| to_c_error(&e)) {
        Ok(t) => t,
        Err(rc) => return rc,
    };
    let tile = match tiff.read_tile(0, column, row) {
        Ok(t) => t,
        Err(e) => return to_c_error(&e),
    };
    // `buffer` is guaranteed large enough (checked above), so a full tile fits.
    // Safety: `buffer` points at `buffer_size` writable bytes >= tile.len().
    unsafe { std::ptr::copy_nonoverlapping(tile.as_ptr(), buffer, tile.len()) };
    // Safety: `bytes_read` is a valid writable usize.
    unsafe { *bytes_read = tile.len() };
    0
}

// ---------------------------------------------------------------------------
// Write side: ptiff_sink
// ---------------------------------------------------------------------------

/// Opaque, heap-owned sink handle.
///
/// Buffers the full tiled raster in memory and flushes it to disk on close via
/// [`ptiff::Tiff::to_bytes_with_pixels`]. Only tiled images are supported (the
/// C header requires `desc->has_tile_info`, matching TIFF tile I/O).
pub struct PtiffSink {
    /// Destination path.
    path: PathBuf,
    /// The scene to write (single image).
    scene: Scene,
    /// Grid columns/rows for the tiled image.
    columns: u32,
    rows: u32,
    /// Bytes of one tile.
    tile_bytes: usize,
    /// The accumulated raster (grid-ordered, exactly what `to_bytes_with_pixels`
    /// expects): rows x columns x tile_bytes bytes.
    raster: Vec<u8>,
    /// Tiles written so far (for a sanity out-of-range check on writes).
    written: u64,
}

impl PtiffSink {
    /// Builds the sink from a C descriptor (optionally carrying a structured
    /// camera); returns a core error for inputs the header forbids (non-tiled
    /// or unsupported sample layout).
    fn create(
        path: &str,
        desc: &ptiff_image_descriptor,
        camera: Option<&crate::camera::ptiff_camera>,
    ) -> Result<PtiffSink, ptiff::Error> {
        if desc.has_tile_info == 0
            || desc.tile_info.tile_width == 0
            || desc.tile_info.tile_height == 0
        {
            return Err(Error::invalid_argument(
                "ptiff_sink_create: image must be tiled (desc->has_tile_info)",
            ));
        }
        let tw = desc.tile_info.tile_width;
        let th = desc.tile_info.tile_height;

        let mut scene = Scene::new();
        let mut image = crate::types::descriptor_from_c(desc);
        if let Some(cam) = camera {
            image.camera = Some(crate::camera::camera_from_c(cam));
        }
        let channels = image.channel_count;
        let bps = image.pixel_type.bytes_per_sample();
        let tile_bytes = (tw as usize) * (th as usize) * (channels as usize) * bps;

        // Build the layout the write path will use, so we can expose the grid
        // and byte-size upfront and allocate the exact raster.
        let layout =
            ptiff::TileLayout::new(ptiff::TileExtent::new(tw, th), image.width, image.height, 1);
        let columns = layout.columns(0);
        let rows = layout.rows(0);
        if columns == 0 || rows == 0 {
            return Err(Error::invalid_argument(
                "ptiff_sink_create: bad tile/image dimensions",
            ));
        }

        scene.add_image(image)?;
        Ok(PtiffSink {
            path: PathBuf::from(path),
            scene,
            columns,
            rows,
            tile_bytes,
            raster: vec![0u8; (columns as usize) * (rows as usize) * tile_bytes],
            written: 0,
        })
    }
}

/// Creates the TIFF file at `path` for writing a single tiled image described
/// by `desc`, returning a heap-owned handle positioned for tile writes. Returns
/// `NULL` on failure (see [`ptiff_sink_create_camera`] docs / header).
///
/// # Safety
///
/// `path` and `desc` must be valid non-null pointers for the duration of the call.
#[no_mangle]
pub extern "C" fn ptiff_sink_create(
    path: *const c_char,
    desc: *const ptiff_image_descriptor,
) -> *mut PtiffSink {
    let result = (|| -> Result<*mut PtiffSink, ptiff::Error> {
        let path =
            c_str(path).ok_or_else(|| Error::invalid_argument("ptiff_sink_create: null path"))?;
        if desc.is_null() {
            return Err(Error::invalid_argument(
                "ptiff_sink_create: null descriptor",
            ));
        }
        // Safety: desc validated non-null above; documents a valid descriptor.
        let c_desc = unsafe { &*desc };
        let sink = PtiffSink::create(&path, c_desc, None)?;
        Ok(Box::into_raw(Box::new(sink)))
    })();
    match result {
        Ok(ptr) => ptr,
        Err(_) => std::ptr::null_mut(),
    }
}

/// Same as [`ptiff_sink_create`], but additionally persists the structured
/// camera calibration attached to the written image.
///
/// Returns a heap-owned sink on success, `NULL` on failure (null/unsupported
/// arguments, matching `ptiff_sink_create`).
///
/// # Safety
///
/// `path`, `desc` and `camera` must be valid non-null pointers for the duration
/// of the call (the header documents all three as required).
#[no_mangle]
pub extern "C" fn ptiff_sink_create_camera(
    path: *const c_char,
    desc: *const ptiff_image_descriptor,
    camera: *const crate::camera::ptiff_camera,
) -> *mut PtiffSink {
    let result = (|| -> Result<*mut PtiffSink, ptiff::Error> {
        let path = c_str(path)
            .ok_or_else(|| Error::invalid_argument("ptiff_sink_create_camera: null path"))?;
        if desc.is_null() || camera.is_null() {
            return Err(Error::invalid_argument(
                "ptiff_sink_create_camera: null descriptor/camera",
            ));
        }
        // Safety: both non-null pointers are validated; they document valid
        // descriptor + camera structs.
        let c_desc = unsafe { &*desc };
        let c_cam = unsafe { &*camera };
        let sink = PtiffSink::create(&path, c_desc, Some(c_cam))?;
        Ok(Box::into_raw(Box::new(sink)))
    })();
    match result {
        Ok(ptr) => ptr,
        Err(_) => std::ptr::null_mut(),
    }
}

/// Tile grid columns of the sink's image. `NULL` sink -> 0.
#[no_mangle]
pub extern "C" fn ptiff_sink_tile_columns(sink: *const PtiffSink) -> u32 {
    let Some(s) = sink_handle(sink) else { return 0 };
    s.columns
}

/// Tile grid rows of the sink's image. `NULL` sink -> 0.
#[no_mangle]
pub extern "C" fn ptiff_sink_tile_rows(sink: *const PtiffSink) -> u32 {
    let Some(s) = sink_handle(sink) else { return 0 };
    s.rows
}

/// Byte size of one written tile. `NULL` sink -> 0.
#[no_mangle]
pub extern "C" fn ptiff_sink_tile_byte_size(sink: *const PtiffSink) -> usize {
    let Some(s) = sink_handle(sink) else { return 0 };
    s.tile_bytes
}

/// Writes one tile of pixel data. `buffer` must be exactly
/// [`ptiff_sink_tile_byte_size`] bytes; the tile index must lie within the
/// grid. Returns 0 on success.
///
/// # Safety
///
/// `sink` and `buffer` must be valid; `buffer` must point to at least
/// `buffer_size` readable bytes (which the header documents must equal the tile
/// byte size).
#[no_mangle]
pub extern "C" fn ptiff_sink_write_tile(
    sink: *mut PtiffSink,
    column: u32,
    row: u32,
    buffer: *const u8,
    buffer_size: usize,
) -> i32 {
    let Some(s) = sink_handle_mut(sink) else {
        return -(ErrorCode::InvalidArgument as i32);
    };
    if buffer.is_null() {
        return -(ErrorCode::InvalidArgument as i32);
    }
    if buffer_size != s.tile_bytes {
        return -(ErrorCode::InvalidArgument as i32);
    }
    if column >= s.columns || row >= s.rows {
        return -(ErrorCode::OutOfRange as i32);
    }
    // Grid-ordered raster: row-major, column fastest (matching read order).
    let offset = ((row as usize) * (s.columns as usize) + column as usize) * s.tile_bytes;
    // Safety: offset+tile_bytes <= raster.len() (grid/byte invariants), buffer
    // is tile_bytes readable bytes.
    unsafe {
        std::ptr::copy_nonoverlapping(buffer, s.raster.as_mut_ptr().add(offset), s.tile_bytes);
    }
    s.written += 1;
    0
}

/// Flushes the sink (writing the file) and releases the handle. `NULL` no-op.
///
/// # Safety
///
/// `sink` must be null or a live handle previously returned by
/// [`ptiff_sink_create`], not already closed.
#[no_mangle]
pub extern "C" fn ptiff_sink_close(sink: *mut PtiffSink) {
    if sink.is_null() {
        return;
    }
    // Safety: non-null handle is unique-owned; we consume it to flush+drop.
    let mut s = unsafe { Box::from_raw(sink) };
    // Extract the owned pieces before dropping the handle, so the file write
    // below does not borrow a value we are about to free. `Scene`, `PathBuf`
    // and `Vec` are all `Default`, so `mem::replace`/`mem::take` move them out.
    let scene = std::mem::replace(&mut s.scene, Scene::new());
    let path = std::mem::take(&mut s.path);
    let raster = std::mem::take(&mut s.raster);
    drop(s);
    // The C ABI sink has no error-reporting channel at close, so on a write
    // failure we swallow the error (the file is simply not emitted), matching
    // the "must write every tile" contract in the header docs.
    let _ = (|| -> Result<(), ptiff::Error> {
        let tiff_bytes = Tiff::to_bytes_with_pixels(&scene, &[&raster[..]])?;
        std::fs::write(&path, tiff_bytes)
            .map_err(|e| Error::invalid_argument(format!("ptiff_sink_close: {e}")))
    })();
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Reads a NUL-terminated C string into an owned `String`. Returns `None` for
/// a null pointer.
///
/// # Safety
///
/// The caller guarantees `path` is null or a valid NUL-terminated C string.
fn c_str(path: *const std::os::raw::c_char) -> Option<String> {
    if path.is_null() {
        return None;
    }
    // Safety: a non-null C string is NUL-terminated, so `CStr::from_ptr` is
    // sound; the UTF-8 conversion falls back to lossy (paths are usually ASCII).
    let s = unsafe { std::ffi::CStr::from_ptr(path) }
        .to_string_lossy()
        .into_owned();
    Some(s)
}

/// Borrows a source handle, `None` for a null pointer.
fn src<'a>(source: *const PtiffSource) -> Option<&'a PtiffSource> {
    if source.is_null() {
        None
    } else {
        // Safety: a non-null handle is unique-owned and valid for the duration
        // of a single call while the caller holds it.
        Some(unsafe { &*source })
    }
}

/// Borrows a sink handle (read-only), `None` for a null pointer.
fn sink_handle<'a>(sink: *const PtiffSink) -> Option<&'a PtiffSink> {
    if sink.is_null() {
        None
    } else {
        // Safety: a non-null handle is unique-owned and valid for the duration
        // of a single call while the caller holds it.
        Some(unsafe { &*sink })
    }
}

/// Borrows a sink handle (mutable), `None` for a null pointer.
fn sink_handle_mut<'a>(sink: *mut PtiffSink) -> Option<&'a mut PtiffSink> {
    if sink.is_null() {
        None
    } else {
        // Safety: a non-null handle is unique-owned and valid for the duration
        // of a single call while the caller holds it.
        Some(unsafe { &mut *sink })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ptiff_image_descriptor;

    fn tiled_desc(width: u32, height: u32, tw: u32, th: u32) -> ptiff_image_descriptor {
        let mut d = ptiff_image_descriptor::c_default();
        d.width = width;
        d.height = height;
        d.channel_count = 1;
        d.has_tile_info = 1;
        d.tile_info.tile_width = tw;
        d.tile_info.tile_height = th;
        d
    }

    /// Writes a tiny tiled TIFF via the C-ABI sink, then reads it back via the
    /// source/CLI read tier, returning the re-read pixels.
    fn write_and_read_temp(
        path: &std::path::Path,
        width: u32,
        height: u32,
        tw: u32,
        th: u32,
    ) -> Vec<u8> {
        let path_s = path.to_str().unwrap();
        let desc = tiled_desc(width, height, tw, th);
        // stack address of `desc` and the NUL-terminated path string are valid.
        let sink = ptiff_sink_create(
            std::ffi::CString::new(path_s).unwrap().as_ptr(),
            &desc as *const ptiff_image_descriptor,
        );
        assert!(!sink.is_null());
        let cols = ptiff_sink_tile_columns(sink);
        let rows = ptiff_sink_tile_rows(sink);
        let tile_bytes = ptiff_sink_tile_byte_size(sink);
        // Fill every tile with a distinguishable value (index-based).
        for row in 0..rows {
            for col in 0..cols {
                let mut buf = vec![0u8; tile_bytes];
                let idx = (row * cols + col) as u8 + 1;
                buf.fill(idx);
                // sink/buffer valid, size exact; write_tile is the C-ABI sink write.
                assert_eq!(
                    ptiff_sink_write_tile(sink, col, row, buf.as_ptr(), buf.len()),
                    0
                );
            }
        }
        // live sink handle.
        ptiff_sink_close(sink);

        // Now read back via the C-ABI source.
        // NUL-terminated path string.
        let source = ptiff_source_open(
            std::ffi::CString::new(path_s).unwrap().as_ptr(),
            std::ptr::null_mut(),
        );
        assert!(!source.is_null());
        let mut out = Vec::new();
        let rcols = ptiff_source_tile_columns(source);
        let rrows = ptiff_source_tile_rows(source);
        assert_eq!((rcols, rrows), (cols, rows));
        for row in 0..rrows {
            for col in 0..rcols {
                let mut buf = vec![0u8; tile_bytes];
                let mut bytes_read = 0usize;
                // Safety: valid buffer + bytes_read.
                assert_eq!(
                    ptiff_source_read_tile(
                        source,
                        col,
                        row,
                        buf.as_mut_ptr(),
                        buf.len(),
                        &mut bytes_read
                    ),
                    0
                );
                assert_eq!(bytes_read, tile_bytes);
                out.extend_from_slice(&buf);
            }
        }
        // live source handle.
        ptiff_source_close(source);
        out
    }

    #[test]
    fn untitled_byte_size_is_full_image() {
        // Direct unit check of the byte-size helper for a single-strip image.
        let layout = ptiff::TileLayout::new(ptiff::TileExtent::new(0, 0), 40, 30, 1);
        assert!(layout.is_untiled());
        assert_eq!(tile_byte_size(&layout, 1, 1), 40 * 30);
    }

    #[test]
    fn sink_source_round_trips_a_tiny_tiled_image() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tiny.tif");
        let pixels = write_and_read_temp(&path, 34, 34, 16, 16);
        // 34x34 with 16x16 tiles => 3x3 grid of 16x16 tiles => 2304 bytes.
        assert_eq!(pixels.len(), 3 * 3 * 16 * 16);
        // Each written tile carries its (row*cols+col) value, so the raster
        // equals the concatenation 1,2,...,9 repeated 16*16 times.
        let mut expected = Vec::new();
        for tile in 1u8..=9 {
            let mut t = vec![0u8; 16 * 16];
            t.fill(tile);
            expected.extend_from_slice(&t);
        }
        assert_eq!(pixels, expected);
    }

    #[test]
    fn source_descriptor_reports_tile_info_for_tiled_input() {
        // The read-side C ABI must report the tiling of a written tiled image
        // (mirroring the C++ oracle, which reads tile info from the opened
        // image's layout). The core's SceneDeserializer deliberately leaves
        // `tile_info` unset, so the bridge re-derives it from `tile_layout`.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tileinfo.tif");
        let path_s = path.to_str().unwrap().to_string();

        // Write a small tiled image via the sink.
        let desc = tiled_desc(64, 48, 16, 16);
        let sink = ptiff_sink_create(
            std::ffi::CString::new(path_s.clone()).unwrap().as_ptr(),
            &desc as *const ptiff_image_descriptor,
        );
        assert!(!sink.is_null());
        let cols = ptiff_sink_tile_columns(sink);
        let rows = ptiff_sink_tile_rows(sink);
        let tile_bytes = ptiff_sink_tile_byte_size(sink);
        let buf = vec![7u8; tile_bytes];
        for row in 0..rows {
            for col in 0..cols {
                assert_eq!(
                    ptiff_sink_write_tile(sink, col, row, buf.as_ptr(), buf.len()),
                    0
                );
            }
        }
        ptiff_sink_close(sink);

        // Read the descriptor back through the C-ABI source.
        let source = ptiff_source_open(
            std::ffi::CString::new(path_s).unwrap().as_ptr(),
            std::ptr::null_mut(),
        );
        assert!(!source.is_null());
        let mut out = ptiff_image_descriptor::c_default();
        assert_eq!(
            ptiff_source_descriptor(source, &mut out as *mut ptiff_image_descriptor),
            0
        );
        assert_eq!(out.width, 64);
        assert_eq!(out.height, 48);
        assert_eq!(out.has_tile_info, 1);
        assert_eq!(out.tile_info.tile_width, 16);
        assert_eq!(out.tile_info.tile_height, 16);
        ptiff_source_close(source);
    }

    #[test]
    fn sink_write_tile_rejects_bad_sizes_and_indices() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sink.tif");
        let path_s = path.to_str().unwrap().to_string();
        let desc = tiled_desc(34, 34, 16, 16);
        // point desc + C string into the FFI call are valid.
        let sink = ptiff_sink_create(
            std::ffi::CString::new(path_s).unwrap().as_ptr(),
            &desc as *const ptiff_image_descriptor,
        );
        assert!(!sink.is_null());
        let tile_bytes = ptiff_sink_tile_byte_size(sink);
        assert_eq!(tile_bytes, 16 * 16);
        let good = vec![0u8; tile_bytes];
        // Wrong size -> INVALID_ARGUMENT.
        let bad = vec![0u8; tile_bytes - 1];
        let rc = ptiff_sink_write_tile(sink, 0, 0, bad.as_ptr(), bad.len());
        assert_eq!(rc, -(ErrorCode::InvalidArgument as i32));
        // Out of grid -> OUT_OF_RANGE.
        let rc = ptiff_sink_write_tile(sink, 99, 0, good.as_ptr(), good.len());
        assert_eq!(rc, -(ErrorCode::OutOfRange as i32));
        // Null sink -> INVALID_ARGUMENT.
        let rc = ptiff_sink_write_tile(std::ptr::null_mut(), 0, 0, good.as_ptr(), good.len());
        assert_eq!(rc, -(ErrorCode::InvalidArgument as i32));
        // Drain + close.
        for row in 0..ptiff_sink_tile_rows(sink) {
            for col in 0..ptiff_sink_tile_columns(sink) {
                let _ = ptiff_sink_write_tile(sink, col, row, good.as_ptr(), good.len());
            }
        }
        ptiff_sink_close(sink);
    }

    #[test]
    fn non_tiled_descriptor_is_rejected() {
        let mut d = ptiff_image_descriptor::c_default();
        d.width = 40;
        d.height = 30;
        // untiled -> INVALID_ARGUMENT per header; always returns null
        d.has_tile_info = 0;
        let sink = ptiff_sink_create(
            std::ffi::CString::new("/tmp/never_used.tif")
                .unwrap()
                .as_ptr(),
            &d as *const ptiff_image_descriptor,
        );
        assert!(sink.is_null());
    }

    #[test]
    fn camera_sink_write_read_round_trips_the_structured_camera() {
        // Write a tiled image with a structured camera via ptiff_sink_create_camera,
        // then read it back through ptiff_open_path_camera: the matrices + flags +
        // timestamp survive the round trip (the core serializes camera via tag 65002).
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("camera.tif");
        let path_s = path.to_str().unwrap().to_string();

        let desc = tiled_desc(34, 34, 16, 16);
        // Build a fully-populated C camera.
        let cam = crate::camera::ptiff_camera {
            has_intrinsics: 1,
            focal_length_x: 900.0,
            focal_length_y: 901.0,
            principal_x: 512.5,
            principal_y: 384.25,
            intrinsics: [900.0, 0.0, 512.5, 0.0, 901.0, 384.25, 0.0, 0.0, 1.0],
            has_extrinsics: 1,
            rotation_w: 0.7,
            rotation_x: 0.1,
            rotation_y: 0.2,
            rotation_z: 0.3,
            position_x: 1.0,
            position_y: 2.0,
            position_z: 3.0,
            extrinsics: [0.0; 12],
            projection: [0.0; 12],
            timestamp: [0u8; 64],
        };

        let sink = ptiff_sink_create_camera(
            std::ffi::CString::new(path_s.clone()).unwrap().as_ptr(),
            &desc as *const ptiff_image_descriptor,
            &cam as *const crate::camera::ptiff_camera,
        );
        assert!(!sink.is_null());
        let cols = ptiff_sink_tile_columns(sink);
        let rows = ptiff_sink_tile_rows(sink);
        let tile_bytes = ptiff_sink_tile_byte_size(sink);
        let buf = vec![9u8; tile_bytes];
        for row in 0..rows {
            for col in 0..cols {
                assert_eq!(
                    ptiff_sink_write_tile(sink, col, row, buf.as_ptr(), buf.len()),
                    0
                );
            }
        }
        ptiff_sink_close(sink);

        // Read the camera back through the C-ABI read entry point.
        let mut out = crate::camera::ptiff_camera {
            has_intrinsics: 0,
            focal_length_x: 0.0,
            focal_length_y: 0.0,
            principal_x: 0.0,
            principal_y: 0.0,
            intrinsics: [0.0; 9],
            has_extrinsics: 0,
            rotation_w: 0.0,
            rotation_x: 0.0,
            rotation_y: 0.0,
            rotation_z: 0.0,
            position_x: 0.0,
            position_y: 0.0,
            position_z: 0.0,
            extrinsics: [0.0; 12],
            projection: [0.0; 12],
            timestamp: [0u8; 64],
        };
        let rc = crate::camera::ptiff_open_path_camera(
            std::ffi::CString::new(path_s).unwrap().as_ptr(),
            &mut out as *mut crate::camera::ptiff_camera,
        );
        assert_eq!(rc, 0);
        assert_eq!(out.has_intrinsics, 1);
        assert_eq!(out.focal_length_x, 900.0);
        assert_eq!(out.focal_length_y, 901.0);
        assert_eq!(out.principal_x, 512.5);
        assert_eq!(out.principal_y, 384.25);
        assert_eq!(out.has_extrinsics, 1);
        assert_eq!(out.rotation_w, 0.7);
        assert_eq!(out.rotation_z, 0.3);
        assert_eq!(out.position_x, 1.0);
        assert_eq!(out.position_z, 3.0);
        // Derived matrix `intrinsics` K must equal the known pinhole K.
        assert_eq!(out.intrinsics[0], 900.0);
        assert_eq!(out.intrinsics[4], 901.0);
        assert_eq!(out.intrinsics[8], 1.0);
    }

    #[test]
    fn open_path_camera_returns_zero_struct_when_no_camera_in_file() {
        // A file with no camera metadata reads back as an all-zero struct with
        // has_* = 0 (matching the C++ oracle), not an error.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("no_camera.tif");
        let path_s = path.to_str().unwrap().to_string();

        let desc = tiled_desc(34, 34, 16, 16);
        let sink = ptiff_sink_create(
            std::ffi::CString::new(path_s.clone()).unwrap().as_ptr(),
            &desc as *const ptiff_image_descriptor,
        );
        assert!(!sink.is_null());
        let cols = ptiff_sink_tile_columns(sink);
        let rows = ptiff_sink_tile_rows(sink);
        let tile_bytes = ptiff_sink_tile_byte_size(sink);
        let buf = vec![0u8; tile_bytes];
        for row in 0..rows {
            for col in 0..cols {
                let _ = ptiff_sink_write_tile(sink, col, row, buf.as_ptr(), buf.len());
            }
        }
        ptiff_sink_close(sink);

        let mut out = crate::camera::ptiff_camera {
            has_intrinsics: 1, // pre-set to detect overwrite
            focal_length_x: 0.0,
            focal_length_y: 0.0,
            principal_x: 0.0,
            principal_y: 0.0,
            intrinsics: [0.0; 9],
            has_extrinsics: 1,
            rotation_w: 0.0,
            rotation_x: 0.0,
            rotation_y: 0.0,
            rotation_z: 0.0,
            position_x: 0.0,
            position_y: 0.0,
            position_z: 0.0,
            extrinsics: [0.0; 12],
            projection: [0.0; 12],
            timestamp: [0u8; 64],
        };
        let rc = crate::camera::ptiff_open_path_camera(
            std::ffi::CString::new(path_s).unwrap().as_ptr(),
            &mut out as *mut crate::camera::ptiff_camera,
        );
        assert_eq!(rc, 0);
        assert_eq!(out.has_intrinsics, 0);
        assert_eq!(out.has_extrinsics, 0);
    }
}
