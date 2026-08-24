//! Pixel-data-level write for the Zarr backend.
//!
//! Mirrors `ptiff::io::backend::zarr::ZarrImageSink` (see
//! `libptiff/src/io/backend/zarr/zarr_image_sink.cpp`). Each `write_tile`
//! seeks to the chunk's fixed slot, writes a 4-byte length prefix + the
//! (possibly compressed) payload, then pads the slot to its fixed size so
//! every later slot's offset is a valid seek target. Tiles may be written in
//! any order; the byte count must match the layout-derived `chunk_bytes`.

use crate::io::{BinaryWriter, ImageSink};
use crate::tile::Tile;
use crate::{Error, Result};

use super::codec::compress;
use super::document::ZarrLayout;

/// A chunk-addressable writer over one Zarr document.
pub struct ZarrImageSink<'a> {
    /// The borrowed byte transport.
    writer: &'a mut dyn BinaryWriter,
    /// The document header + layout.
    layout: ZarrLayout,
    /// Absolute byte offset of the first chunk slot.
    pixel_region: u64,
}

impl<'a> ZarrImageSink<'a> {
    /// Builds a sink over `writer` for the document described by `layout`,
    /// whose chunk block begins at `pixel_region`.
    pub fn new(writer: &'a mut dyn BinaryWriter, layout: ZarrLayout, pixel_region: u64) -> Self {
        Self {
            writer,
            layout,
            pixel_region,
        }
    }
}

impl ImageSink for ZarrImageSink<'_> {
    fn layout(&self) -> &crate::tile::TileLayout {
        &self.layout.layout
    }

    fn write_tile(&mut self, tile: &Tile<'_>) -> Result<()> {
        self.layout.layout.region_for(tile.index())?;
        if tile.data().len() as u64 != u64::from(self.layout.chunk_bytes) {
            return Err(Error::invalid_argument(
                "zarr: tile byte count does not match the chunk size",
            ));
        }
        if self.layout.slot_size < 4 {
            return Err(Error::invalid_argument("zarr: invalid slot size"));
        }
        let columns = self.layout.layout.columns(tile.index().level);
        let linear =
            u64::from(tile.index().row) * u64::from(columns) + u64::from(tile.index().column);
        let slot = u64::from(self.layout.slot_size);
        let chunk_offset = self
            .pixel_region
            .checked_add(
                linear
                    .checked_mul(slot)
                    .ok_or_else(|| Error::invalid_argument("zarr: chunk offset overflows"))?,
            )
            .ok_or_else(|| Error::invalid_argument("zarr: chunk offset overflows"))?;

        let compressed = compress(self.layout.compressor, tile.data())?;
        if 4 + compressed.len() > self.layout.slot_size as usize {
            return Err(Error::invalid_argument(
                "zarr: compressed chunk exceeds its slot",
            ));
        }

        // 4-byte length prefix (little-endian).
        let len = (compressed.len() as u32).to_le_bytes();
        self.writer.seek(chunk_offset)?;
        let n = self.writer.write(&len)?;
        if n != len.len() {
            return Err(Error::invalid_argument("zarr: short length write"));
        }
        let n = self.writer.write(&compressed)?;
        if n != compressed.len() {
            return Err(Error::invalid_argument("zarr: short payload write"));
        }

        // Pad the slot to its fixed size so every slot occupies contiguous
        // on-disk bytes (MemoryBinaryWriter refuses to seek beyond the extent).
        let padding = self.layout.slot_size as usize - 4 - compressed.len();
        if padding > 0 {
            let zeros = vec![0u8; padding];
            let n = self.writer.write(&zeros)?;
            if n != padding {
                return Err(Error::invalid_argument("zarr: short padding write"));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::document::HEADER_SIZE;
    use super::*;
    use crate::id::TileId;
    use crate::io::image_sink::ImageSink as _;
    use crate::io::{MemoryBinaryWriter, StorageModel};
    use crate::tile::{TileExtent, TileIndex, TileRegion};
    use crate::ErrorCode;

    fn layout() -> ZarrLayout {
        // 32x32 image, 16x16 tile, 1 band, UInt8 => chunk_bytes = 256.
        let mut m = StorageModel::new();
        m.set_field("imageWidth", "32");
        m.set_field("imageHeight", "32");
        m.set_field("tileWidth", "16");
        m.set_field("tileHeight", "16");
        m.set_field("samplesPerPixel", "1");
        m.set_field("pixelType", "UInt8");
        m.set_field("compression", "None");
        super::super::document::build_layout(&m).expect("build_layout")
    }

    fn tile(index: TileIndex, bytes: &[u8]) -> Tile<'_> {
        Tile::new(
            TileId::new(0),
            index,
            TileRegion::new(0, 0, TileExtent::new(16, 16)),
            bytes,
        )
    }

    fn pre_sized_writer(pixel_region: u64, slot_count: u64, slot_size: u64) -> MemoryBinaryWriter {
        let mut writer = MemoryBinaryWriter::new();
        use crate::io::BinaryWriter as _;
        let zeros = vec![0u8; (pixel_region + slot_count * slot_size) as usize];
        writer.seek(0).unwrap();
        writer.write(&zeros).unwrap();
        writer
    }

    #[test]
    fn writes_tile_into_its_slot() {
        let layout = layout();
        let prefix = HEADER_SIZE + layout.header.len() as u64;
        let slot_size = u64::from(layout.slot_size);
        let mut writer = pre_sized_writer(prefix, 2, slot_size);
        {
            let mut sink = ZarrImageSink::new(&mut writer, layout, prefix);
            let mut payload = vec![0u8; 256];
            payload[0] = 0x11;
            payload[255] = 0xFF;
            sink.write_tile(&tile(TileIndex::new(1, 0, 0), &payload))
                .expect("write");
        }
        let buf = writer.take_buffer();
        // Slot 1 starts at prefix + 1*slot_size (slots are >= 4 + chunkBytes = 260).
        let start = prefix as usize + slot_size as usize;
        // The payload begins 4 bytes after the slot start (after the length prefix)
        // for slot 1 (linear index 1).
        assert_eq!(buf[start + 4], 0x11);
        assert_eq!(buf[start + 4 + 255], 0xFF);
    }

    #[test]
    fn mismatched_tile_bytes_is_invalid_argument() {
        let layout = layout();
        let prefix = HEADER_SIZE + layout.header.len() as u64;
        let slot_size = u64::from(layout.slot_size);
        let mut writer = pre_sized_writer(prefix, 2, slot_size);
        let mut sink = ZarrImageSink::new(&mut writer, layout, prefix);
        let err = sink
            .write_tile(&tile(TileIndex::new(0, 0, 0), &[0u8; 10]))
            .unwrap_err();
        assert_eq!(err.code(), ErrorCode::InvalidArgument);
        assert_eq!(
            err.message(),
            "zarr: tile byte count does not match the chunk size"
        );
    }

    #[test]
    fn out_of_range_tile_is_rejected() {
        let layout = layout();
        let prefix = HEADER_SIZE + layout.header.len() as u64;
        let slot_size = u64::from(layout.slot_size);
        let mut writer = pre_sized_writer(prefix, 2, slot_size);
        let mut sink = ZarrImageSink::new(&mut writer, layout, prefix);
        let err = sink
            .write_tile(&tile(TileIndex::new(99, 0, 0), &[0u8; 256]))
            .unwrap_err();
        assert_eq!(err.code(), ErrorCode::OutOfRange);
    }
}
