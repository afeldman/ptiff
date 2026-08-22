//! Concrete [`BinaryWriter`] over an in-memory byte buffer.
//!
//! Mirrors `ptiff::io::MemoryBinaryWriter` (see
//! `libptiff/include/ptiff/io/memory_binary_writer.hpp` and
//! `libptiff/src/io/memory_binary_writer.cpp`).

use crate::io::BinaryWriter;
use crate::{Error, Result};

/// A concrete [`BinaryWriter`] over an in-memory byte buffer.
///
/// Accumulates written bytes into an internally owned `Vec<u8>` and exposes
/// that buffer through [`buffer`](MemoryBinaryWriter::buffer) /
/// [`take_buffer`](MemoryBinaryWriter::take_buffer) so the result can be handed
/// back to a [`MemoryBinaryReader`] without touching the filesystem.
///
/// **Seek semantics:** `seek` may move the cursor to any offset **within the
/// current buffer extent** (`0..=size()` inclusive); seeking past the end is
/// rejected with [`crate::ErrorCode::InvalidArgument`]. `write` always lands at
/// the current cursor: writing at the cursor when it equals `size()` **appends**
/// and grows the buffer, while writing at a position already within the buffer
/// overwrites those bytes in place. This matches the in-memory transport's need
/// to grow a write stream while guarding against a wild seek followed by a
/// giant write.
///
/// Not thread-safe, under the same contract as its [`BinaryWriter`] base: it
/// holds a mutable write cursor.
///
/// [`MemoryBinaryReader`]: crate::io::MemoryBinaryReader
pub struct MemoryBinaryWriter {
    buffer: Vec<u8>,
    pos: u64,
}

impl MemoryBinaryWriter {
    /// Constructs an empty in-memory writer (empty buffer, cursor at 0).
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            pos: 0,
        }
    }

    /// Returns a view over the accumulated buffer.
    #[must_use]
    pub fn buffer(&self) -> &[u8] {
        &self.buffer
    }

    /// Moves the accumulated buffer out of the writer, leaving an empty buffer
    /// and cursor at 0.
    pub fn take_buffer(&mut self) -> Vec<u8> {
        let out = std::mem::take(&mut self.buffer);
        self.pos = 0;
        out
    }

    /// Returns the total size of the accumulated buffer in bytes.
    #[must_use]
    pub fn len(&self) -> u64 {
        self.buffer.len() as u64
    }

    /// Returns whether the buffer is currently empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }
}

impl Default for MemoryBinaryWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl BinaryWriter for MemoryBinaryWriter {
    fn write(&mut self, source: &[u8]) -> Result<usize> {
        let end = self.pos.checked_add(source.len() as u64).ok_or_else(|| {
            Error::invalid_argument("MemoryBinaryWriter::write: position overflow")
        })?;

        // Appending past the current extent grows the buffer, zero-filling any
        // gap (only reachable by an in-buffer overwrite that extends past the
        // old end).
        if end > self.buffer.len() as u64 {
            self.buffer.resize(end as usize, 0);
        }
        let start = self.pos as usize;
        self.buffer[start..end as usize].copy_from_slice(source);
        self.pos = end;
        Ok(source.len())
    }

    /// Moves the write cursor to an absolute byte `offset`.
    ///
    /// Returns [`crate::ErrorCode::InvalidArgument`] if `offset` is beyond the
    /// current buffer extent, matching the concrete C++ `MemoryBinaryWriter::seek`.
    fn seek(&mut self, offset: u64) -> Result<()> {
        if offset > self.buffer.len() as u64 {
            return Err(Error::invalid_argument(
                "MemoryBinaryWriter::seek: offset beyond the current buffer extent",
            ));
        }
        self.pos = offset;
        Ok(())
    }

    fn position(&self) -> Result<u64> {
        Ok(self.pos)
    }

    /// In-memory output needs no explicit flush; always succeeds.
    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_in_order() {
        let mut w = MemoryBinaryWriter::new();
        assert_eq!(w.write(b"ab").unwrap(), 2);
        assert_eq!(w.write(b"cd").unwrap(), 2);
        assert_eq!(w.buffer(), b"abcd");
        assert_eq!(w.position().unwrap(), 4);
        assert_eq!(w.len(), 4);
    }

    #[test]
    fn overwrite_in_place_at_cursor() {
        let mut w = MemoryBinaryWriter::new();
        w.write(b"abcdef").unwrap();
        w.seek(1).unwrap();
        w.write(b"XY").unwrap();
        assert_eq!(w.buffer(), b"aXYdef");
        assert_eq!(w.position().unwrap(), 3);
    }

    #[test]
    fn write_past_buffer_grows_and_zero_fills() {
        let mut w = MemoryBinaryWriter::new();
        w.write(b"ab").unwrap();
        // Cursor at 2; writing 3 bytes lands past the 2-byte extent.
        w.write(b"cde").unwrap();
        assert_eq!(w.buffer(), b"abcde");
    }

    #[test]
    fn seek_past_end_is_invalid_argument() {
        let mut w = MemoryBinaryWriter::new();
        w.write(b"abc").unwrap();
        let err = w.seek(99).unwrap_err();
        assert_eq!(err.code(), crate::ErrorCode::InvalidArgument);
        assert_eq!(
            err.message(),
            "MemoryBinaryWriter::seek: offset beyond the current buffer extent"
        );
    }

    #[test]
    fn flush_is_a_noop_success() {
        let mut w = MemoryBinaryWriter::new();
        w.flush().unwrap();
    }

    #[test]
    fn take_buffer_moves_out_and_resets_cursor() {
        let mut w = MemoryBinaryWriter::new();
        w.write(b"hello").unwrap();
        let out = w.take_buffer();
        assert_eq!(out, b"hello");
        assert_eq!(w.len(), 0);
        assert!(w.is_empty());
        assert_eq!(w.position().unwrap(), 0);
    }
}
