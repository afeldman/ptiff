//! A scene: the composition of multiple related images.
//!
//! Mirrors `ptiff::Scene` (see `libptiff/include/ptiff/scene.hpp`).

use crate::id::ImageId;
use crate::image::Image;
use crate::image::ImageDescriptor;
use crate::{Error, Result};

/// A scene: the composition of multiple related images (e.g. a stereo pair, a
/// mosaic).
///
/// An owning container of [`Image`] objects. Each image is given a
/// monotonically increasing [`ImageId`] when added (0, 1, 2, ...). Copy is
/// intentionally not implemented; a `Scene` is cheap to move.
///
/// **Round-trip:** a `Scene` round-trips through `io::StorageModel`. Not every
/// `Image` field survives a TIFF round-trip: `tile_info` and
/// `ground_sample_distance_meters` are not preserved.
#[derive(Debug, Default)]
pub struct Scene {
    images: Vec<Image>,
    next_id: u64,
}

impl Scene {
    /// Constructs an empty scene (no images).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends an image built from `descriptor` and returns its new [`ImageId`].
    ///
    /// The returned id is the next value of the per-Scene counter starting at
    /// 0.
    pub fn add_image(&mut self, descriptor: ImageDescriptor) -> Result<ImageId> {
        // Matches the C++ oracle: images are appended unconditionally (no
        // degenerate-dimension validation here).
        let id = self.next_id;
        self.next_id += 1;
        self.images.push(Image::new(descriptor));
        Ok(ImageId::new(id))
    }

    /// Number of images in the scene.
    #[must_use]
    pub fn image_count(&self) -> usize {
        self.images.len()
    }

    /// Looks up an image by its [`ImageId`].
    ///
    /// # Errors
    ///
    /// Returns [`ErrorCode::NotFound`] if `id` was never returned by
    /// `add_image`.
    pub fn image(&self, id: ImageId) -> Result<&Image> {
        self.images
            .get(id.value() as usize)
            .ok_or_else(|| Error::not_found("Scene::image: no image with this id"))
    }

    /// Returns the image at `index` in scene order (the order they were added).
    ///
    /// # Errors
    ///
    /// Returns [`ErrorCode::OutOfRange`] if `index` is not in `[0, image_count())`.
    pub fn image_at(&self, index: usize) -> Result<&Image> {
        self.images
            .get(index)
            .ok_or_else(|| Error::out_of_range("Scene::image_at: index out of range"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ErrorCode;

    fn desc(width: u32, height: u32) -> ImageDescriptor {
        ImageDescriptor::new(width, height)
    }

    #[test]
    fn add_image_assigns_monotonic_ids_from_zero() {
        let mut scene = Scene::new();
        assert_eq!(scene.add_image(desc(64, 32)).unwrap(), ImageId::new(0));
        assert_eq!(scene.add_image(desc(128, 64)).unwrap(), ImageId::new(1));
        assert_eq!(scene.image_count(), 2);
    }

    #[test]
    fn image_lookup_by_id() {
        let mut scene = Scene::new();
        let id = scene.add_image(desc(64, 32)).unwrap();
        let img = scene.image(id).unwrap();
        assert_eq!(img.width(), 64);
        assert_eq!(img.height(), 32);
    }

    #[test]
    fn image_by_missing_id_is_not_found() {
        let scene = Scene::new();
        let err = scene.image(ImageId::new(42)).unwrap_err();
        assert_eq!(err.code(), ErrorCode::NotFound);
    }

    #[test]
    fn image_at_in_scene_order() {
        let mut scene = Scene::new();
        scene.add_image(desc(64, 32)).unwrap();
        scene.add_image(desc(128, 64)).unwrap();
        assert_eq!(scene.image_at(0).unwrap().width(), 64);
        assert_eq!(scene.image_at(1).unwrap().width(), 128);
    }

    #[test]
    fn image_at_out_of_range() {
        let scene = Scene::new();
        let err = scene.image_at(0).unwrap_err();
        assert_eq!(err.code(), ErrorCode::OutOfRange);
    }
}
