//! Byte transport and format-neutral storage abstractions.
//!
//! Mirrors the C++ `ptiff::io` layer's foundational, dependency-free types.

pub mod backend_capabilities;
// The concrete storage backends live in `backend/`, each gated by a feature.
#[cfg(feature = "memory-backend")]
pub mod backend;
pub mod binary_reader;
pub mod binary_writer;
pub mod deserializer;
pub mod image_sink;
pub mod image_source;
pub mod memory_binary_reader;
pub mod memory_binary_writer;
pub mod scene_deserializer;
pub mod scene_serializer;
pub mod serializer;
pub mod storage_backend;
pub mod storage_model;
pub mod tile_provider;

pub use backend_capabilities::BackendCapabilities;
pub use binary_reader::BinaryReader;
pub use binary_writer::BinaryWriter;
pub use deserializer::Deserializer;
pub use image_sink::ImageSink;
pub use image_source::ImageSource;
pub use memory_binary_reader::MemoryBinaryReader;
pub use memory_binary_writer::MemoryBinaryWriter;
pub use scene_deserializer::SceneDeserializer;
pub use scene_serializer::SceneSerializer;
pub use serializer::Serializer;
pub use storage_backend::StorageBackend;
pub use storage_model::StorageModel;
pub use tile_provider::TileProvider;
