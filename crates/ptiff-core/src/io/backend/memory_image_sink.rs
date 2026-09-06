//! [`ImageSink`] over one image's pixel region of a Memory ("PMEM") document.
//!
//! Mirrors `libptiff/src/io/backend/memory/memory_image_sink.cpp`. Writes each
//! tile's raw, uncompressed bytes at the linear offset its [`crate::tile::TileLayout`]
//! implies (seeking as needed), so tiles may be written in any order. The byte
//! count per tile must equal the layout-derived `tile_bytes`; edge tiles are
//! written padded to the full tile size, matching the system-wide contract.

use crate::io::{BinaryWriter, ImageSink};
use crate::tile::Tile;
use crate::{Error, Result};

use super::memory_layout::MemoryImageInfo;

/// Writes tiles into a memory document's pixel region.
pub struct MemoryImageSink<'a> {
    writer: &'a mut dyn BinaryWriter,
    info: MemoryImageInfo,
    image_pixel_start: u64,
}

impl<'a> MemoryImageSink<'a> {
    /// Constructs the sink over `writer` and the image's pixel region.
    pub fn new(
        writer: &'a mut dyn BinaryWriter,
        info: MemoryImageInfo,
        image_pixel_start: u64,
    ) -> Self {
        Self {
            writer,
            info,
            image_pixel_start,
        }
    }
}

impl ImageSink for MemoryImageSink<'_> {
    fn layout(&self) -> &crate::tile::TileLayout {
        &self.info.layout
    }

    fn write_tile(&mut self, tile: &Tile<'_>) -> Result<()> {
        let _region = self.info.layout.region_for(tile.index())?;

        let columns = self.info.layout.columns(tile.index().level);
        let linear_index =
            u64::from(tile.index().row) * u64::from(columns) + u64::from(tile.index().column);

        if tile.data().len() as u64 != self.info.tile_bytes {
            return Err(Error::invalid_argument(
                "MemoryImageSink::writeTile: tile byte count does not match the layout",
            ));
        }
        if self.info.tile_bytes == 0 {
            return Err(Error::invalid_argument(
                "MemoryImageSink::writeTile: zero tile size",
            ));
        }

        let tile_offset = linear_index
            .checked_mul(self.info.tile_bytes)
            .ok_or_else(|| {
                Error::invalid_argument(
                    "MemoryImageSink::writeTile: tile offset computation overflows",
                )
            })?;
        let absolute_offset = self
            .image_pixel_start
            .checked_add(tile_offset)
            .ok_or_else(|| {
                Error::invalid_argument(
                    "MemoryImageSink::writeTile: absolute tile offset overflows",
                )
            })?;

        self.writer.seek(absolute_offset)?;
        let n = self.writer.write(tile.data())?;
        if n != tile.data().len() {
            return Err(Error::invalid_argument(
                "MemoryImageSink::writeTile: short write",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::TileId;
    use crate::io::{MemoryBinaryWriter, StorageModel};
    use crate::tile::{TileExtent, TileIndex, TileRegion};
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

    fn tile(index: TileIndex, bytes: &[u8]) -> Tile<'_> {
        Tile::new(
            TileId::new(0),
            index,
            TileRegion::new(0, 0, TileExtent::new(16, 16)),
            bytes,
        )
    }

    #[test]
    fn writes_tile_at_layout_offset() {
        let info = info();
        let mut writer = MemoryBinaryWriter::new();
        // Pre-size the buffer to the full document extent so seeks land inside
        // the buffer (MemoryBinaryWriter rejects seeks past its extent).
        let total = 12u64 + info.image_pixel_bytes;
        {
            use crate::io::BinaryWriter as _;
            let zeros = vec![0u8; total as usize];
            writer.seek(0).unwrap();
            writer.write(&zeros).unwrap();
        }

        let mut sink = MemoryImageSink::new(&mut writer, info, 12);
        let mut payload = vec![0u8; 256];
        payload[0] = 0x11;
        payload[255] = 0xFF;
        sink.write_tile(&tile(TileIndex::new(2, 0, 0), &payload))
            .expect("write");

        let buf = writer.take_buffer();
        // Tile (2,0) -> linear index 2 -> offset 12 + 2*256 = 524
        assert_eq!(buf[12 + 2 * 256], 0x11);
        assert_eq!(buf[12 + 2 * 256 + 255], 0xFF);
    }

    #[test]
    fn mismatched_tile_bytes_is_invalid_argument() {
        let info = info();
        let mut writer = MemoryBinaryWriter::new();
        let mut sink = MemoryImageSink::new(&mut writer, info, 12);
        let small = vec![0u8; 10];
        let err = sink
            .write_tile(&tile(TileIndex::new(0, 0, 0), &small))
            .unwrap_err();
        assert_eq!(err.code(), ErrorCode::InvalidArgument);
        assert_eq!(
            err.message(),
            "MemoryImageSink::writeTile: tile byte count does not match the layout"
        );
    }

    #[test]
    fn out_of_range_tile_is_rejected() {
        let info = info();
        let mut writer = MemoryBinaryWriter::new();
        let mut sink = MemoryImageSink::new(&mut writer, info, 12);
        let payload = vec![0u8; 256];
        let err = sink
            .write_tile(&tile(TileIndex::new(99, 0, 0), &payload))
            .unwrap_err();
        assert_eq!(err.code(), ErrorCode::OutOfRange);
    }
}
