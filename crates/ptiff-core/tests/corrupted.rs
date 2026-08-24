//! Corrupted-file tests (§11.2).
//!
//! Malformed TIFFs (bad byte order/magic, truncated data, bad IFD offsets,
//! over-wide tag counts, out-of-line values exceeding the file, non-advancing
//! IFD chains) must be rejected with a well-defined [`ErrorCode`] instead of a
//! panic, memory blow-up, or misinterpretation — mirroring the C++ oracle's
//! `ErrorCode::InvalidArgument/OutOfRange/NotFound` taxonomy.
//!
//! Rust is the reference implementation, so these assert the Rust reader's own
//! rejection behaviour (and its error-code mapping), which the C++ oracle must
//! match. Corrupting a valid file byte-by-byte is the vehicle.

use ptiff_core::io::backend::tiff::TiffBackend;
use ptiff_core::io::backend::tiff::{read_tiff_header, Endian};
use ptiff_core::io::{MemoryBinaryReader, StorageBackend};
use ptiff_core::{ErrorCode, Result};

/// Classic TIFF header bytes: little-endian, magic 42, first IFD at 8.
fn classic_header(ifd_offset: u32) -> Vec<u8> {
    let mut h = vec![b'I', b'I', 42, 0];
    h.extend_from_slice(&ifd_offset.to_le_bytes());
    h
}

/// A minimal but *valid* single-image 1x1 UInt8 file with a no-op 0-entry IFD
/// and no next IFD. Used to generate corrupted variants from a known-good seed.
fn valid_minimal_file() -> Vec<u8> {
    // Header: II 42, IFD at 8.
    // IFD at 8: 0 entries, next-IFD offset 0.
    let mut b = classic_header(8);
    b.extend_from_slice(&0u16.to_le_bytes()); // entry count = 0
    b.extend_from_slice(&0u32.to_le_bytes()); // next IFD = 0
    b
}

/// Classic 2-entry IFD where the second entry carries its value out-of-line
/// at a fixed offset. Enough structure to exercise value-resolution errors.
fn two_entry_ifd_offset(offset_of_ifd: u32) -> Vec<u8> {
    let mut b = classic_header(offset_of_ifd);
    // 2 entries, next IFD = 0.
    b.extend_from_slice(&2u16.to_le_bytes());
    // Entry 0: ImageWidth (256), SHORT, count 1, inline value 16.
    b.extend_from_slice(&256u16.to_le_bytes());
    b.extend_from_slice(&3u16.to_le_bytes()); // SHORT
    b.extend_from_slice(&1u32.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(&0u16.to_le_bytes());
    // Entry 1: a long ASCII value (tag 269, DocumentName, type ASCII 2) with a
    // large count that must point out-of-line; patched by the caller.
    b.extend_from_slice(&269u16.to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes()); // ASCII
    b.extend_from_slice(&0u32.to_le_bytes()); // count patched below
    b.extend_from_slice(&[0u8; 4]); // offset patched below
    b.extend_from_slice(&0u32.to_le_bytes()); // next IFD
    b
}

fn deserialize(bytes: &[u8]) -> Result<()> {
    let mut reader = MemoryBinaryReader::from_slice(bytes);
    TiffBackend.deserialize_model(&mut reader).map(|_| ())
}

#[test]
fn truncated_file_is_rejected() {
    // Shorter than the 8-byte header.
    assert_eq!(
        deserialize(&[b'I', b'I', 42]).unwrap_err().code(),
        ErrorCode::InvalidArgument
    );
}

#[test]
fn unrecognized_byte_order_is_rejected() {
    let mut b = vec![b'X', b'X', 42, 0];
    b.extend_from_slice(&8u32.to_le_bytes());
    assert_eq!(
        deserialize(&b).unwrap_err().code(),
        ErrorCode::InvalidArgument
    );
}

#[test]
fn unrecognized_magic_is_rejected() {
    let mut b = classic_header(8);
    b[2] = 99; // corrupt magic 42 -> 99
    assert_eq!(
        deserialize(&b).unwrap_err().code(),
        ErrorCode::InvalidArgument
    );
}

#[test]
fn big_tiff_wrong_offset_byte_size_is_rejected() {
    // Big header: II 43, offsetBySize=4 (must be 8), constant, 8-byte IFD off.
    let mut b = vec![b'I', b'I', 43, 0];
    b.extend_from_slice(&4u16.to_le_bytes()); // offset byte size (must be 8)
    b.extend_from_slice(&0u16.to_le_bytes()); // reserved 0
    b.extend_from_slice(&8u64.to_le_bytes()); // IFD offset
    assert_eq!(
        deserialize(&b).unwrap_err().code(),
        ErrorCode::InvalidArgument
    );
}

#[test]
fn ifd_offset_outside_file_is_rejected() {
    // Header points the first IFD far beyond EOF.
    let b = classic_header(u32::MAX);
    assert_eq!(
        deserialize(&b).unwrap_err().code(),
        ErrorCode::InvalidArgument
    );
}

#[test]
fn non_advancing_ifd_chain_is_rejected() {
    // IFD at 8 points its next-IFD back at 8 → cycle / no advance.
    let mut b = classic_header(8);
    b.extend_from_slice(&0u16.to_le_bytes()); // 0 entries
    b.extend_from_slice(&8u32.to_le_bytes()); // next-IFD = 8 (self-cycle)
    assert_eq!(
        deserialize(&b).unwrap_err().code(),
        ErrorCode::InvalidArgument
    );
}

#[test]
fn truncated_ifd_entry_count_is_rejected() {
    // Header points at 8, but file ends before the 2-byte count can be read.
    let mut b = classic_header(8);
    b.extend_from_slice(&[0u8]); // only 1 byte of the entry count
    assert_eq!(
        deserialize(&b).unwrap_err().code(),
        ErrorCode::InvalidArgument
    );
}

#[test]
fn over_wide_tag_count_is_rejected_via_out_of_line() {
    // A tag claiming an ASCII count that overflows into out-of-line storage
    // pointing beyond EOF must be rejected (InvalidArgument).
    let mut b = two_entry_ifd_offset(8);
    // Locate the DocumentName entry (entry 1, at offset 8 + 2 + 12 = 22):
    //   count  @ 22+4=26? entry layout (tag 2b, type 2b, count 4b, value 4b).
    // Let the reader parse; we just assert that a huge out-of-line count is an
    // error regardless of the exact field position.
    //
    // Simpler: set the whole second entry's count+offset fields to point out of
    // bounds via patched bytes below.
    // entry1 starts at 8 + 2 + 12 = 22.  count is at 22+4 = 26..30;
    // offset (ASCII inline area is 4 bytes in classic) is at 30..34.
    let count_bytes = u32::MAX.to_le_bytes();
    b[26..30].copy_from_slice(&count_bytes);
    // offset beyond file
    b[30..34].copy_from_slice(&u32::MAX.to_le_bytes());
    let err = deserialize(&b).unwrap_err();
    assert_eq!(err.code(), ErrorCode::InvalidArgument);
}

#[test]
fn ifd_missing_required_tags_is_rejected() {
    // A structurally parseable 0-entry IFD still fails image interpretation
    // because the required core tags (ImageWidth/ImageLength) are absent.
    let b = valid_minimal_file();
    assert_eq!(
        deserialize(&b).unwrap_err().code(),
        ErrorCode::InvalidArgument
    );
}

#[test]
fn header_reader_rejects_each_corruption_category() {
    // Direct header-level checks mirror the unit tests but through the public
    // re-export (`read_tiff_header`), tying the taxonomy to the crate surface.
    fn read(bytes: &[u8]) -> ptiff_core::Result<()> {
        let mut r = MemoryBinaryReader::from_slice(bytes);
        read_tiff_header(&mut r).map(|_| ())
    }
    assert!(read(&[b'I', b'I', 42, 0]).is_err()); // too short for header
    assert!(read(&[b'X', b'X', 42, 0, 8, 0, 0, 0]).is_err());
    assert!(read(&[b'I', b'I', 99, 0, 8, 0, 0, 0]).is_err());
    // A valid header parses: II, 42, 8-byte.
    let h = read_tiff_header(&mut MemoryBinaryReader::from_slice(&classic_header(8))).unwrap();
    assert_eq!(h.first_ifd_offset, 8);
    assert!(!h.is_big_tiff);
    assert_eq!(h.endian, Endian::Little);
}
