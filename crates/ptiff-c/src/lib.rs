//! # `ptiff-c` — the PTIFF 1.0 C ABI over the Rust core
//!
//! This crate implements the stable `ptiff_*` C ABI. The Rust `extern "C"`
//! signatures in this crate are **the single source of truth** for the header:
//! cbindgen (see `build.rs` + `cbindgen.toml`) authorises `target/ptiff_c.h`
//! from them (plan §7.4, updated 2026-08-24 — the hand-maintained
//! `bindings/c/*.h` veneer has been deleted).
//!
//! ## Architecture
//!
//! - **Opaque handles.** All resources (`ptiff_source`, `ptiff_sink`,
//!   `ptiff_image`) are opaque, heap-owned pointers; the consumer never
//!   dereferences them. Each handle is released with its `_close`/`_destroy`
//!   counterpart (`NULL` = no-op).
//! - **Error codes.** Every fallible function returns a negative
//!   [`crate::error::ptiff_error_code`] on failure (`0` on success), matching
//!   the existing `libptiff_c` contract so foreign runtimes (Go, Ruby, Python,
//!   Octave) switch on the raw integers unchanged.
//! - **Ownership.** String-ish outputs returned by the callee are released with
//!   [`crate::bridge::ptiff_free_string`]; out-structs are written into
//!   caller-owned buffers.
//! - **No C++ types cross the boundary.** The surface is pure `extern "C"`
//!   over `stdint.h`/`stddef.h` types, so any FFI runtime can link it.
//!
//! ## Slice scope (Phase 7, erster Slice)
//!
//! Implemented over the idiomatic `ptiff` crate and the core's
//! `Tiff`/`Scene`/`ImageDescriptor`/`TileLayout` types:
//!
//! - version (`ptiff_version.h`)
//! - backend names + `ptiff_free_string` (`ptiff_bridge.h`)
//! - image handle (`ptiff_image_bridge.h`)
//! - pixel `ptiff_source_*` read + `ptiff_sink_*` write (`ptiff_pixel_bridge.h`)
//! - `ptiff_open_path` descriptor (`ptiff_metadata.h`)
//! - logger (`ptiff_logger.h` — forwards to the dependency-free `ptiff-core` logger)
//! - camera (`ptiff_camera.h` `ptiff_open_path_camera` read + `ptiff_sink_create_camera`
//!   write, on top of the core's structured camera domain)
//! - flattened `ptiff.*` extension fields (`ptiff_metadata.h`
//!   `ptiff_open_path_fields` + `ptiff_fields_free`, decoded from the private
//!   tags 65001-65005 via the core's format-neutral `StorageModel`)

// `ptiff-c` is the ABI boundary: it deliberately wraps core resources in
// pointer-typed opaque handles, so it cannot `forbid(unsafe_code)` the way the
// core (and the idiomatic `ptiff` crate) can. Every unsafe block is small,
// documented with a Safety comment, and scoped to the single crossing of a
// caller-supplied pointer.
//
// `missing_docs` is relaxed here: the structs/enums on this crate mirror the
// C ABI that cbindgen authorises into `target/ptiff_c.h` from these very
// declarations — the generated header is the documented specification foreign
// runtimes read. Requiring redundant per-field docs on every ABI-derived
// struct would add noise without keeping the ABI any safer; the
// module/function-level docs above carry the meaningful guidance.
//
// `not_unsafe_ptr_arg_deref` is allowed because these `#[no_mangle] extern "C"`
// functions are the ABI boundary: their callers are *C* runtimes governed by
// the pointer contracts documented in the generated header (non-null out-params,
// caller-owned buffers, NULL = no-op), not Rust callers whom a `unsafe fn`
// signature would protect. Marking them `unsafe extern "C"` would also force
// every `#[test]` (which cannot be `unsafe fn`) to re-wrap the calls, at the
// cost of readability without gaining real safety for the intended audience.
#![allow(missing_docs, clippy::not_unsafe_ptr_arg_deref)]

pub mod bridge;
pub mod camera;
pub mod error;
pub mod image_bridge;
pub mod logger;
pub mod metadata;
pub mod pixel_bridge;
pub mod types;
pub mod version;

// Re-export the FFI-safe value types so foreign tooling and our own tests can
// name them without reaching into the internals.
pub use camera::ptiff_camera;
pub use types::{
    ptiff_compression_kind, ptiff_image_descriptor, ptiff_pixel_type, ptiff_tile_info,
};
