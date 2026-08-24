//! Byte-level write cursor over an underlying output.
//!
//! Mirrors `ptiff::io::BinaryWriter` (see
//! `libptiff/include/ptiff/io/binary_writer.hpp`).

use crate::{Error, Result};

/// A byte-level write cursor over an underlying output.
///
/// The write-side mirror of [`crate::io::BinaryReader`]. Backends write files,
/// memory buffers or remote sinks through this abstraction.
///
/// **Thread-safety:** implementations are expected to be thread-safe for
/// independent instances; mutations of one instance must be externally
/// synchronized by the caller.
pub trait BinaryWriter {
    /// Writes `source.len()` bytes starting at the current cursor.
    ///
    /// Returns the number of bytes actually written.
    fn write(&mut self, source: &[u8]) -> Result<usize>;

    /// Moves the write cursor to an absolute byte `offset`.
    ///
    /// # Errors
    ///
    /// Returns [`crate::ErrorCode::OutOfRange`] if `offset` is beyond the output.
    fn seek(&mut self, offset: u64) -> Result<()>;

    /// Reports the current absolute byte offset of the write cursor.
    fn position(&self) -> Result<u64>;

    /// Pushes any buffered output to the underlying transport.
    ///
    /// Returns `Ok(())` once pending buffered bytes are committed to the
    /// transport. A no-op for unbuffered transports; always safe to call before
    /// releasing the writer.
    fn flush(&mut self) -> Result<()>;
}

/// A convenience helper: write all of `buf` or fail.
///
/// Used by backend writers once ported; currently exercised only by unit tests.
#[allow(dead_code)]
pub(crate) fn write_all_or(writer: &mut impl BinaryWriter, buf: &[u8], what: &str) -> Result<()> {
    let n = writer.write(buf)?;
    if n != buf.len() {
        return Err(Error::unknown(format!(
            "short write while writing {what}: wanted {} bytes, wrote {n}",
            buf.len()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct VecWriter {
        data: Vec<u8>,
        pos: usize,
    }
    impl BinaryWriter for VecWriter {
        fn write(&mut self, source: &[u8]) -> Result<usize> {
            self.data.extend_from_slice(source);
            self.pos = self.data.len();
            Ok(source.len())
        }
        fn seek(&mut self, offset: u64) -> Result<()> {
            if offset as usize > self.data.len() {
                return Err(Error::out_of_range("seek beyond output"));
            }
            self.pos = offset as usize;
            Ok(())
        }
        fn position(&self) -> Result<u64> {
            Ok(self.pos as u64)
        }
        fn flush(&mut self) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn writer_appends_and_remembers_position() {
        let mut w = VecWriter {
            data: Vec::new(),
            pos: 0,
        };
        assert_eq!(w.write(b"ab").unwrap(), 2);
        assert_eq!(w.write(b"cd").unwrap(), 2);
        assert_eq!(w.position().unwrap(), 4);
        assert_eq!(w.data, b"abcd");
    }

    #[test]
    fn write_all_or_ok() {
        let mut w = VecWriter {
            data: Vec::new(),
            pos: 0,
        };
        assert!(write_all_or(&mut w, b"hello", "test").is_ok());
        assert_eq!(w.data, b"hello");
    }

    #[test]
    fn flush_is_safe() {
        let mut w = VecWriter {
            data: Vec::new(),
            pos: 0,
        };
        assert!(w.flush().is_ok());
    }
}
