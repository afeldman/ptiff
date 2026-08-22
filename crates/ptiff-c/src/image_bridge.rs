//! `ptiff_image_*` handle surface (`bindings/c/ptiff_image_bridge.h`).
//!
//! The opaque `ptiff_image` is a heap-owned metadata model (no pixels) built
//! from a C descriptor. It mirrors the C++ `libptiff_c` behaviour of wrapping
//! a `ptiff::Image` (the move-only model object) behind an opaque pointer the
//! consumer never dereferences. The accessors are read-only for the lifetime
//! of the handle.

use crate::types::{descriptor_from_c, ptiff_image_descriptor, ptiff_pixel_type, ptiff_tile_info};
use ptiff::ImageDescriptor;

/// Opaque, heap-owned image metadata model.
///
/// Never dereferenced by C callers; every access goes through the `ptiff_image_*`
/// functions below. The handle owns its [`ImageDescriptor`] for its whole life.
pub struct PtiffImage {
    /// The image metadata model backing this handle.
    descriptor: ImageDescriptor,
}

impl PtiffImage {
    /// Wraps a core descriptor into a heap handle.
    fn from_descriptor(d: ImageDescriptor) -> *mut PtiffImage {
        Box::into_raw(Box::new(PtiffImage { descriptor: d }))
    }
}

/// Creates a heap-owned image from a C descriptor. Returns `NULL` on a null
/// descriptor argument.
///
/// # Safety
///
/// `desc` must be null or point to a valid, initialised `ptiff_image_descriptor`
/// for the duration of the call. The returned pointer is owned by the caller
/// and must be released with [`ptiff_image_destroy`]. If `desc` is a valid
/// non-null pointer (the only path that allocates), the returned raw pointer is
/// a freshly boxed, non-null heap allocation with unique ownership.
#[no_mangle]
pub extern "C" fn ptiff_image_create(desc: *const ptiff_image_descriptor) -> *mut PtiffImage {
    if desc.is_null() {
        return std::ptr::null_mut();
    }
    // Safety: the caller promises a valid, initialised descriptor (documented
    // in the C header). We read it once into an owned ImageDescriptor.
    let c_desc = unsafe { &*desc };
    let d = descriptor_from_c(c_desc);
    PtiffImage::from_descriptor(d)
}

/// Frees a handle returned by [`ptiff_image_create`]. `NULL` is a no-op.
///
/// # Safety
///
/// `img` must be null or a pointer previously returned by [`ptiff_image_create`]
/// that has not already been freed.
#[no_mangle]
pub extern "C" fn ptiff_image_destroy(img: *mut PtiffImage) {
    if !img.is_null() {
        // Safety: a non-null handle is unique-owned and returned by create;
        // dropping is the symmetric free.
        drop(unsafe { Box::from_raw(img) });
    }
}

/// Returns the image width. `NULL` handle -> 0.
#[no_mangle]
pub extern "C" fn ptiff_image_width(img: *const PtiffImage) -> u32 {
    let Some(img) = handle(img) else { return 0 };
    img.descriptor.width
}

/// Returns the image height. `NULL` handle -> 0.
#[no_mangle]
pub extern "C" fn ptiff_image_height(img: *const PtiffImage) -> u32 {
    let Some(img) = handle(img) else { return 0 };
    img.descriptor.height
}

/// Returns the C pixel-type enum value.
#[no_mangle]
pub extern "C" fn ptiff_image_pixel_type(img: *const PtiffImage) -> i32 {
    let Some(img) = handle(img) else {
        return ptiff_pixel_type::PTIFF_PIXEL_UINT8 as i32;
    };
    crate::types::pixel_type_to_c(img.descriptor.pixel_type) as i32
}

/// Returns the channel count (samples/pixel).
#[no_mangle]
pub extern "C" fn ptiff_image_channel_count(img: *const PtiffImage) -> u32 {
    let Some(img) = handle(img) else { return 0 };
    img.descriptor.channel_count
}

/// Reads the ground-sample distance. Returns 1 and sets `*out` if present,
/// else returns 0 and leaves `*out` untouched.
///
/// # Safety
///
/// `out` must be null or point to a writable `double` when the GSD is present.
/// A null `out` yields 0 (absent) regardless.
#[no_mangle]
pub extern "C" fn ptiff_image_gsd(img: *const PtiffImage, out: *mut f64) -> i32 {
    let Some(image) = handle(img) else { return 0 };
    let Some(gsd) = image.descriptor.ground_sample_distance_meters else {
        return 0;
    };
    if out.is_null() {
        return 0;
    }
    // Safety: non-null out is documented to point at a writable double; we
    // write a single f64 (plain scalar, no aliasing hazard to the caller's
    // object graph beyond the intended slot).
    unsafe { *out = gsd };
    1
}

/// Reads the tile info. Returns 1 and fills `*out` if present, else 0.
///
/// # Safety
///
/// `out` must be null or point to a writable `ptiff_tile_info` when the tile
/// info is present.
#[no_mangle]
pub extern "C" fn ptiff_image_tile_info(img: *const PtiffImage, out: *mut ptiff_tile_info) -> i32 {
    let Some(image) = handle(img) else { return 0 };
    let Some(tile) = image.descriptor.tile_info else {
        return 0;
    };
    if out.is_null() {
        return 0;
    }
    // Safety: non-null out points at a writable ptiff_tile_info (sizeof two
    // u32); we store two plain integers.
    unsafe {
        *out = ptiff_tile_info {
            tile_width: tile.tile_width,
            tile_height: tile.tile_height,
        };
    }
    1
}

/// Reads the compression. Returns 1 and sets `*out` if present, else 0.
///
/// # Safety
///
/// `out` must be null or point to a writable `int` when the compression is present.
#[no_mangle]
pub extern "C" fn ptiff_image_compression(img: *const PtiffImage, out: *mut i32) -> i32 {
    let Some(image) = handle(img) else { return 0 };
    let Some(c) = image.descriptor.compression else {
        return 0;
    };
    if out.is_null() {
        return 0;
    }
    // Safety: non-null out points at a writable int; we store a plain integer.
    unsafe { *out = crate::types::compression_to_c(c) as i32 };
    1
}

/// Borrows a handle, returning `None` for a null pointer.
fn handle<'a>(img: *const PtiffImage) -> Option<&'a PtiffImage> {
    if img.is_null() {
        None
    } else {
        // Safety: a non-null handle is unique-owned by the caller and valid for
        // as long as it was not destroyed; the reference is derived for the
        // duration of one accessor call, while the caller holds the handle.
        Some(unsafe { &*img })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ptiff_compression_kind, ptiff_image_descriptor, ptiff_tile_info};

    fn desc(width: u32, height: u32) -> ptiff_image_descriptor {
        let mut d = ptiff_image_descriptor::c_default();
        d.width = width;
        d.height = height;
        d.channel_count = 1;
        d
    }

    #[test]
    fn create_null_returns_null() {
        assert!(ptiff_image_create(std::ptr::null()).is_null());
    }

    #[test]
    fn accessors_report_descriptor_fields() {
        let mut d = desc(40, 30);
        d.pixel_type = ptiff_pixel_type::PTIFF_PIXEL_UINT16 as i32;
        d.channel_count = 3;
        d.has_gsd = 1;
        d.gsd = 1.5;
        d.has_tile_info = 1;
        d.tile_info = ptiff_tile_info {
            tile_width: 16,
            tile_height: 16,
        };
        d.has_compression = 1;
        d.compression = ptiff_compression_kind::PTIFF_COMPRESSION_LZW as i32;

        let img = ptiff_image_create(&d);
        assert!(!img.is_null());
        assert_eq!(ptiff_image_width(img), 40);
        assert_eq!(ptiff_image_height(img), 30);
        assert_eq!(
            ptiff_image_pixel_type(img),
            ptiff_pixel_type::PTIFF_PIXEL_UINT16 as i32
        );
        assert_eq!(ptiff_image_channel_count(img), 3);

        let mut gsd = -1.0;
        assert_eq!(ptiff_image_gsd(img, &mut gsd), 1);
        assert_eq!(gsd, 1.5);

        let mut tile = ptiff_tile_info {
            tile_width: 0,
            tile_height: 0,
        };
        assert_eq!(ptiff_image_tile_info(img, &mut tile), 1);
        assert_eq!(tile.tile_width, 16);
        assert_eq!(tile.tile_height, 16);

        let mut comp = -1;
        assert_eq!(ptiff_image_compression(img, &mut comp), 1);
        assert_eq!(comp, ptiff_compression_kind::PTIFF_COMPRESSION_LZW as i32);

        // img was created above and is still alive; destroy frees it.
        ptiff_image_destroy(img);
    }

    #[test]
    fn absent_optionals_return_zero() {
        let mut d = desc(4, 4);
        d.has_gsd = 0;
        d.has_tile_info = 0;
        d.has_compression = 0;
        let img = ptiff_image_create(&d);
        assert!(!img.is_null());
        let mut gsd = 99.0;
        assert_eq!(ptiff_image_gsd(img, &mut gsd), 0);
        assert_eq!(gsd, 99.0, "out is untouched when the field is absent");
        assert_eq!(ptiff_image_compression(img, std::ptr::null_mut()), 0);
        // img was created above and is still alive; destroy frees it.
        ptiff_image_destroy(img);
    }

    #[test]
    fn null_handle_accessors_read_zero() {
        assert_eq!(ptiff_image_width(std::ptr::null()), 0);
        assert_eq!(ptiff_image_height(std::ptr::null()), 0);
        assert_eq!(
            ptiff_image_pixel_type(std::ptr::null()),
            ptiff_pixel_type::PTIFF_PIXEL_UINT8 as i32
        );
        assert_eq!(ptiff_image_channel_count(std::ptr::null()), 0);
    }
}
