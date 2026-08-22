//! PTIFF spatial geometry: domain values plus frame semantics.
//!
//! This module mirrors the C++ geometry oracle
//! (`libptiff/include/ptiff/geometry/`) and extends it with a Rust-side frame
//! abstraction (see [`frames`]) per GEOMETRY-FOUNDATION.md.
//!
//! The mathematical kernel is provided by the external `multicalc` crate
//! (SO(3)/SE(3), quaternions, Twist/Wrench); this module owns the **domain
//! semantics** — the storage values (`Vec3`, `Quaternion`, `Extrinsics`,
//! `Intrinsics`) and the explicit `Frame` labels attached to poses.
//!
//! Per the migration oracle, the storage-value types are deliberately minimal
//! and self-contained; the math operations live in the [`Pose`] / SE(3)
//! wrappers in later phases.

mod extrinsics;
mod frames;
mod intrinsics;
mod quaternion;
mod vector3;

pub use extrinsics::Extrinsics;
pub use frames::{Frame, FramePair};
pub use intrinsics::Intrinsics;
pub use quaternion::Quaternion;
pub use vector3::Vec3;
