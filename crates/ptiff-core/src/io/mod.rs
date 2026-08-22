//! Byte transport and format-neutral storage abstractions.
//!
//! Mirrors the C++ `ptiff::io` layer's foundational, dependency-free types.

pub mod binary_reader;
pub mod binary_writer;
pub mod storage_model;
pub mod tile_provider;

pub use binary_reader::BinaryReader;
pub use binary_writer::BinaryWriter;
pub use storage_model::StorageModel;
pub use tile_provider::TileProvider;
