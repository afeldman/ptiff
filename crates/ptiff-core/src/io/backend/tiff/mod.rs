//! TIFF/BigTIFF on-disk format layer.
//!
//! Mirrors the C++ `ptiff::io::backend::tiff` namespace (see
//! `libptiff/include/ptiff/io/backend/tiff/` and the corresponding `.cpp`
//! files). This is the **format** layer: byte-order helpers, header
//! read/write, tag/field-type enums, pixel-type mapping, and the IFD parser
//! — all working over the dependency-free [`crate::io::BinaryReader`] /
//! [`crate::io::BinaryWriter`] transports.
//!
//! The C++ oracle also contains a compile-time **policy** layer
//! (`tiff_policy_*.hpp`: `PixelValue`, `ClassicContainer`/`BigTiffContainer`,
//! `NonePolicy`/`PackBitsPolicy`/`LzwPolicy`, `StripPolicy`/`TiledPolicy`,
//! `UInt8Policy`/`UInt16Policy`/`UInt32Policy`/`Float32Policy`, and
//! `TiffPolicyWriter`). This layer is deliberately **not** mirrored in Rust:
//! the policies are `constexpr` templates used only by their own isolated unit
//! test (`tiff_policy_test.cpp`) with no production callers, and every value
//! they encode (header sizes, bits/sample-format per pixel type, tile
//! dimensions, compression tag values) is already covered by the equivalent
//! runtime constructs here (`header`, `pixel_format`, `directory_writer`).
//! Replicating those constants as a Rust type/constant layer would add
//! maintenance burden without new capability, so Phase B of the C++ plan is
//! closed as "functionally covered by the runtime writer".
//!
//! This module is only compiled when the `tiff-backend` feature is enabled.

pub mod checked_arithmetic;
pub mod compression;
pub mod directory;
pub mod directory_writer;
pub mod endian;
pub mod header;
pub mod ifd;
pub mod ifd_writer;
pub mod image_sink;
pub mod image_source;
#[cfg(feature = "parallel")]
pub mod parallel;
pub mod pixel_format;
pub mod ptiff_metadata;
pub mod tag;
pub mod tiff_backend;

pub use checked_arithmetic::{checked_add_u64, checked_mul_u64, K_MAX_TAG_COUNT};
pub use compression::{
    apply_horizontal_differencing, decode_lzw, decode_pack_bits, encode_lzw, encode_pack_bits,
    undo_horizontal_differencing,
};
pub use directory::{
    interpret_tiff_ifd, to_storage_model, TiffCompression, TiffDirectory, TiffPredictor,
    TileByteRange,
};
pub use directory_writer::{
    plan_tiff_write, plan_tiff_write_multi, TiffFileWritePlan, TiffWritePlan,
};
pub use endian::{read_u16, read_u32, read_u64, write_u16, write_u32, write_u64, Endian};
pub use header::{read_tiff_header, TiffHeader};
pub use ifd::{read_tiff_ifd, TiffIfd};
pub use ifd_writer::{
    tiff_ifd_byte_size, value_slot_offsets_relative, write_tiff_ifd, TiffIfdEntryToWrite,
};
pub use image_sink::TiffImageSink;
pub use image_source::TiffImageSource;
#[cfg(feature = "parallel")]
pub use parallel::{decode_all, encode_all};
pub use pixel_format::{
    bits_per_sample_for, bytes_per_sample, pixel_type_field_value, pixel_type_from_field_value,
    require_uniform_bits_per_sample, resolve_pixel_type, sample_format_for,
};
pub use ptiff_metadata::{
    decode_metadata_payload, encode_metadata_payload, records_from_storage_model, MetadataRecord,
    K_PTIFF_MAGIC, K_PTIFF_METADATA_VERSION,
};
pub use tag::{
    field_type_size, field_type_size_raw, FieldType, RawTagEntry, TagId, K_PRIVATE_TAG_BASE,
};
pub use tiff_backend::TiffBackend;
