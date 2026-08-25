//! PTIFF spatial geometry: domain values, frame semantics, and lie-algebra
//! wrappers.
//!
//! This module mirrors the C++ geometry oracle (`libptiff/include/ptiff/geometry/`)
//! and extends it per GEOMETRY-FOUNDATION.md with:
//!   - the `multicalc` math kernel (SO(3)/SE(3), quaternions, Twist/Wrench),
//!   - the PTIFF domain values (storage PODs + camera / planet / CRS / ...),
//!   - Rust-side frame semantics ([`Frame`]/[`FramePair`]) and SE(3) wrappers
//!     ([`Pose`], [`Screw`]).
//!
//! Per the migration oracle, the storage-value types (`Vec3`, `Quaternion`,
//! `Extrinsics`, `Intrinsics`) remain the canonical serialized representation;
//! `Pose`/`Screw` add the computational SE(3)/screw wrappers on top.

mod camera;
mod coordinate_reference_system;
mod ellipsoid;
mod extrinsics;
mod frames;
mod intrinsics;
mod lens_model;
mod marshal;
mod planet;
mod pose;
mod projection;
mod quaternion;
mod scene_geometry;
mod screw;
mod spice;
mod vector3;

pub use camera::{Camera, ExtrinsicsMatrix, IntrinsicsMatrix, ProjectionMatrix, RotationMatrix};
pub use coordinate_reference_system::CoordinateReferenceSystem;
pub use ellipsoid::Ellipsoid;
pub use extrinsics::Extrinsics;
pub use frames::{Frame, FramePair};
pub use intrinsics::Intrinsics;
pub use lens_model::{lens_model_kind_from_str, lens_model_kind_str, LensModel, LensModelKind};
/// Typed marshalling between the camera / CRS / geometry domain classes
/// (`camera_fields`, `camera_from_model`, `crs_fields`, `crs_from_model`,
/// `geometry_fields`, `geometry_from_model`) and the flat
/// `ptiff.<domain>.<key>` storage-model field convention.
pub use marshal::{
    camera_fields, camera_from_model, crs_fields, crs_from_model, geometry_fields,
    geometry_from_model,
};
pub use planet::Planet;
pub use pose::Pose;
pub use projection::{Projection, ProjectionKind};
pub use quaternion::Quaternion;
pub use scene_geometry::{Geometry, GeometryKind};
pub use screw::{Screw, ScrewAxis, ScrewMotion};
pub use spice::SpiceState;
pub use vector3::Vec3;

#[cfg(test)]
mod serde_roundtrip {
    //! Serialization MVP (GEOMETRY-FOUNDATION.md §8 Phase III): the serde-capable
    //! geometry values round-trip through JSON losslessly.

    use super::*;

    /// Round-trips a serde-serializable value through JSON and asserts structural
    /// equality.
    fn roundtrip<T>(value: &T, label: &str)
    where
        T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
    {
        let json =
            serde_json::to_string(value).unwrap_or_else(|e| panic!("{label}: serialize: {e}"));
        let back: T =
            serde_json::from_str(&json).unwrap_or_else(|e| panic!("{label}: deserialize: {e}"));
        assert_eq!(&back, value, "{label}: round-trip mismatch");
    }

    #[test]
    fn vec3_roundtrips() {
        roundtrip(&Vec3::new(1.0, 2.5, -3.0), "Vec3");
        roundtrip(&Vec3::ZERO, "Vec3::ZERO");
    }

    #[test]
    fn quaternion_roundtrips() {
        roundtrip(&Quaternion::from_euler_angles(0.3, -0.7, 1.2), "Quaternion");
        roundtrip(&Quaternion::IDENTITY, "Quaternion::IDENTITY");
    }

    #[test]
    fn extrinsics_and_intrinsics_roundtrip() {
        let e = Extrinsics::new(
            Quaternion::from_euler_angles(0.1, 0.2, 0.3),
            Vec3::new(100.0, 0.0, 500.0),
        );
        roundtrip(&e, "Extrinsics");
        roundtrip(&Intrinsics::new(1000.0, 1000.0, 640.0, 360.0), "Intrinsics");
    }

    #[test]
    fn camera_roundtrips() {
        let cam = Camera::from_model(
            "pinhole",
            Intrinsics::new(1000.0, 1001.0, 640.0, 360.0),
            Extrinsics::new(
                Quaternion::from_euler_angles(0.0, 0.0, 0.0),
                Vec3::new(1.0, 2.0, 3.0),
            ),
            "2026-08-21T12:34:56.000Z",
        );
        roundtrip(&cam, "Camera");
    }

    #[test]
    fn lens_and_projection_roundtrip() {
        let mut lens = LensModel::new(LensModelKind::Pinhole);
        lens.set_parameter("focal_length_mm", 35.0);
        roundtrip(&lens, "LensModel");

        let mut proj = Projection::new(ProjectionKind::Stereographic);
        proj.set_parameter("central_meridian_deg", 0.0);
        roundtrip(&proj, "Projection");
        roundtrip(&ProjectionKind::Equirectangular, "ProjectionKind");
        roundtrip(&GeometryKind::Unspecified, "GeometryKind");
    }

    #[test]
    fn ellipsoid_roundtrips() {
        roundtrip(&Ellipsoid::new(1_737_400.0, 1_737_400.0), "Ellipsoid");
    }

    #[test]
    fn frame_is_at_least_serializable() {
        // Frame/FramePair are Serialize-only (static str, no Deserialize).
        let f = Frame::J2000;
        let json = serde_json::to_string(&f).expect("Frame serializes");
        assert!(!json.is_empty());
        let p = FramePair::new(Frame::J2000, Frame::SPACECRAFT);
        let _ = serde_json::to_string(&p).expect("FramePair serializes");
    }
}
