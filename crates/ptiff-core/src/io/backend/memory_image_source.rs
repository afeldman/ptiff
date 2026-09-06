//! [`ImageSource`] over one image's pixel region of a Memory ("PMEM") document.
//!
//! Mirrors `libptiff/src/io/backend/memory/memory_image_source.cpp`. Reads each
//! tile's raw, uncompressed bytes directly from a [`BinaryReader`] on demand,
//! at the linear offset computed from the image's [`crate::tile::TileLayout`] — no upfront
//! full-image read.

use crate::id::TileId;
use crate::io::{BinaryReader, ImageSource};
use crate::tile::{Tile, TileIndex};
use crate::{Error, Result};

use super::memory_layout::MemoryImageInfo;

/// Reads one tile at a time from a memory document's pixel region.
///
/// The `Tile` returned by [`read_tile`](MemoryImageSource::read_tile) borrows
/// this source's reusable single buffer, so it is valid only until the next
/// `read_tile` call or until this source is dropped — the same invalidation
/// contract as a single-buffer iterator.
pub struct MemoryImageSource<'a> {
    reader: &'a mut dyn BinaryReader,
    info: MemoryImageInfo,
    image_pixel_start: u64,
    buffer: Vec<u8>,
}

impl<'a> MemoryImageSource<'a> {
    /// Constructs the source over `reader` and the image's pixel region.
    pub fn new(
        reader: &'a mut dyn BinaryReader,
        info: MemoryImageInfo,
        image_pixel_start: u64,
    ) -> Self {
        Self {
            reader,
            info,
            image_pixel_start,
            buffer: Vec::new(),
        }
    }
}

impl ImageSource for MemoryImageSource<'_> {
    fn layout(&self) -> &crate::tile::TileLayout {
        &self.info.layout
    }

    fn read_tile(&mut self, index: TileIndex) -> Result<Tile<'_>> {
        let region = self.info.layout.region_for(index)?;

        let columns = self.info.layout.columns(index.level);
        let linear_index = u64::from(index.row) * u64::from(columns) + u64::from(index.column);

        let tile_offset = linear_index
            .checked_mul(self.info.tile_bytes)
            .ok_or_else(|| {
                Error::invalid_argument(
                    "MemoryImageSource::readTile: absolute tile offset overflows",
                )
            })?;
        let absolute_offset = self
            .image_pixel_start
            .checked_add(tile_offset)
            .ok_or_else(|| {
                Error::invalid_argument(
                    "MemoryImageSource::readTile: absolute tile offset overflows",
                )
            })?;

        let file_size = self.reader.size()?;
        if absolute_offset > file_size || self.info.tile_bytes > file_size - absolute_offset {
            return Err(Error::invalid_argument(
                "MemoryImageSource::readTile: tile byte range exceeds reader size",
            ));
        }

        let tile_bytes = self.info.tile_bytes as usize;
        self.buffer.resize(tile_bytes, 0);
        self.reader.seek(absolute_offset)?;
        let n = self.reader.read(&mut self.buffer)?;
        if n != tile_bytes {
            return Err(Error::invalid_argument(
                "MemoryImageSource::readTile: truncated tile data",
            ));
        }

        Ok(Tile::new(
            TileId::new(linear_index),
            index,
            region,
            &self.buffer,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::MemoryBinaryReader;
    use crate::io::StorageModel;
    use crate::ErrorCode;

    fn info() -> MemoryImageInfo {
        let mut m = StorageModel::new();
        m.set_field("imageWidth", "64");
        m.set_field("imageHeight", "32");
        m.set_field("tileWidth", "16");
        m.set_field("tileHeight", "16");
        m.set_field("samplesPerPixel", "1");
        m.set_field("pixelType", "UInt8");
        super::super::memory_layout::image_info_from_model(&m).unwrap()
    }

    fn read_sibling(data: Vec<u8>) -> MemoryBinaryReader {
        MemoryBinaryReader::from_vec(data)
    }

    #[test]
    fn reads_tile_zero_offset() {
        let info = info();
        // Pixel region starts at offset 12; document is just the pixel region.
        let mut bytes = vec![0u8; 12 + info.image_pixel_bytes as usize];
        // Write a distinguishable pattern into tile (0,0): fill bytes [12..12+256).
        for (i, b) in bytes[12..12 + 256].iter_mut().enumerate() {
            *b = (i % 251) as u8;
        }

        let mut reader = read_sibling(bytes);
        let mut source = MemoryImageSource::new(&mut reader, info, 12);
        let tile = source
            .read_tile(TileIndex::new(0, 0, 0))
            .expect("read tile");
        assert_eq!(tile.region().extent.width, 16);
        assert_eq!(tile.data().len(), 256);
        assert_eq!(tile.data()[0], 0);
        assert_eq!(tile.data()[255], 255 % 251);
    }

    #[test]
    fn reads_tile_at_linear_offset() {
        let info = info();
        let start = 12usize;
        let mut bytes = vec![0u8; start + info.image_pixel_bytes as usize];
        // Tile (1,0) starts at pixel offset 256 from docPrefix.
        let tile_off = start + 256;
        bytes[tile_off] = 0xAB;
        bytes[tile_off + 1] = 0xCD;

        let mut reader = read_sibling(bytes);
        let mut source = MemoryImageSource::new(&mut reader, info, start as u64);
        let tile = source.read_tile(TileIndex::new(1, 0, 0)).expect("tile 1,0");
        assert_eq!(tile.data()[0], 0xAB);
        assert_eq!(tile.data()[1], 0xCD);
        assert_eq!(tile.id().value(), 1); // linear index (row0*4 + col1)
    }

    #[test]
    fn out_of_range_tile_is_rejected() {
        let info = info();
        let mut reader = read_sibling(vec![0u8; 4096]);
        let mut source = MemoryImageSource::new(&mut reader, info, 12);
        let err = source.read_tile(TileIndex::new(99, 0, 0)).unwrap_err();
        assert_eq!(err.code(), ErrorCode::OutOfRange);
    }

    #[test]
    fn truncated_data_is_invalid_argument() {
        let info = info();
        // Only enough bytes for ~half a tile at offset 12.
        let mut reader = read_sibling(vec![0u8; 12 + 128]);
        let mut source = MemoryImageSource::new(&mut reader, info, 12);
        let err = source.read_tile(TileIndex::new(0, 0, 0)).unwrap_err();
        assert_eq!(err.code(), ErrorCode::InvalidArgument);
        assert_eq!(
            err.message(),
            "MemoryImageSource::readTile: tile byte range exceeds reader size"
        );
    }
}
