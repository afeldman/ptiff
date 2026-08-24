//! Reads a PDS4 document's seek header + XML label from a [`BinaryReader`]
//! and derives the absolute pixel origin.
//!
//! Mirrors `libptiff/src/io/backend/pds4/pds4_document.cpp` /
//! `pds4_document.hpp`. Layout of a PDS4 document produced/consumed by
//! [`super::Pds4Backend`]:
//!
//! ```text
//! [0 .. 8)       label byte length N, little-endian u64 (the 'seek header')
//! [8 .. 8+N)     the PDS4 label (UTF-8 XML, exactly N bytes)
//! [8+N ..)       the image pixel block (tile-addressable, see image_info_from_model)
//! ```
//!
//! The seek header is the only non-XML part and exists so the absolute pixel
//! offsets stay deterministic without circularly depending on the label's
//! own text length (the same mechanism the memory backend uses for its
//! document header).

use crate::io::BinaryReader;
use crate::io::StorageModel;
use crate::{Error, Result};

use super::label::{parse_label, HEADER_SIZE};

/// Upper bound on a plausible label byte length: guards against a corrupt
/// seek header turning into a huge allocation (mirrors the C++
/// `labelBytes > (1ull << 24)` check).
const MAX_LABEL_BYTES: u64 = 1 << 24;

/// A parsed PDS4 document plus the byte length of its XML label (needed to
/// compute the absolute pixel origin, which sits directly after the label).
#[derive(Debug)]
pub struct Pds4Document {
    /// Flat per-image storage fields.
    pub image: StorageModel,
    /// Length of the label text, in bytes.
    pub label_bytes: u64,
}

/// Reads a PDS4 document (seek header + label) from `reader` and parses it.
///
/// A PDS4 document is self-describing from byte 0, and both
/// `deserialize_model` and `open_image_source` read it (possibly after a
/// prior read advanced the reader), so this always starts from the document
/// origin.
///
/// # Errors
///
/// Returns [`crate::ErrorCode::InvalidArgument`] on a truncated seek header,
/// an implausible label byte length, a truncated label, or a malformed
/// label (see [`parse_label`]).
pub fn read_document(reader: &mut dyn BinaryReader) -> Result<Pds4Document> {
    reader.seek(0)?;

    // 1. Seek header: little-endian u64 giving the label byte length.
    let mut header = [0u8; HEADER_SIZE as usize];
    let read_header = reader.read(&mut header)?;
    if read_header != header.len() {
        return Err(Error::invalid_argument("pds4: truncated seek header"));
    }
    let label_bytes = u64::from_le_bytes(header);
    if label_bytes == 0 || label_bytes > MAX_LABEL_BYTES {
        return Err(Error::invalid_argument(
            "pds4: implausible label byte length",
        ));
    }

    // 2. Label text of that length.
    let mut label = vec![0u8; label_bytes as usize];
    let read_label = reader.read(&mut label)?;
    if read_label != label.len() {
        return Err(Error::invalid_argument("pds4: truncated XML label"));
    }

    let image = parse_label(&label)?;
    Ok(Pds4Document { image, label_bytes })
}

/// Absolute byte offset of this image's pixel block (seek header + label
/// length).
#[must_use]
pub fn pixel_origin(label_bytes: u64) -> u64 {
    HEADER_SIZE + label_bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::MemoryBinaryReader;
    use crate::ErrorCode;

    fn image_model() -> StorageModel {
        let mut m = StorageModel::new();
        m.set_field("imageWidth", "32");
        m.set_field("imageHeight", "32");
        m.set_field("tileWidth", "16");
        m.set_field("tileHeight", "16");
        m.set_field("samplesPerPixel", "1");
        m.set_field("pixelType", "UInt8");
        m.set_field("compression", "None");
        m
    }

    fn encode_document(label: &[u8], pixels: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(label.len() as u64).to_le_bytes());
        bytes.extend_from_slice(label);
        bytes.extend_from_slice(pixels);
        bytes
    }

    #[test]
    fn reads_label_then_reports_pixel_origin() {
        let label = super::super::label::write_label(&image_model()).expect("write_label");
        let label_len = label.len() as u64;
        let bytes = encode_document(&label, &[0u8; 32 * 32]);

        let mut reader = MemoryBinaryReader::from_vec(bytes);
        let doc = read_document(&mut reader).expect("read_document");
        assert_eq!(doc.label_bytes, label_len);
        assert_eq!(pixel_origin(doc.label_bytes), HEADER_SIZE + label_len);
        assert_eq!(doc.image.field("imageWidth").unwrap(), "32");
    }

    #[test]
    fn truncated_seek_header_is_invalid_argument() {
        let mut reader = MemoryBinaryReader::from_vec(vec![1, 2, 3]);
        let err = read_document(&mut reader).expect_err("must fail");
        assert_eq!(err.code(), ErrorCode::InvalidArgument);
    }

    #[test]
    fn zero_label_length_is_invalid_argument() {
        let bytes = 0u64.to_le_bytes().to_vec();
        let mut reader = MemoryBinaryReader::from_vec(bytes);
        let err = read_document(&mut reader).expect_err("must fail");
        assert_eq!(err.code(), ErrorCode::InvalidArgument);
    }

    #[test]
    fn implausible_label_length_is_invalid_argument() {
        let bytes = (MAX_LABEL_BYTES + 1).to_le_bytes().to_vec();
        let mut reader = MemoryBinaryReader::from_vec(bytes);
        let err = read_document(&mut reader).expect_err("must fail");
        assert_eq!(err.code(), ErrorCode::InvalidArgument);
    }

    #[test]
    fn truncated_label_is_invalid_argument() {
        let mut bytes = 100u64.to_le_bytes().to_vec();
        bytes.extend_from_slice(b"<Product_Observational>");
        let mut reader = MemoryBinaryReader::from_vec(bytes);
        let err = read_document(&mut reader).expect_err("must fail");
        assert_eq!(err.code(), ErrorCode::InvalidArgument);
    }
}
