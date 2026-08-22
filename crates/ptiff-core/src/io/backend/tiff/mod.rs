//! TIFF/BigTIFF on-disk format layer.
//!
//! Mirrors the C++ `ptiff::io::backend::tiff` namespace (see
//! `libptiff/include/ptiff/io/backend/tiff/` and the corresponding `.cpp`
//! files). This is the **format** layer: byte-order helpers, header
//! read/write, tag/field-type enums, pixel-type mapping, and the IFD parser
//! — all working over the dependency-free [`crate::io::BinaryReader`] /
//! [`crate::io::BinaryWriter`] transports.
//!
//! Only this compile-time dependency-free portion is implemented so far; the
//! writer policies, directory writer and image sink/source that build on top
//! are planned as the next increment.
//!
//! This module is only compiled when the `tiff-backend` feature is enabled.

pub mod checked_arithmetic;
pub mod directory;
pub mod endian;
pub mod header;
pub mod ifd;
pub mod ifd_writer;
pub mod pixel_format;
pub mod ptiff_metadata;
pub mod tag;

pub use checked_arithmetic::{checked_add_u64, checked_mul_u64, K_MAX_TAG_COUNT};
pub use directory::{
    interpret_tiff_ifd, to_storage_model, TiffCompression, TiffDirectory, TiffPredictor,
    TileByteRange,
};
pub use endian::{read_u16, read_u32, read_u64, write_u16, write_u32, write_u64, Endian};
pub use header::{read_tiff_header, TiffHeader};
pub use ifd::{read_tiff_ifd, TiffIfd};
pub use ifd_writer::{tiff_ifd_byte_size, write_tiff_ifd, TiffIfdEntryToWrite};
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
