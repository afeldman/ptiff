//! Pixel-data-level read for the Zarr backend.
//!
//! Mirrors `ptiff::io::backend::zarr::ZarrImageSource` (see
//! `libptiff/src/io/backend/zarr/zarr_image_source.cpp`). Chunks are addressed
//! linearly (row-major, same order as the memory backend's tiles); each
//! `read_tile` seeks to that chunk's fixed slot, reads its 4-byte length
//! prefix + payload, decompresses it into `chunk_bytes` and returns a tile
//! borrowing this source's reusable buffer.

use crate::id::TileId;
use crate::io::BinaryReader;
use crate::tile::{Tile, TileIndex, TileLayout};
use crate::{Error, Result};

use super::codec::decompress;
use super::document::ZarrLayout;

/// A chunk-addressable reader over one Zarr document.
pub struct ZarrImageSource<'a> {
    /// The borrowed byte transport.
    reader: &'a mut dyn BinaryReader,
    /// The document header + layout.
    layout: ZarrLayout,
    /// Absolute byte offset of the first chunk slot.
    pixel_region: u64,
    /// Reusable per-tile decompressed buffer (valid until the next `read_tile`).
    buffer: Vec<u8>,
}

impl<'a> ZarrImageSource<'a> {
    /// Builds a source over `reader` for the document described by `layout`,
    /// whose chunk block begins at `pixel_region`.
    pub fn new(reader: &'a mut dyn BinaryReader, layout: ZarrLayout, pixel_region: u64) -> Self {
        Self {
            reader,
            layout,
            pixel_region,
            buffer: Vec::new(),
        }
    }

    /// Absolute byte offset of the chunk slot for `index`, given the layout's
    /// column count at that level. Returns an error on arithmetic overflow.
    fn chunk_offset(&self, columns: u32, index: TileIndex) -> Result<u64> {
        let linear = u64::from(index.row) * u64::from(columns) + u64::from(index.column);
        let slot = u64::from(self.layout.slot_size);
        let chunk_offset = self
            .pixel_region
            .checked_add(
                linear
                    .checked_mul(slot)
                    .ok_or_else(|| Error::invalid_argument("zarr: chunk offset overflows"))?,
            )
            .ok_or_else(|| Error::invalid_argument("zarr: chunk offset overflows"))?;
        Ok(chunk_offset)
    }
}

impl crate::io::ImageSource for ZarrImageSource<'_> {
    fn layout(&self) -> &TileLayout {
        &self.layout.layout
    }

    fn read_tile(&mut self, index: TileIndex) -> Result<Tile<'_>> {
        let region = self.layout.layout.region_for(index)?;
        if self.layout.slot_size == 0 || self.layout.chunk_bytes == 0 {
            return Err(Error::invalid_argument("zarr: zero chunk geometry"));
        }
        let columns = self.layout.layout.columns(index.level);
        let chunk_offset = self.chunk_offset(columns, index)?;

        // Bound-check the slot against the reader size.
        let file_size = self.reader.size()?;
        let slot_size = u64::from(self.layout.slot_size);
        if chunk_offset > file_size || slot_size > file_size - chunk_offset {
            return Err(Error::invalid_argument(
                "zarr: chunk slot exceeds reader size",
            ));
        }

        // 4-byte length prefix.
        let mut len_bytes = [0u8; 4];
        self.reader.seek(chunk_offset)?;
        let n = self.reader.read(&mut len_bytes)?;
        if n != len_bytes.len() {
            return Err(Error::invalid_argument("zarr: truncated chunk length"));
        }
        let payload_len = u32::from_le_bytes(len_bytes);
        if payload_len == 0 || u64::from(payload_len) > u64::from(self.layout.slot_size) - 4 {
            return Err(Error::invalid_argument(
                "zarr: invalid chunk payload length",
            ));
        }

        let mut payload = vec![0u8; payload_len as usize];
        let read_payload = self.reader.read(&mut payload)?;
        if read_payload != payload.len() {
            return Err(Error::invalid_argument("zarr: truncated chunk payload"));
        }

        let pixels = decompress(
            self.layout.compressor,
            &payload,
            self.layout.chunk_bytes as usize,
        )?;
        self.buffer = pixels;
        Ok(Tile::new(
            TileId::new(u64::from(index.row) * u64::from(columns) + u64::from(index.column)),
            index,
            region,
            &self.buffer,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::super::document::HEADER_SIZE;
    use super::*;
    use crate::io::image_source::ImageSource as _;
    use crate::io::MemoryBinaryReader;
    use crate::ErrorCode;

    fn layout() -> ZarrLayout {
        // 32x32 image, 16x16 tile, 1 band, UInt8 => chunk_bytes = 256, slot_size >= 4+256.
        let mut m = crate::io::StorageModel::new();
        m.set_field("imageWidth", "32");
        m.set_field("imageHeight", "32");
        m.set_field("tileWidth", "16");
        m.set_field("tileHeight", "16");
        m.set_field("samplesPerPixel", "1");
        m.set_field("pixelType", "UInt8");
        super::super::document::build_layout(&m).expect("build_layout")
    }

    /// Builds a raw-chunk document with two fixed slots, the second holding an
    /// uncompressed payload filled with `marker`.
    fn doc_with_tile(marker: u8, offset: u64) -> Vec<u8> {
        let mut doc = layout();
        doc.compressor = super::super::document::Compressor::None;
        let prefix = HEADER_SIZE + doc.header.len() as u64; // pixel_region
        let slot = u64::from(doc.slot_size);
        let mut bytes = vec![0u8; prefix as usize + (slot * 2) as usize];
        // Second slot (linear index 1): 4-byte length prefix + a full payload of `marker`.
        let start = (prefix + offset * slot) as usize;
        bytes[start..start + 4].copy_from_slice(&(doc.chunk_bytes).to_le_bytes());
        for b in bytes[start + 4..start + 4 + doc.chunk_bytes as usize].iter_mut() {
            *b = marker;
        }
        bytes
    }

    #[test]
    fn reads_tile_from_its_slot() {
        let doc = layout();
        let prefix = HEADER_SIZE + doc.header.len() as u64;
        let bytes = doc_with_tile(0xAB, 1);
        let mut reader = MemoryBinaryReader::from_vec(bytes);
        let mut source = ZarrImageSource::new(&mut reader, doc, prefix);
        let tile = source.read_tile(TileIndex::new(1, 0, 0)).expect("read");
        assert_eq!(tile.data().len(), 256);
        assert_eq!(tile.data()[0], 0xAB);
        assert_eq!(tile.id().value(), 1);
    }

    #[test]
    fn out_of_range_tile_is_rejected() {
        let doc = layout();
        let prefix = HEADER_SIZE + doc.header.len() as u64;
        let bytes = doc_with_tile(1, 0);
        let mut reader = MemoryBinaryReader::from_vec(bytes);
        let mut source = ZarrImageSource::new(&mut reader, doc, prefix);
        let err = source.read_tile(TileIndex::new(99, 0, 0)).unwrap_err();
        assert_eq!(err.code(), ErrorCode::OutOfRange);
    }
}
