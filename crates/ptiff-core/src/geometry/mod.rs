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
mod planet;
mod pose;
mod projection;
mod quaternion;
mod scene_geometry;
mod screw;
mod vector3;

pub use camera::{Camera, ExtrinsicsMatrix, IntrinsicsMatrix, ProjectionMatrix, RotationMatrix};
pub use coordinate_reference_system::CoordinateReferenceSystem;
pub use ellipsoid::Ellipsoid;
pub use extrinsics::Extrinsics;
pub use frames::{Frame, FramePair};
pub use intrinsics::Intrinsics;
pub use lens_model::{LensModel, LensModelKind};
pub use planet::Planet;
pub use pose::Pose;
pub use projection::{Projection, ProjectionKind};
pub use quaternion::Quaternion;
pub use scene_geometry::{Geometry, GeometryKind};
pub use screw::{Screw, ScrewAxis, ScrewMotion};
pub use vector3::Vec3;
