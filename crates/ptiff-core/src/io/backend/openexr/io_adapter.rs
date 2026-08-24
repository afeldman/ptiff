//! `std::io` adapters over the core's [`BinaryReader`]/[`BinaryWriter`].
//!
//! The pure-Rust `exr` crate reads and writes OpenEXR through the standard
//! [`std::io::Read`]/[`std::io::Write`]/[`std::io::Seek`] traits (plus, on the
//! read side, the `#[cfg(feature = "read_into_whole_image")]` "whole image"
//! convenience builders). To keep the OpenEXR backend on the core's unified
//! byte-transport abstraction (`BinaryReader`/`BinaryWriter`), we bridge the
//! two with thin adapters. This keeps the backend usable over files, memory and
//! remote range requests alike, exactly like the other format backends.

use std::io::{self, SeekFrom};

use crate::Error;

/// `std::io::Read + Seek` over a [`BinaryReader`].
///
/// [`BinaryReader`]: crate::io::BinaryReader
pub struct ReaderAdapter<'a> {
    inner: &'a mut dyn crate::io::BinaryReader,
}

impl<'a> ReaderAdapter<'a> {
    /// Wraps the borrowed `BinaryReader`.
    pub fn new(inner: &'a mut dyn crate::io::BinaryReader) -> Self {
        Self { inner }
    }
}

/// Maps a core [`Error`] into the `std::io::ErrorKind` the `exr` crate expects.
fn io_err(e: Error) -> io::Error {
    io::Error::other(e)
}

impl io::Read for ReaderAdapter<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf).map_err(io_err)
    }
}

impl io::Seek for ReaderAdapter<'_> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        // Compute the target absolute offset relative to the current position
        // (or the stream size / start) depending on the SeekFrom variant.
        let base = match pos {
            SeekFrom::Start(_) => 0,
            SeekFrom::End(_) => self.inner.size().map_err(io_err)?,
            SeekFrom::Current(_) => self.inner.position().map_err(io_err)?,
        };
        let target = match pos {
            SeekFrom::Start(o) => i128::from(o),
            SeekFrom::End(o) => i128::from(base) + i128::from(o),
            SeekFrom::Current(o) => i128::from(base) + i128::from(o),
        };
        if target < 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "seek before beginning of stream",
            ));
        }
        self.inner.seek(target as u64).map_err(io_err)?;
        self.inner.position().map_err(io_err)
    }
}

/// `std::io::Write + Seek` over a [`BinaryWriter`].
///
/// [`BinaryWriter`]: crate::io::BinaryWriter
pub struct WriterAdapter<'a> {
    inner: &'a mut dyn crate::io::BinaryWriter,
}

impl<'a> WriterAdapter<'a> {
    /// Wraps the borrowed `BinaryWriter`.
    pub fn new(inner: &'a mut dyn crate::io::BinaryWriter) -> Self {
        Self { inner }
    }
}

impl io::Write for WriterAdapter<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf).map_err(io_err)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush().map_err(io_err)
    }
}

impl io::Seek for WriterAdapter<'_> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        // Mirror ReaderAdapter: `BinaryWriter::seek` only supports seeking to
        // an absolute offset; `position()` reports the current cursor.
        let base = match pos {
            SeekFrom::Start(_) => 0,
            SeekFrom::End(_) => {
                // Writers do not report a `size()`; for End seeks the caller is
                // expected to have a deterministic layout (OpenEXR's chunk
                // tables are written back at known positions). Fall back to the
                // current position, which the writer's own layout uses.
                self.inner.position().map_err(io_err)?
            }
            SeekFrom::Current(_) => self.inner.position().map_err(io_err)?,
        };
        let target = match pos {
            SeekFrom::Start(o) => i128::from(o),
            SeekFrom::End(o) => i128::from(base) + i128::from(o),
            SeekFrom::Current(o) => i128::from(base) + i128::from(o),
        };
        if target < 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "seek before beginning of stream",
            ));
        }
        self.inner.seek(target as u64).map_err(io_err)?;
        self.inner.position().map_err(io_err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::{MemoryBinaryReader, MemoryBinaryWriter};
    use std::io::{Read, Seek, Write};

    #[test]
    fn reader_adapter_reads_and_seeks() {
        let mut reader = MemoryBinaryReader::from_vec(b"hello world".to_vec());
        let mut adapter = ReaderAdapter::new(&mut reader);

        let mut buf = [0u8; 5];
        assert_eq!(adapter.read(&mut buf).unwrap(), 5);
        assert_eq!(&buf, b"hello");

        adapter.seek(SeekFrom::Start(6)).unwrap();
        let mut rest = Vec::new();
        adapter.read_to_end(&mut rest).unwrap();
        assert_eq!(rest, b"world");
    }

    #[test]
    fn writer_adapter_writes_and_seeks() {
        let mut writer = MemoryBinaryWriter::new();
        {
            let mut adapter = WriterAdapter::new(&mut writer);
            adapter.write_all(b"abcdef").unwrap();
            adapter.seek(SeekFrom::Start(3)).unwrap();
            adapter.write_all(b"XY").unwrap();
        }
        assert_eq!(writer.take_buffer(), b"abcXYf");
    }
}
