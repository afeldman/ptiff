//! Dependency-free TIFF compression codecs (PackBits, LZW, Predictor=2).
//!
//! Mirrors the `ptiff::compression` namespace for the codecs that require no
//! external dependency. Deflate (zlib) and JPEG pull in third-party libraries
//! and are exposed separately behind the `tiff-codecs` feature; this module
//! stays dependency-free so the `tiff-backend` feature alone keeps the default
//! build free of external crates.

#[cfg(feature = "tiff-codecs")]
pub mod deflate;
#[cfg(feature = "tiff-codecs")]
pub mod jpeg;
pub mod lzw;
pub mod packbits;
pub mod predictor;

#[cfg(feature = "tiff-codecs")]
pub use deflate::{decode_deflate, encode_deflate};
#[cfg(feature = "tiff-codecs")]
pub use jpeg::{decode_jpeg, encode_jpeg};
pub use lzw::{decode_lzw, encode_lzw};
pub use packbits::{decode_pack_bits, encode_pack_bits};
pub use predictor::{apply_horizontal_differencing, undo_horizontal_differencing};
