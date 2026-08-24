//! Metadata C ABI (`bindings/c/ptiff_metadata.h`).
//!
//! [`ptiff_open_path`] opens a TIFF/BigTIFF file read-only and fills a
//! [`crate::types::ptiff_image_descriptor`] with the first image's metadata
//! (the same fields `ptiff_source_descriptor` exposes, without retaining a
//! handle).
//!
//! [`ptiff_open_path_fields`] / [`ptiff_fields_free`] expose the flattened
//! `ptiff.<domain>.<key>` extension fields decoded from the private tags
//! 65001-65005 (SPICE, camera geometry, CRS, scientific layers, provenance),
//! together with the primary image metadata. Field keys carry the
//! `ptiff.<domain>.<name>` shape (e.g. `ptiff.spice.frame`) and are returned in
//! lexicographic order, matching the C++ oracle's `deserializeModel` +
//! `for_each_field` iteration over the (key-)sorted `StorageModel` field map.
//! Domains absent from the file simply contribute no entries.
//!
//! The flat field view is read by re-deriving the format-neutral
//! [`ptiff::StorageModel`] through the core [`TiffBackend::deserialize_model`]
//! — the same lossless `ptiff.*` field archive the write path emits — rather
//! than the structural `Scene`/`Image` camera/CRS domains, so every extension
//! domain (including `spice`/`layers`/`provenance`, which are not part of the
//! typed `ImageDescriptor`) round-trips verbatim.

use crate::error::{ptiff_error_code, to_c_error};
use crate::types::{apply_layout_tile_info, image_to_c, ptiff_image_descriptor};
use ptiff::{Error, Tiff};
use ptiff_core::StorageBackend as _;
use std::os::raw::{c_char, c_int};

/// A single flattened PTIFF extension field (private tags 65001-65005). `key`
/// is the fully qualified field name, e.g. `ptiff.spice.frame` or
/// `ptiff.camera.model`. Both strings are owned by the array returned from
/// [`ptiff_open_path_fields`]; free the whole array with [`ptiff_fields_free`].
///
/// `#[repr(C)]` and plain `*mut c_char` mirrors `ptiff_field` in
/// `bindings/c/ptiff_metadata.h`, so foreign runtimes read it directly.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ptiff_field {
    /// NUL-terminated fully-qualified field key (e.g. `ptiff.spice.frame`).
    pub key: *mut c_char,
    /// NUL-terminated field value.
    pub value: *mut c_char,
}

/// Allocates a NUL-terminated C string from `bytes` using a `Vec` with capacity
/// `len + 1`. The returned pointer is freed with the matching `from_raw_parts`
/// reconstruction in [`ptiff_fields_free`] — the same idiom as
/// [`crate::bridge::ptiff_free_string`].
fn alloc_c_string(bytes: &[u8]) -> *mut c_char {
    let mut v: Vec<u8> = Vec::with_capacity(bytes.len() + 1);
    v.extend_from_slice(bytes);
    v.push(0);
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p as *mut c_char
}

/// Opens the TIFF/BigTIFF file at `path` (read-only) and fills `desc` with the
/// first image's metadata. Returns 0 on success, a negative ptiff error code on
/// failure (and leaves `*desc` untouched). `path` and `desc` must be non-null.
///
/// # Safety
///
/// `path` must be a valid NUL-terminated C string; `desc` must point to
/// writable storage of at least `sizeof(ptiff_image_descriptor)`.
#[no_mangle]
pub extern "C" fn ptiff_open_path(path: *const c_char, desc: *mut ptiff_image_descriptor) -> i32 {
    if path.is_null() || desc.is_null() {
        return -(ptiff_error_code::PTIFF_ERROR_INVALID_ARGUMENT as i32);
    }
    // Safety: validated non-null above; C string is NUL-terminated per header.
    let path_s = unsafe { std::ffi::CStr::from_ptr(path) }
        .to_string_lossy()
        .into_owned();
    let result = (|| -> Result<ptiff_image_descriptor, ptiff::Error> {
        let tiff = Tiff::open(&path_s)?;
        let scene = tiff.scene();
        if scene.image_count() == 0 {
            return Err(Error::not_found("ptiff_open_path: file has no images"));
        }
        let image = scene.image_at(0)?;
        let mut descriptor = image_to_c(image);
        let layout = tiff.tile_layout(0)?;
        apply_layout_tile_info(&layout, &mut descriptor);
        Ok(descriptor)
    })();
    match result {
        Ok(d) => {
            // Safety: `desc` is a writable pointer of at least the struct size.
            unsafe { *desc = d };
            0
        }
        Err(e) => to_c_error(&e),
    }
}

/// Opens the TIFF/BigTIFF file at `path` (read-only) and returns the flattened
/// PTIFF extension fields decoded from the private tags 65001-65005 together
/// with the primary image metadata. Field keys carry the `ptiff.<domain>.<name>`
/// shape (e.g. `ptiff.spice.frame`); ordering is lexicographic by key; domains
/// absent from the file contribute no entries.
///
/// On success returns `0`, sets `*out_count` to the number of fields and
/// `*out` to a malloc'd array of that many [`ptiff_field`]s (which is `NULL`
/// when count is 0); the caller must call [`ptiff_fields_free`]`(*out,
/// *out_count)`. On failure returns a negative ptiff error code and leaves
/// `*out`/`*out_count` untouched. `path`, `out` and `out_count` must be
/// non-null.
///
/// # Safety
///
/// `path` must be a valid NUL-terminated C string; `out`/`out_count` must point
/// to writable storage. On success the returned array is owned by the caller
/// and must be released with [`ptiff_fields_free`].
#[no_mangle]
pub extern "C" fn ptiff_open_path_fields(
    path: *const c_char,
    out: *mut *mut ptiff_field,
    out_count: *mut c_int,
) -> i32 {
    if path.is_null() || out.is_null() || out_count.is_null() {
        return -(ptiff_error_code::PTIFF_ERROR_INVALID_ARGUMENT as i32);
    }
    // Safety: validated non-null above; C string is NUL-terminated per header.
    let path_s = unsafe { std::ffi::CStr::from_ptr(path) }
        .to_string_lossy()
        .into_owned();

    let result = (|| -> Result<Vec<(String, String)>, ptiff::Error> {
        // Read the file bytes, then re-derive the format-neutral StorageModel
        // through the core TIFF backend — the same lossless `ptiff.*` field
        // archive the write path emits (mirrors the C++ oracle's
        // `TiffBackend::deserializeModel` + `imageModel`).
        let bytes = std::fs::read(&path_s).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                ptiff::Error::not_found(format!("ptiff_open_path_fields: file not found: {e}"))
            } else {
                ptiff::Error::invalid_argument(format!(
                    "ptiff_open_path_fields: cannot read file: {e}"
                ))
            }
        })?;
        let mut reader = ptiff_core::io::MemoryBinaryReader::from_slice(&bytes);
        let model = ptiff_core::io::backend::tiff::TiffBackend.deserialize_model(&mut reader)?;

        // Normalize root-model-with-child / flat per-image model (C++ imageModel).
        let image = model.children().first().unwrap_or(&model);

        // Collect the flattened `ptiff.<domain>.<name>` fields in the
        // StorageModel's ascending key order (BTreeMap), mirroring the C++
        // `for_each_field` with a `ptiff.`-prefix filter.
        let mut fields = Vec::new();
        image.for_each_field(|key, value| {
            if key.starts_with("ptiff.") {
                fields.push((key.to_string(), value.to_string()));
            }
        });
        Ok(fields)
    })();

    match result {
        Ok(fields) => {
            if fields.is_empty() {
                // Safety: out is validated non-null; both writes are to writable
                // caller-provided storage.
                unsafe {
                    *out = std::ptr::null_mut();
                    *out_count = 0;
                }
                return 0;
            }
            // Allocate a contiguous array of `ptiff_field`s (leaked to the
            // caller), each holding its own leaked key/value C strings.
            let field_count = fields.len();
            let mut array: Vec<ptiff_field> = Vec::with_capacity(field_count);
            for (key, value) in fields {
                array.push(ptiff_field {
                    key: alloc_c_string(key.as_bytes()),
                    value: alloc_c_string(value.as_bytes()),
                });
            }
            let array_ptr = array.as_mut_ptr();
            std::mem::forget(array);    // ownership transfers to the caller
                                        // Safety: out/out_count validated non-null; array_ptr is a heap
                                        // allocation freed only via ptiff_fields_free.
            unsafe {
                *out = array_ptr;
                *out_count = field_count as c_int;
            }
            0
        }
        Err(e) => to_c_error(&e),
    }
}

/// Frees an array returned by [`ptiff_open_path_fields`]. A `NULL` array is a
/// no-op; `count` releases each field's two strings, then the array itself.
///
/// # Safety
///
/// `arr` must be null or a pointer previously returned by
/// [`ptiff_open_path_fields`], freed exactly once; `count` must be the
/// matching `*out_count` value.
#[no_mangle]
pub extern "C" fn ptiff_fields_free(arr: *mut ptiff_field, count: c_int) {
    if arr.is_null() {
        return;
    }
    let count: usize = count.max(0) as usize;
    // Safety: `arr` points to a `Vec<ptiff_field>` of `count` elements
    // allocated in `ptiff_open_path_fields`; each element's key/value point to
    // NUL-terminated C strings allocated with `Vec` capacity `len + 1` by
    // `alloc_c_string`. The three `from_raw_parts`/`drop` reconstructions are
    // the symmetric frees.
    unsafe {
        for i in 0..count {
            let cur = arr.add(i);
            if !(*cur).key.is_null() {
                let key_ptr = (*cur).key;
                let len = std::ffi::CStr::from_ptr(key_ptr).to_bytes().len();
                drop(Vec::from_raw_parts(key_ptr, len, len + 1));
            }
            if !(*cur).value.is_null() {
                let value_ptr = (*cur).value;
                let len = std::ffi::CStr::from_ptr(value_ptr).to_bytes().len();
                drop(Vec::from_raw_parts(value_ptr, len, len + 1));
            }
        }
        drop(Vec::from_raw_parts(arr, count, count));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_path_or_desc_is_invalid_argument() {
        assert_eq!(
            ptiff_open_path(std::ptr::null(), std::ptr::null_mut()),
            -(ptiff_error_code::PTIFF_ERROR_INVALID_ARGUMENT as i32)
        );
    }

    #[test]
    fn null_path_with_valid_desc_is_invalid_argument() {
        let mut d = ptiff_image_descriptor::c_default();
        assert_eq!(
            // Safety: null path + valid desc; path is checked first.
            ptiff_open_path(std::ptr::null(), &mut d as *mut ptiff_image_descriptor),
            -(ptiff_error_code::PTIFF_ERROR_INVALID_ARGUMENT as i32)
        );
    }

    #[test]
    fn open_path_reports_error_for_missing_file() {
        let mut d = ptiff_image_descriptor::c_default();
        let path = std::ffi::CString::new("/nonexistent/definitely_missing.tif").unwrap();
        // Safety: valid path + valid desc; the file does not exist.
        let rc = ptiff_open_path(path.as_ptr(), &mut d as *mut ptiff_image_descriptor);
        // A missing path surfaces as NOT_FOUND (matching the C++ oracle /
        // ptiff_error.h semantics), not an invalid-stream code.
        assert_eq!(rc, -(ptiff_error_code::PTIFF_ERROR_NOT_FOUND as i32));
    }

    #[test]
    fn open_path_reports_tile_info_for_tiled_file() {
        // A tiled file must round-trip its tile info through ptiff_open_path,
        // the same re-derivation from the layout that ptiff_source_descriptor
        // does (C++ oracle parity; the core leaves tile_info unset).
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("meta.tif");
        let path_s = path.to_str().unwrap().to_string();

        // Use the pixel-bridge sink to produce a small tiled file.
        let mut d = ptiff_image_descriptor::c_default();
        d.width = 64;
        d.height = 48;
        d.pixel_type = crate::types::ptiff_pixel_type::PTIFF_PIXEL_UINT8 as i32;
        d.channel_count = 1;
        d.has_tile_info = 1;
        d.tile_info.tile_width = 16;
        d.tile_info.tile_height = 16;
        let sink = crate::pixel_bridge::ptiff_sink_create(
            std::ffi::CString::new(path_s.clone()).unwrap().as_ptr(),
            &d as *const ptiff_image_descriptor,
        );
        assert!(!sink.is_null());
        let cols = crate::pixel_bridge::ptiff_sink_tile_columns(sink);
        let rows = crate::pixel_bridge::ptiff_sink_tile_rows(sink);
        let n = crate::pixel_bridge::ptiff_sink_tile_byte_size(sink);
        let buf = vec![0u8; n];
        for row in 0..rows {
            for col in 0..cols {
                assert_eq!(
                    crate::pixel_bridge::ptiff_sink_write_tile(
                        sink,
                        col,
                        row,
                        buf.as_ptr(),
                        buf.len()
                    ),
                    0
                );
            }
        }
        crate::pixel_bridge::ptiff_sink_close(sink);

        // Read the metadata descriptor back.
        let mut out = ptiff_image_descriptor::c_default();
        let rc = ptiff_open_path(
            std::ffi::CString::new(path_s).unwrap().as_ptr(),
            &mut out as *mut ptiff_image_descriptor,
        );
        assert_eq!(rc, 0);
        assert_eq!(out.width, 64);
        assert_eq!(out.height, 48);
        assert_eq!(out.has_tile_info, 1);
        assert_eq!(out.tile_info.tile_width, 16);
        assert_eq!(out.tile_info.tile_height, 16);
    }

    /// Serializes a `ptiff.*`-carrying StorageModel to a temp TIFF file and
    /// returns its path. Purely metadata (no pixel data is needed for the
    /// field view), mirroring the write side of the private tags 65001-65005.
    fn write_meta_file(dir: &std::path::Path, name: &str) -> String {
        let mut model = ptiff::StorageModel::new();
        // Baseline required image dimensions/pixel type.
        model.set_field("imageWidth", "8");
        model.set_field("imageHeight", "8");
        model.set_field("samplesPerPixel", "1");
        model.set_field("pixelType", "UInt8");
        model.set_field("compression", "None");
        // PTIFF extension fields (camera + spice), deliberately out of order so
        // the test proves lexicographic (BTreeMap) output.
        model.set_field("ptiff.camera.model", "pinhole");
        model.set_field("ptiff.camera.focal_length_y", "700.0");
        model.set_field("ptiff.camera.focal_length_x", "700.0");
        model.set_field("ptiff.spice.frame", "IAU_MOON");
        // A non-ptiff field must be excluded from the flat view.
        model.set_field("imageWidth", "8");

        let mut writer = ptiff_core::io::MemoryBinaryWriter::new();
        use ptiff_core::StorageBackend as _;
        ptiff_core::io::backend::tiff::TiffBackend
            .serialize_model(&model, &mut writer)
            .expect("serialize model");
        let bytes = writer.take_buffer();
        let path = dir.join(name);
        std::fs::write(&path, &bytes).expect("write fixture");
        path.to_str().unwrap().to_string()
    }

    /// Reads the C-string at `p` into an owned `String` (test helper only).
    fn read_cstr(p: *mut c_char) -> String {
        if p.is_null() {
            return String::new();
        }
        // Safety: p is a valid NUL-terminated C string owned by the caller.
        unsafe { std::ffi::CStr::from_ptr(p) }
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn open_path_fields_reads_ptiff_extension_fields() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_meta_file(dir.path(), "fields.tif");
        let path_c = std::ffi::CString::new(path).unwrap();

        let mut out: *mut ptiff_field = std::ptr::null_mut();
        let mut out_count: c_int = 0;
        let rc = ptiff_open_path_fields(
            path_c.as_ptr(),
            &mut out as *mut *mut ptiff_field,
            &mut out_count as *mut c_int,
        );
        assert_eq!(rc, 0);
        assert!(!out.is_null());
        assert_eq!(out_count, 4);

        // Safety: out points to an array of `out_count` ptiff_fields we own.
        let fields = unsafe { std::slice::from_raw_parts(out, out_count as usize) };
        let pairs: Vec<(String, String)> = fields
            .iter()
            .map(|f| (read_cstr(f.key), read_cstr(f.value)))
            .collect();

        // Lexicographic order by key.
        let expected = vec![
            ("ptiff.camera.focal_length_x", "700.0"),
            ("ptiff.camera.focal_length_y", "700.0"),
            ("ptiff.camera.model", "pinhole"),
            ("ptiff.spice.frame", "IAU_MOON"),
        ];
        assert_eq!(
            pairs,
            expected
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect::<Vec<_>>()
        );

        // Free exactly once (symmetric deallocator).
        ptiff_fields_free(out, out_count);
    }

    #[test]
    fn open_path_fields_empty_file_returns_null_zero() {
        let dir = tempfile::tempdir().unwrap();
        // A model with no ptiff.* fields.
        let mut model = ptiff::StorageModel::new();
        model.set_field("imageWidth", "8");
        model.set_field("imageHeight", "8");
        model.set_field("samplesPerPixel", "1");
        model.set_field("pixelType", "UInt8");
        let mut writer = ptiff_core::io::MemoryBinaryWriter::new();
        use ptiff_core::StorageBackend as _;
        ptiff_core::io::backend::tiff::TiffBackend
            .serialize_model(&model, &mut writer)
            .expect("serialize");
        let path = dir.path().join("empty.tif");
        std::fs::write(&path, writer.take_buffer()).unwrap();

        let path_c = std::ffi::CString::new(path.to_str().unwrap().to_string()).unwrap();
        let mut out: *mut ptiff_field = std::ptr::null_mut();
        let mut out_count: c_int = -1; // must be overwritten to 0
        let rc = ptiff_open_path_fields(
            path_c.as_ptr(),
            &mut out as *mut *mut ptiff_field,
            &mut out_count as *mut c_int,
        );
        assert_eq!(rc, 0);
        assert!(out.is_null());
        assert_eq!(out_count, 0);
        // NULL array is a documented no-op for fields_free.
        ptiff_fields_free(std::ptr::null_mut(), 0);
    }

    #[test]
    fn open_path_fields_null_arguments_is_invalid_argument() {
        let path_c = std::ffi::CString::new("x.tif").unwrap();
        let mut out: *mut ptiff_field = std::ptr::null_mut();
        let mut out_count: c_int = 0;
        assert_eq!(
            ptiff_open_path_fields(std::ptr::null(), &mut out, &mut out_count),
            -(ptiff_error_code::PTIFF_ERROR_INVALID_ARGUMENT as i32)
        );
        // null out
        assert_eq!(
            ptiff_open_path_fields(path_c.as_ptr(), std::ptr::null_mut(), &mut out_count),
            -(ptiff_error_code::PTIFF_ERROR_INVALID_ARGUMENT as i32)
        );
        // null out_count
        assert_eq!(
            ptiff_open_path_fields(path_c.as_ptr(), &mut out, std::ptr::null_mut()),
            -(ptiff_error_code::PTIFF_ERROR_INVALID_ARGUMENT as i32)
        );
    }

    #[test]
    fn open_path_fields_missing_file_is_not_found() {
        let path_c = std::ffi::CString::new("/nonexistent/definitely_missing.tif").unwrap();
        let mut out: *mut ptiff_field = std::ptr::null_mut();
        let mut out_count: c_int = 0;
        let rc = ptiff_open_path_fields(
            path_c.as_ptr(),
            &mut out as *mut *mut ptiff_field,
            &mut out_count as *mut c_int,
        );
        assert_eq!(rc, -(ptiff_error_code::PTIFF_ERROR_NOT_FOUND as i32));
        // out/out_count left untouched on failure.
        assert!(out.is_null());
        assert_eq!(out_count, 0);
    }

    #[test]
    fn fields_free_round_trips_heap_allocation() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_meta_file(dir.path(), "free.tif");
        let path_c = std::ffi::CString::new(path).unwrap();
        let mut out: *mut ptiff_field = std::ptr::null_mut();
        let mut out_count: c_int = 0;
        let rc = ptiff_open_path_fields(
            path_c.as_ptr(),
            &mut out as *mut *mut ptiff_field,
            &mut out_count as *mut c_int,
        );
        assert_eq!(rc, 0);
        // Free (which undoes the leaked Vecs+strings); must not double-free
        // because the test reads before freeing.
        ptiff_fields_free(out, out_count);
    }
}
