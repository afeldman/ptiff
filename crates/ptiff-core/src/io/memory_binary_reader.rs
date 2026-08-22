//! Concrete [`BinaryReader`] over an in-memory, shared byte buffer.
//!
//! Mirrors `ptiff::io::MemoryBinaryReader` (see
//! `libptiff/include/ptiff/io/memory_binary_reader.hpp` and
//! `libptiff/src/io/memory_binary_reader.cpp`).

use std::sync::Arc;

use crate::io::BinaryReader;
use crate::{Error, Result};

/// A concrete [`BinaryReader`] over an in-memory byte buffer.
///
/// Reads from a fixed, read-only byte buffer (typically the buffer a
/// [`MemoryBinaryWriter`] produced, shared via [`Arc<[u8]>`]) using the
/// standard [`BinaryReader`] cursor API (`read`/`seek`/`position`/`size`). It
/// keeps the buffer alive for as long as the reader lives, so no external
/// lifetime management is needed.
///
/// Not thread-safe, under the same contract as its [`BinaryReader`] base: it
/// holds a mutable read cursor.
///
/// [`MemoryBinaryWriter`]: crate::io::MemoryBinaryWriter
pub struct MemoryBinaryReader {
    data: Arc<[u8]>,
    pos: u64,
}

impl MemoryBinaryReader {
    /// Constructs a reader over `data`, sharing ownership of the buffer.
    pub fn new(data: Arc<[u8]>) -> Self {
        Self { data, pos: 0 }
    }

    /// Constructs a reader that owns a copy of `bytes`.
    pub fn from_vec(bytes: Vec<u8>) -> Self {
        Self::new(bytes.into())
    }

    /// Constructs a reader over a borrowed slice, copying it in.
    pub fn from_slice(bytes: &[u8]) -> Self {
        Self::new(bytes.into())
    }

    /// Returns the total size of the underlying buffer, if it were fully read.
    #[must_use]
    pub fn len(&self) -> u64 {
        self.data.len() as u64
    }

    /// Returns whether the buffer is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

impl BinaryReader for MemoryBinaryReader {
    fn read(&mut self, destination: &mut [u8]) -> Result<usize> {
        if destination.is_empty() {
            return Ok(0);
        }
        let available = self.data.len().saturating_sub(self.pos as usize);
        let to_copy = destination.len().min(available);
        if to_copy > 0 {
            let start = self.pos as usize;
            destination[..to_copy].copy_from_slice(&self.data[start..start + to_copy]);
            self.pos += to_copy as u64;
        }
        Ok(to_copy)
    }

    /// Moves the read cursor to an absolute byte `offset`.
    ///
    /// Returns [`crate::ErrorCode::InvalidArgument`] if `offset` is beyond the
    /// buffer extent, matching the concrete C++ `MemoryBinaryReader::seek`.
    fn seek(&mut self, offset: u64) -> Result<()> {
        if offset > self.data.len() as u64 {
            return Err(Error::invalid_argument(
                "MemoryBinaryReader::seek: offset beyond the buffer extent",
            ));
        }
        self.pos = offset;
        Ok(())
    }

    fn position(&self) -> Result<u64> {
        Ok(self.pos)
    }

    fn size(&self) -> Result<u64> {
        Ok(self.data.len() as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_bytes_from_buffer() {
        let data: Arc<[u8]> = Arc::from(&b"abcdef"[..]);
        let mut r = MemoryBinaryReader::new(data);
        assert_eq!(r.size().unwrap(), 6);
        let mut buf = [0u8; 4];
        assert_eq!(r.read(&mut buf).unwrap(), 4);
        assert_eq!(&buf, b"abcd");
        assert_eq!(r.position().unwrap(), 4);
    }

    #[test]
    fn read_at_end_returns_zero() {
        let mut r = MemoryBinaryReader::from_vec(b"abc".to_vec());
        let mut full = [0u8; 3];
        assert_eq!(r.read(&mut full).unwrap(), 3);
        let mut buf = [0u8; 1];
        assert_eq!(r.read(&mut buf).unwrap(), 0);
    }

    #[test]
    fn seek_and_reposition() {
        let mut r = MemoryBinaryReader::from_vec(b"abcdef".to_vec());
        r.seek(3).unwrap();
        let mut buf = [0u8; 2];
        assert_eq!(r.read(&mut buf).unwrap(), 2);
        assert_eq!(&buf, b"de");
        // Re-seek within bounds works.
        r.seek(0).unwrap();
        assert_eq!(r.position().unwrap(), 0);
    }

    #[test]
    fn seek_past_end_is_invalid_argument() {
        let mut r = MemoryBinaryReader::from_vec(b"abc".to_vec());
        let err = r.seek(99).unwrap_err();
        // Matches the concrete C++ MemoryBinaryReader::seek code.
        assert_eq!(err.code(), crate::ErrorCode::InvalidArgument);
        assert_eq!(
            err.message(),
            "MemoryBinaryReader::seek: offset beyond the buffer extent"
        );
    }

    #[test]
    fn constructors_cover_arc_and_vec_and_slice() {
        assert_eq!(MemoryBinaryReader::from_vec(vec![1, 2, 3]).len(), 3);
        assert_eq!(MemoryBinaryReader::from_slice(&[9]).len(), 1);
        assert!(MemoryBinaryReader::from_vec(Vec::new()).is_empty());
    }
}
