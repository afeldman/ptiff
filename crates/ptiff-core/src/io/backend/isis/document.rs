//! Scans an ISIS3 document from the start of a [`BinaryReader`] and reports
//! where its pixel block begins.
//!
//! Mirrors `libptiff/src/io/backend/isis/isis_document.cpp`.

use crate::io::BinaryReader;
use crate::io::StorageModel;
use crate::{Error, Result};

use super::label::parse_label;

/// Upper bound of how much of the front we scan for the label's "End" marker.
/// The pixel block is addressed right after it, so we never need to read the
/// whole file for the label.
const MAX_LABEL_SCAN: u64 = 4 * 1024 * 1024; // 4 MiB

/// A scanned ISIS3 document plus the byte length of its label (the pixel
/// origin sits directly after the bare "End" marker).
#[derive(Debug)]
pub struct IsisDocument {
    /// The parsed flat per-image [`StorageModel`].
    pub image: StorageModel,
    /// Label text length in bytes (including the trailing `"End\n"`).
    pub label_bytes: u64,
}

/// Scans and parses an ISIS3 document from the start of `reader`.
///
/// # Errors
///
/// Returns [`crate::ErrorCode::InvalidArgument`] on malformed input (no
/// label / no End marker / unparsable label).
pub fn read_document(reader: &mut dyn BinaryReader) -> Result<IsisDocument> {
    let size = reader.size()?;
    let scan = size.min(MAX_LABEL_SCAN);

    reader.seek(0)?;
    let mut buffer = vec![0u8; scan as usize];
    let got = reader.read(&mut buffer)?;
    let text = std::str::from_utf8(&buffer[..got])
        .map_err(|_| Error::invalid_argument("isis: label is not valid UTF-8"))?;

    // The label ends with a bare "End\n" line. Because the only such line is
    // the final one (End_Object has a trailing underscore), the last
    // "\nEnd\n" marks the pixel origin.
    const END_MARKER: &str = "\nEnd\n";
    let marker_pos = text
        .rfind(END_MARKER)
        .ok_or_else(|| Error::invalid_argument("isis: no End marker closing the label"))?;
    let label_bytes = (marker_pos + END_MARKER.len()) as u64;

    let image = parse_label(&buffer[..label_bytes as usize])?;
    Ok(IsisDocument { image, label_bytes })
}

/// Absolute byte offset of this image's pixel block (== the label byte
/// length).
#[must_use]
pub fn pixel_origin(label_bytes: u64) -> u64 {
    label_bytes
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

    #[test]
    fn reads_label_then_reports_pixel_origin() {
        let label = super::super::label::write_label(&image_model()).expect("write_label");
        let label_len = label.len() as u64;
        let mut bytes = label;
        bytes.extend_from_slice(&[0u8; 32 * 32]);

        let mut reader = MemoryBinaryReader::from_vec(bytes);
        let doc = read_document(&mut reader).expect("read_document");
        assert_eq!(doc.label_bytes, label_len);
        assert_eq!(pixel_origin(doc.label_bytes), label_len);
        assert_eq!(doc.image.field("imageWidth").unwrap(), "32");
    }

    #[test]
    fn missing_end_marker_is_invalid_argument() {
        let mut reader = MemoryBinaryReader::from_vec(b"not a label".to_vec());
        let err = read_document(&mut reader).expect_err("must fail");
        assert_eq!(err.code(), ErrorCode::InvalidArgument);
    }
}
