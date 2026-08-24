//! Explicit coordinate-frame labels for PTIFF spatial transforms.
//!
//! Per GEOMETRY-FOUNDATION.md, a bare SE(3) transform does not say whether it
//! means `camera → spacecraft` or `spacecraft → J2000`. PTIFF attaches an
//! explicit **frame pair** (`from`/`to`) to its [`Pose`](crate::geometry::Pose)
//! so the scientific meaning is preserved in the type system without heavy
//! type-level ceremony.
//!
//! A [`Frame`] holds a `'static` identifier string, so it is cheap and `Copy`
//! and can be stored/built as a const. Well-known frames are provided as
//! constants; the frame set is **open-ended** (any `'static` label is valid),
//! matching the C++ `Planet` extension-by-instance philosophy.
//!
//! Serialization: [`Frame`]/[`FramePair`] derive `serde::Serialize` (for
//! diagnostics/audit), but **not** `Deserialize`: a deserialised value cannot
//! return a `'static` borrow. Frame persistence is orthogonal to the PTIFF file
//! format and is deferred (see GEOMETRY-FOUNDATION.md §7).

use std::fmt;

/// A named coordinate frame.
///
/// A cheap `Copy` handle carrying a `'static` identifier string. Common frames
/// are provided as constants ([`Frame::IAU_MOON`], [`Frame::J2000`], ...), but
/// the frame set is **open-ended**: any `'static` label is a valid frame, so
/// new bodies or regions need no library change.
///
/// Frames are compared by their identifier. Two handles with the same name are
/// equal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Frame {
    id: &'static str,
}

impl Frame {
    /// The inertial J2000 equatorial frame.
    pub const J2000: Self = Frame { id: "J2000" };
    /// The Moon's canonical IAU frame.
    pub const IAU_MOON: Self = Frame { id: "IAU_MOON" };
    /// The Earth's canonical IAU frame.
    pub const IAU_EARTH: Self = Frame { id: "IAU_EARTH" };
    /// A spacecraft body frame (attitude reference).
    pub const SPACECRAFT: Self = Frame { id: "spacecraft" };
    /// A camera body frame.
    pub const CAMERA: Self = Frame { id: "camera" };

    /// Constructs a named frame from a `'static` identifier string.
    ///
    /// The identifier is stored as-given (case-sensitive); it is not normalised.
    #[inline]
    pub const fn new(id: &'static str) -> Self {
        Self { id }
    }

    /// The frame's identifier string.
    #[inline]
    pub const fn id(&self) -> &'static str {
        self.id
    }
}

impl fmt::Display for Frame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id)
    }
}

/// A directed frame pair, e.g. `from = Frame::CAMERA`, `to = Frame::SPACECRAFT`
/// denotes the transform **camera → spacecraft**.
///
/// Frames are written as `from → to`; `FramePair::new(from, to)` means "a pose
/// expressing coordinates of `from` in `to`". Reversing the transform swaps the
/// direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct FramePair {
    /// The source frame.
    pub from: Frame,
    /// The destination frame.
    pub to: Frame,
}

impl FramePair {
    /// Constructs a directed frame pair from the source and destination frames.
    #[inline]
    pub const fn new(from: Frame, to: Frame) -> Self {
        Self { from, to }
    }
}

impl fmt::Display for FramePair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} → {}", self.from, self.to)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_string_constants() {
        assert_eq!(Frame::IAU_MOON.id(), "IAU_MOON");
        assert_eq!(Frame::J2000.id(), "J2000");
        assert_eq!(Frame::CAMERA.id(), "camera");
    }

    #[test]
    fn frames_equal_by_identifier() {
        assert_eq!(Frame::new("IAU_MOON"), Frame::IAU_MOON);
        assert_ne!(Frame::new("IAU_MOON"), Frame::new("IAU_MARS"));
    }

    #[test]
    fn arbitrary_identifiers_are_valid() {
        let frame = Frame::new("IAU_PHOBOS");
        assert_eq!(frame.id(), "IAU_PHOBOS");
    }

    #[test]
    fn frame_is_copy() {
        let a = Frame::CAMERA;
        let b = a;
        assert_eq!(a, b);
    }

    #[test]
    fn frame_pair_is_directed() {
        let cam_to_sc = FramePair::new(Frame::CAMERA, Frame::SPACECRAFT);
        assert_eq!(cam_to_sc.from, Frame::CAMERA);
        assert_eq!(cam_to_sc.to, Frame::SPACECRAFT);
        // The reverse is a distinct pair.
        let sc_to_cam = FramePair::new(Frame::SPACECRAFT, Frame::CAMERA);
        assert_ne!(cam_to_sc, sc_to_cam);
    }

    #[test]
    fn display_uses_direction_arrow() {
        let pair = FramePair::new(Frame::CAMERA, Frame::SPACECRAFT);
        assert_eq!(pair.to_string(), "camera → spacecraft");
    }
}
