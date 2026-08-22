//! Dependency-free TIFF compression codecs (PackBits, LZW, Predictor=2).
//!
//! Mirrors the `ptiff::compression` namespace for the codecs that require no
//! external dependency. Deflate (zlib) and JPEG pull in third-party libraries
//! and are exposed separately behind additional features; this module stays
//! dependency-free so the `tiff-backend` feature alone keeps the default build
//! free of external crates.

pub mod lzw;
pub mod packbits;
pub mod predictor;

pub use lzw::{decode_lzw, encode_lzw};
pub use packbits::{decode_pack_bits, encode_pack_bits};
pub use predictor::{apply_horizontal_differencing, undo_horizontal_differencing};
