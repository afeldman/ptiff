//! TIFF/BigTIFF file-header parse and write.
//!
//! Mirrors `ptiff::io::backend::tiff::tiff_header.{hpp,cpp}` (read side) and
//! `tiff_header_writer.{hpp,cpp}` (write side).

use crate::io::BinaryReader;
use crate::io::BinaryWriter;
use crate::{Error, Result};

use super::endian::{read_u16, read_u32, read_u64, write_u16, write_u32, write_u64, Endian};

/// Byte size of a classic TIFF header.
pub const K_CLASSIC_TIFF_HEADER_SIZE: u64 = 8;
/// Byte size of a BigTIFF header.
pub const K_BIG_TIFF_HEADER_SIZE: u64 = 16;

const K_CLASSIC_MAGIC: u16 = 42;
const K_BIG_TIFF_MAGIC: u16 = 43;

/// Parsed TIFF/BigTIFF file header: byte order, container kind, and the absolute
/// offset of the first IFD.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TiffHeader {
    /// The byte order declared in the header.
    pub endian: Endian,
    /// Whether this is a BigTIFF container (magic 43) vs classic (magic 42).
    pub is_big_tiff: bool,
    /// Absolute file offset of the first IFD.
    pub first_ifd_offset: u64,
}

fn read_byte_order(mark: &[u8]) -> Result<Endian> {
    assert!(mark.len() >= 2, "byte-order mark requires 2 bytes");
    let first = mark[0];
    let second = mark[1];
    match (first, second) {
        (b'I', b'I') => Ok(Endian::Little),
        (b'M', b'M') => Ok(Endian::Big),
        _ => Err(Error::invalid_argument(
            "read_tiff_header: unrecognized byte-order mark",
        )),
    }
}

/// Reads and validates the 8-byte classic TIFF header or 16-byte BigTIFF header,
/// starting at `reader`'s current position.
///
/// # Errors
///
/// Returns [`crate::ErrorCode::InvalidArgument`] if the byte-order mark or magic
/// number is not recognized, the input is shorter than a header, or (BigTIFF
/// only) the offset-byte-size field is not 8.
pub fn read_tiff_header(reader: &mut impl BinaryReader) -> Result<TiffHeader> {
    let mut raw = [0u8; 16];
    let first_read = reader.read(&mut raw[..8])?;
    if first_read != 8 {
        return Err(Error::invalid_argument(
            "read_tiff_header: file shorter than a TIFF header",
        ));
    }

    let endian = read_byte_order(&raw[..2])?;

    let magic = read_u16(&raw[2..4], endian);
    if magic == K_CLASSIC_MAGIC {
        let offset = u64::from(read_u32(&raw[4..8], endian));
        return Ok(TiffHeader {
            endian,
            is_big_tiff: false,
            first_ifd_offset: offset,
        });
    }
    if magic != K_BIG_TIFF_MAGIC {
        return Err(Error::invalid_argument(
            "read_tiff_header: unrecognized magic number",
        ));
    }

    let offset_byte_size = read_u16(&raw[4..6], endian);
    let constant_zero = read_u16(&raw[6..8], endian);
    if offset_byte_size != 8 || constant_zero != 0 {
        return Err(Error::invalid_argument(
            "read_tiff_header: malformed BigTIFF header fields",
        ));
    }

    let second_read = reader.read(&mut raw[8..16])?;
    if second_read != 8 {
        return Err(Error::invalid_argument(
            "read_tiff_header: file shorter than a BigTIFF header",
        ));
    }

    let offset = read_u64(&raw[8..16], endian);
    Ok(TiffHeader {
        endian,
        is_big_tiff: true,
        first_ifd_offset: offset,
    })
}

fn write_and_check(writer: &mut impl BinaryWriter, header: &[u8]) -> Result<()> {
    let n = writer.write(header)?;
    if n != header.len() {
        return Err(Error::invalid_argument("write_tiff_header: short write"));
    }
    Ok(())
}

/// Writes the classic 8-byte or BigTIFF 16-byte little-endian TIFF header at
/// `writer`'s current position. Classic: "II", magic 42, `first_ifd_offset`
/// truncated to 4 bytes. BigTIFF: "II", magic 43, offsetByteSize=8, constant 0,
/// `first_ifd_offset` as 8 bytes. `is_big_tiff` selects the variant; defaults to
/// classic.
///
/// Mirrors `ptiff::io::backend::tiff::writeTiffHeader`.
pub fn write_tiff_header(
    writer: &mut impl BinaryWriter,
    first_ifd_offset: u64,
    is_big_tiff: bool,
) -> Result<()> {
    if !is_big_tiff {
        let mut header = [b'I', b'I', 0x2A, 0x00, 0x00, 0x00, 0x00, 0x00];
        write_u32(&mut header[4..8], first_ifd_offset as u32, Endian::Little);
        return write_and_check(writer, &header);
    }

    let mut header = [b'I', b'I', 0x2B, 0x00, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    write_u16(&mut header[4..6], 8, Endian::Little); // offsetByteSize
    write_u16(&mut header[6..8], 0, Endian::Little); // constant
    write_u64(&mut header[8..16], first_ifd_offset, Endian::Little);
    write_and_check(writer, &header)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::memory_binary_reader::MemoryBinaryReader;
    use crate::io::memory_binary_writer::MemoryBinaryWriter;
    use crate::ErrorCode;

    fn vec_reader(bytes: &[u8]) -> MemoryBinaryReader {
        MemoryBinaryReader::from_slice(bytes)
    }

    #[test]
    fn parse_classic_little_endian_header() {
        // "II", magic 42, offset 8 (little-endian).
        let mut r = vec_reader(&[b'I', b'I', 0x2A, 0x00, 0x08, 0x00, 0x00, 0x00]);
        let h = read_tiff_header(&mut r).unwrap();
        assert_eq!(h.endian, Endian::Little);
        assert!(!h.is_big_tiff);
        assert_eq!(h.first_ifd_offset, 8);
    }

    #[test]
    fn parse_classic_big_endian_header() {
        // "MM", magic 0x2A, offset 16 (big-endian).
        let mut r = vec_reader(&[b'M', b'M', 0x00, 0x2A, 0x00, 0x00, 0x00, 0x10]);
        let h = read_tiff_header(&mut r).unwrap();
        assert_eq!(h.endian, Endian::Big);
        assert!(!h.is_big_tiff);
        assert_eq!(h.first_ifd_offset, 16);
    }

    #[test]
    fn parse_big_tiff_little_endian_header() {
        let mut r = vec_reader(&[
            b'I', b'I', 0x2B, 0x00, 0x08, 0x00, 0x00, 0x00, // offsetByteSize=8, constant=0
            0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // offset 16
        ]);
        let h = read_tiff_header(&mut r).unwrap();
        assert_eq!(h.endian, Endian::Little);
        assert!(h.is_big_tiff);
        assert_eq!(h.first_ifd_offset, 16);
    }

    #[test]
    fn parse_big_tiff_big_endian_header() {
        let mut r = vec_reader(&[
            b'M', b'M', 0x00, 0x2B, 0x00, 0x08, 0x00, 0x00, // offsetByteSize=8, constant=0
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x20, // offset 32 (big-endian)
        ]);
        let h = read_tiff_header(&mut r).unwrap();
        assert_eq!(h.endian, Endian::Big);
        assert!(h.is_big_tiff);
        assert_eq!(h.first_ifd_offset, 32);
    }

    #[test]
    fn reject_unrecognized_byte_order_mark() {
        let mut r = vec_reader(&[b'X', b'X', 0x2A, 0x00, 0x08, 0x00, 0x00, 0x00]);
        let e = read_tiff_header(&mut r).unwrap_err();
        assert_eq!(e.code(), ErrorCode::InvalidArgument);
    }

    #[test]
    fn reject_unrecognized_magic_number() {
        let mut r = vec_reader(&[b'I', b'I', 0x99, 0x00, 0x08, 0x00, 0x00, 0x00]);
        let e = read_tiff_header(&mut r).unwrap_err();
        assert_eq!(e.code(), ErrorCode::InvalidArgument);
    }

    #[test]
    fn reject_truncated_file() {
        let mut r = vec_reader(&[b'I', b'I', 0x2A]);
        let e = read_tiff_header(&mut r).unwrap_err();
        assert_eq!(e.code(), ErrorCode::InvalidArgument);
    }

    #[test]
    fn reject_malformed_big_tiff_offset_byte_size() {
        let mut r = vec_reader(&[
            b'I', b'I', 0x2B, 0x00, 0x04, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ]);
        let e = read_tiff_header(&mut r).unwrap_err();
        assert_eq!(e.code(), ErrorCode::InvalidArgument);
    }

    #[test]
    fn write_classic_header() {
        assert_eq!(K_CLASSIC_TIFF_HEADER_SIZE, 8);
        let mut w = MemoryBinaryWriter::new();
        write_tiff_header(&mut w, 8, false).unwrap();
        assert_eq!(w.buffer(), &[b'I', b'I', 0x2A, 0x00, 8, 0, 0, 0]);
    }

    #[test]
    fn write_classic_header_arbitrary_offset() {
        let mut w = MemoryBinaryWriter::new();
        write_tiff_header(&mut w, 0x0000_01A2, false).unwrap();
        assert_eq!(
            w.buffer(),
            &[b'I', b'I', 0x2A, 0x00, 0xA2, 0x01, 0x00, 0x00]
        );
    }

    #[test]
    fn write_big_tiff_header() {
        assert_eq!(K_BIG_TIFF_HEADER_SIZE, 16);
        let mut w = MemoryBinaryWriter::new();
        write_tiff_header(&mut w, 16, true).unwrap();
        assert_eq!(
            w.buffer(),
            &[b'I', b'I', 0x2B, 0x00, 8, 0, 0, 0, 16, 0, 0, 0, 0, 0, 0, 0]
        );
    }

    #[test]
    fn write_big_tiff_header_arbitrary_offset() {
        let mut w = MemoryBinaryWriter::new();
        write_tiff_header(&mut w, 0x0000_0001_0000_01A2, true).unwrap();
        assert_eq!(
            w.buffer(),
            &[b'I', b'I', 0x2B, 0x00, 8, 0, 0, 0, 0xA2, 0x01, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00]
        );
    }
}
