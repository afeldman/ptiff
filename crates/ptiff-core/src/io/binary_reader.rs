//! Byte-level read cursor over an underlying input.
//!
//! Mirrors `ptiff::io::BinaryReader` (see
//! `libptiff/include/ptiff/io/binary_reader.hpp`).

use crate::{Error, Result};

/// A byte-level read cursor over an underlying input.
///
/// Backends use a `BinaryReader` as the transport abstraction: all byte reads
/// go through it, so the same reader-driven logic (TIFF parser, tile decoder,
/// ...) works over files, memory and remote range requests alike.
///
/// **Thread-safety:** implementations are expected to be thread-safe for
/// independent instances; sharing one instance across concurrent reads is the
/// caller's responsibility (synchronization is not guaranteed by this trait).
pub trait BinaryReader {
    /// Reads up to `destination.len()` bytes into `destination`.
    ///
    /// Returns the number of bytes actually read, which may be less than
    /// `destination.len()` at end-of-input (and `0` at end-of-input).
    fn read(&mut self, destination: &mut [u8]) -> Result<usize>;

    /// Moves the read cursor to an absolute byte `offset`.
    ///
    /// # Errors
    ///
    /// Returns [`crate::ErrorCode::OutOfRange`] if `offset` is beyond the input.
    fn seek(&mut self, offset: u64) -> Result<()>;

    /// Reports the current absolute byte offset of the read cursor.
    fn position(&self) -> Result<u64>;

    /// Reports the total size in bytes of the underlying input, if known.
    ///
    /// Returns [`crate::ErrorCode::Unknown`] if the transport cannot determine
    /// it (e.g. an unbounded stream).
    fn size(&self) -> Result<u64>;
}

/// A convenience helper: read a slice of exactly `buf.len()` bytes or fail.
///
/// Used by backend parsers (TIFF, Zarr, ...) once ported; currently exercised
/// only by unit tests.
#[allow(dead_code)]
pub(crate) fn read_exact_or(
    reader: &mut impl BinaryReader,
    buf: &mut [u8],
    what: &str,
) -> Result<()> {
    let n = reader.read(buf)?;
    if n != buf.len() {
        return Err(Error::out_of_range(format!(
            "unexpected end of input while reading {what}: wanted {} bytes, got {n}",
            buf.len()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // A minimal in-memory reader used to validate the trait and helper.
    struct VecReader<'a> {
        data: &'a [u8],
        pos: usize,
    }
    impl BinaryReader for VecReader<'_> {
        fn read(&mut self, destination: &mut [u8]) -> Result<usize> {
            let n = destination.len().min(self.data.len() - self.pos);
            destination[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
            self.pos += n;
            Ok(n)
        }
        fn seek(&mut self, offset: u64) -> Result<()> {
            if offset as usize > self.data.len() {
                return Err(Error::out_of_range("seek beyond input"));
            }
            self.pos = offset as usize;
            Ok(())
        }
        fn position(&self) -> Result<u64> {
            Ok(self.pos as u64)
        }
        fn size(&self) -> Result<u64> {
            Ok(self.data.len() as u64)
        }
    }

    #[test]
    fn reader_wraps_around_input() {
        let data = b"abcdef";
        let mut r = VecReader { data, pos: 0 };
        let mut buf = [0u8; 4];
        assert_eq!(r.read(&mut buf).unwrap(), 4);
        assert_eq!(&buf, b"abcd");
        assert_eq!(r.position().unwrap(), 4);
        r.seek(0).unwrap();
        assert_eq!(r.position().unwrap(), 0);
        assert_eq!(r.size().unwrap(), 6);
    }

    #[test]
    fn seek_past_end_fails() {
        let data = b"abcd";
        let mut r = VecReader { data, pos: 0 };
        assert!(r.seek(99).is_err());
    }

    #[test]
    fn read_exact_or_ok_and_eof() {
        let data = b"abcd";
        let mut r = VecReader { data, pos: 0 };
        let mut buf = [0u8; 4];
        assert!(read_exact_or(&mut r, &mut buf, "test").is_ok());
        let mut eof = [0u8; 8];
        assert!(read_exact_or(&mut r, &mut eof, "test").is_err());
    }
}
