//! TIFF/BigTIFF [`StorageBackend`].
//!
//! Mirrors `ptiff::io::backend::TiffBackend` (`tiff_backend.{hpp,cpp}`): reads
//! baseline TIFF/BigTIFF — including multi-image files via the IFD chain — and
//! writes classic TIFF / BigTIFF for one or more images. A single image is a
//! single IFD followed by its data; multiple images are an IFD chain followed
//! by each image's data region.
//!
//! Call [`StorageBackend::serialize_model_list`] (or
//! [`StorageBackend::serialize_model`] for one image) before a pixel-data
//! write: it writes the header/IFD chain, and [`Self::open_image_sink_at`]
//! re-derives the same layout to position per-image pixel-data writes.
//!
//! This module is only compiled when the `tiff-backend` feature is enabled.

use crate::io::backend::tiff::header::{
    write_tiff_header, K_BIG_TIFF_HEADER_SIZE, K_CLASSIC_TIFF_HEADER_SIZE,
};
use crate::io::backend::tiff::{
    checked_add_u64, interpret_tiff_ifd, plan_tiff_write, plan_tiff_write_multi, read_tiff_header,
    read_tiff_ifd, tiff_ifd_byte_size, to_storage_model, write_tiff_ifd, TiffDirectory,
    TiffImageSink, TiffImageSource, TiffWritePlan,
};
use crate::io::{
    BackendCapabilities, BinaryReader, BinaryWriter, ImageSink, ImageSource, StorageBackend,
};
use crate::{Error, Result, StorageModel};

/// The TIFF/BigTIFF storage backend.
///
/// Capabilities: tiled layouts, random access, cloud streaming; not
/// format-streamable in the small-stream sense (it opens the full directory
/// chain).
///
/// Registered with [`crate::io::BackendFactory`] under the name `"tiff"`.
pub struct TiffBackend;

/// A parsed IFD chain: one directory per image, in file order.
struct TiffDirectoryChain {
    directories: Vec<TiffDirectory>,
    #[allow(dead_code)]
    is_big_tiff: bool,
}

/// Walks the whole IFD chain from the header's first directory, interpreting
/// each into a [`TiffDirectory`] (one per image, in file order). Uses `reader`
/// starting from its current position for the header, then follows each IFD's
/// trailing next-IFD offset until 0.
///
/// The IFD offset is untrusted (RFC-0001 §13): each offset must point at a
/// structure that lives inside the file, and a cyclic or non-advancing chain is
/// rejected.
fn read_directory_chain(reader: &mut dyn BinaryReader) -> Result<TiffDirectoryChain> {
    let header = read_tiff_header(reader)?;
    let file_size = reader.size()?;

    let mut directories = Vec::new();
    let mut ifd_offset = header.first_ifd_offset;
    const K_MAX_DIRECTORIES: usize = 100_000;
    for _ in 0..K_MAX_DIRECTORIES {
        // Validate the minimum footprint (the entry-count field, 2 bytes
        // classic / 8 bytes BigTIFF) before parsing.
        let count_field_size: u64 = if header.is_big_tiff { 8 } else { 2 };
        let min_ifd_end = checked_add_u64(ifd_offset, count_field_size)?;
        if min_ifd_end > file_size {
            return Err(Error::invalid_argument(
                "readDirectoryChain: IFD offset outside the file bounds",
            ));
        }

        let ifd = read_tiff_ifd(reader, ifd_offset, header.endian, header.is_big_tiff)?;
        let mut directory = interpret_tiff_ifd(&ifd)?;
        directory.endian = header.endian;
        directories.push(directory);

        let next = ifd.next_ifd_offset();
        if next == 0 {
            break;
        }
        if next <= ifd_offset {
            return Err(Error::invalid_argument(
                "readDirectoryChain: IFD chain does not advance (cyclic or corrupt)",
            ));
        }
        ifd_offset = next;
    }

    if directories.is_empty() {
        return Err(Error::invalid_argument(
            "readDirectoryChain: no IFD in file",
        ));
    }

    Ok(TiffDirectoryChain {
        directories,
        is_big_tiff: header.is_big_tiff,
    })
}

impl TiffBackend {
    /// Opens a sink whose per-image pixel data is positioned by re-planning
    /// the full multi-image file. `image_index` is the position of the image
    /// within `models`.
    fn open_image_sink_at_impl<'a>(
        &self,
        writer: &'a mut dyn BinaryWriter,
        models: &[StorageModel],
        image_index: usize,
    ) -> Result<Box<dyn ImageSink + 'a>> {
        let plan = plan_tiff_write_multi(models)?;
        let image = plan.images.get(image_index).ok_or_else(|| {
            Error::out_of_range("TiffBackend::openImageSinkAt: image index out of range")
        })?;
        let data_offset = image
            .directory
            .tile_byte_ranges
            .first()
            .map(|r| r.offset)
            .ok_or_else(|| {
                Error::invalid_argument("TiffBackend::openImageSinkAt: image has no byte ranges")
            })?;
        writer.seek(data_offset)?;
        Ok(Box::new(TiffImageSink::new(
            writer,
            image.directory.clone(),
        )))
    }
}

impl StorageBackend for TiffBackend {
    fn name(&self) -> &'static str {
        "tiff"
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            supports_tiling: true,
            supports_streaming: false,
            supports_random_access: true,
            supports_cloud_streaming: true,
        }
    }

    fn open_image_source<'a>(
        &self,
        reader: &'a mut dyn BinaryReader,
    ) -> Result<Box<dyn ImageSource + 'a>> {
        let chain = read_directory_chain(reader)?;
        let directory = chain
            .directories
            .into_iter()
            .next()
            .ok_or_else(|| Error::invalid_argument("TiffBackend: no image in file"))?;
        Ok(Box::new(TiffImageSource::new(reader, directory)))
    }

    fn open_image_sink<'a>(
        &self,
        writer: &'a mut dyn BinaryWriter,
        model: &StorageModel,
    ) -> Result<Box<dyn ImageSink + 'a>> {
        let plan = plan_tiff_write(model)?;
        let data_offset = plan
            .directory
            .tile_byte_ranges
            .first()
            .map(|r| r.offset)
            .ok_or_else(|| {
                Error::invalid_argument("TiffBackend::openImageSink: image has no byte ranges")
            })?;
        writer.seek(data_offset)?;
        Ok(Box::new(TiffImageSink::new(writer, plan.directory)))
    }

    fn deserialize_model(&self, reader: &mut dyn BinaryReader) -> Result<StorageModel> {
        let chain = read_directory_chain(reader)?;
        let mut root = StorageModel::new();
        for directory in &chain.directories {
            root.add_child(to_storage_model(directory));
        }
        Ok(root)
    }

    fn serialize_model(&self, model: &StorageModel, writer: &mut dyn BinaryWriter) -> Result<()> {
        let plan = plan_tiff_write(model)?;
        serialize_single_plan(&plan, writer)
    }

    fn serialize_model_list(
        &self,
        models: &[StorageModel],
        writer: &mut dyn BinaryWriter,
    ) -> Result<()> {
        let plan = plan_tiff_write_multi(models)?;
        let header_size = if plan.is_big_tiff {
            K_BIG_TIFF_HEADER_SIZE
        } else {
            K_CLASSIC_TIFF_HEADER_SIZE
        };
        write_tiff_header(writer, header_size, plan.is_big_tiff)?;

        // Write the IFD chain contiguously after the header, each linked to the
        // next via its next-IFD offset; the last links to 0. Pixel data is
        // written later through per-image sinks, which seek to the rebased
        // absolute offsets the file plan already carries.
        let mut cursor = header_size;
        for (i, image) in plan.images.iter().enumerate() {
            let ifd_size = tiff_ifd_byte_size(&image.entries, plan.is_big_tiff);
            let next_ifd_offset = if i + 1 < plan.images.len() {
                cursor + ifd_size
            } else {
                0
            };
            write_tiff_ifd(
                writer,
                image.entries.clone(),
                plan.is_big_tiff,
                next_ifd_offset,
            )?;
            cursor += ifd_size;
        }
        Ok(())
    }

    fn open_image_sink_at<'a>(
        &self,
        writer: &'a mut dyn BinaryWriter,
        models: &[StorageModel],
        image_index: usize,
    ) -> Result<Box<dyn ImageSink + 'a>> {
        self.open_image_sink_at_impl(writer, models, image_index)
    }

    fn open_image_source_at<'a>(
        &self,
        reader: &'a mut dyn BinaryReader,
        image_index: usize,
    ) -> Result<Box<dyn ImageSource + 'a>> {
        let chain = read_directory_chain(reader)?;
        let directory = chain
            .directories
            .into_iter()
            .nth(image_index)
            .ok_or_else(|| {
                Error::not_found("TiffBackend::openImageSourceAt: image index out of range")
            })?;
        Ok(Box::new(TiffImageSource::new(reader, directory)))
    }
}

/// Writes one image's header + IFD (single-image path).
fn serialize_single_plan(plan: &TiffWritePlan, writer: &mut dyn BinaryWriter) -> Result<()> {
    let header_size = if plan.is_big_tiff {
        K_BIG_TIFF_HEADER_SIZE
    } else {
        K_CLASSIC_TIFF_HEADER_SIZE
    };
    write_tiff_header(writer, header_size, plan.is_big_tiff)?;
    write_tiff_ifd(writer, plan.entries.clone(), plan.is_big_tiff, 0)
}

impl Default for TiffBackend {
    fn default() -> Self {
        Self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::TileId;
    use crate::io::MemoryBinaryReader;
    use crate::io::MemoryBinaryWriter;
    use crate::tile::{Tile, TileExtent, TileIndex, TileRegion};

    fn strip_uint8_model(width: u32, height: u32) -> StorageModel {
        let mut m = StorageModel::new();
        m.set_field("imageWidth", width.to_string());
        m.set_field("imageHeight", height.to_string());
        m.set_field("samplesPerPixel", "1");
        m.set_field("pixelType", "UInt8");
        m
    }

    /// Writes one strip's payload through [`StorageBackend::open_image_sink`]
    /// (single-image path) after `serialize_model`.
    fn write_single_into(
        backend: &TiffBackend,
        writer: &mut dyn BinaryWriter,
        model: &StorageModel,
        payload: &[u8],
    ) {
        let mut sink = backend
            .open_image_sink(writer, model)
            .expect("openImageSink");
        let layout = sink.layout();
        let region = TileRegion::new(
            0,
            0,
            TileExtent::new(layout.tile_size.width, layout.tile_size.height),
        );
        let tile = Tile::new(TileId::new(0), TileIndex::new(0, 0, 0), region, payload);
        sink.write_tile(&tile).expect("writeTile");
        drop(sink);
    }

    /// Reads a single-image file and returns the decoded tile bytes.
    fn read_single_into(backend: &TiffBackend, bytes: &[u8]) -> Vec<u8> {
        let mut reader = MemoryBinaryReader::from_slice(bytes);
        let mut source = backend
            .open_image_source(&mut reader)
            .expect("openImageSource");
        let tile = source
            .read_tile(TileIndex::new(0, 0, 0))
            .expect("readTile")
            .data()
            .to_vec();
        tile
    }

    #[test]
    fn name_and_capabilities() {
        let backend = TiffBackend;
        assert_eq!(backend.name(), "tiff");
        let caps = backend.capabilities();
        assert!(caps.supports_tiling);
        assert!(caps.supports_random_access);
        assert!(caps.supports_cloud_streaming);
        assert!(!caps.supports_streaming);
    }

    #[test]
    fn single_image_uncompressed_round_trip() {
        let model = strip_uint8_model(16, 16);
        let payload: Vec<u8> = (0..(16 * 16)).map(|i| (i % 251) as u8).collect();

        let mut writer = MemoryBinaryWriter::new();
        TiffBackend
            .serialize_model(&model, &mut writer)
            .expect("serializeModel");
        write_single_into(&TiffBackend, &mut writer, &model, &payload);
        let bytes = writer.take_buffer();

        let decoded = read_single_into(&TiffBackend, &bytes);
        assert_eq!(decoded, payload);
    }

    #[test]
    fn deserialize_model_reads_single_image() {
        // Build a file via serialize + write, then interpret it back to a model.
        let model = strip_uint8_model(4, 4);
        let payload = vec![0u8; 16];
        let mut writer = MemoryBinaryWriter::new();
        TiffBackend
            .serialize_model(&model, &mut writer)
            .expect("serializeModel");
        write_single_into(&TiffBackend, &mut writer, &model, &payload);
        let bytes = writer.take_buffer();

        let mut reader = MemoryBinaryReader::from_slice(&bytes);
        let root = TiffBackend
            .deserialize_model(&mut reader)
            .expect("deserializeModel");
        assert_eq!(root.child_count(), 1);
        let image = &root.children()[0];
        assert_eq!(image.field("imageWidth").unwrap(), "4");
        assert_eq!(image.field("imageHeight").unwrap(), "4");
        assert_eq!(image.field("samplesPerPixel").unwrap(), "1");
        assert_eq!(image.field("pixelType").unwrap(), "UInt8");
    }

    #[test]
    fn multi_image_strips_round_trip() {
        let m0 = strip_uint8_model(8, 8);
        let m1 = strip_uint8_model(4, 4);
        let models = [m0.clone(), m1.clone()];
        let payload0: Vec<u8> = (0..(8 * 8)).map(|i| (i % 251) as u8).collect();
        let payload1: Vec<u8> = (0..(4 * 4)).map(|i| (255 - (i % 200)) as u8).collect();

        // Write header + IFD chain, then each image's pixel region.
        let mut writer = MemoryBinaryWriter::new();
        TiffBackend
            .serialize_model_list(&models, &mut writer)
            .expect("serializeModelList");
        for (i, payload) in [&payload0, &payload1].iter().enumerate() {
            let mut sink = TiffBackend
                .open_image_sink_at(&mut writer, &models, i)
                .expect("openImageSinkAt");
            let layout = sink.layout();
            let region = TileRegion::new(
                0,
                0,
                TileExtent::new(layout.tile_size.width, layout.tile_size.height),
            );
            let tile = Tile::new(TileId::new(0), TileIndex::new(0, 0, 0), region, payload);
            sink.write_tile(&tile).expect("writeTile");
            drop(sink);
        }
        let bytes = writer.take_buffer();

        // Read each image back via open_image_source_at.
        for i in 0..2 {
            let mut reader = MemoryBinaryReader::from_slice(&bytes);
            let mut source = TiffBackend
                .open_image_source_at(&mut reader, i)
                .expect("openImageSourceAt");
            let decoded = source
                .read_tile(TileIndex::new(0, 0, 0))
                .expect("readTile")
                .data()
                .to_vec();
            let expected = if i == 0 { &payload0 } else { &payload1 };
            assert_eq!(decoded, *expected);
        }

        // deserialize_model exposes one child per image.
        let mut reader = MemoryBinaryReader::from_slice(&bytes);
        let root = TiffBackend
            .deserialize_model(&mut reader)
            .expect("deserializeModel");
        assert_eq!(root.child_count(), 2);
    }

    #[test]
    fn single_image_packbits_round_trip() {
        // A compressible single strip (all identical bytes) exercises PackBits
        // encoding through the backend sink and decoding through the source.
        let mut model = strip_uint8_model(16, 16);
        model.set_field("compression", "PackBits");
        let payload = vec![9u8; 16 * 16];
        let mut writer = MemoryBinaryWriter::new();
        TiffBackend
            .serialize_model(&model, &mut writer)
            .expect("serializeModel");
        write_single_into(&TiffBackend, &mut writer, &model, &payload);
        let bytes = writer.take_buffer();

        let decoded = read_single_into(&TiffBackend, &bytes);
        assert_eq!(decoded, payload);
    }

    #[test]
    fn create_via_factory_instance_is_registered() {
        let backend = crate::io::BackendFactory::instance()
            .create("tiff")
            .expect("tiff backend registered");
        assert_eq!(backend.name(), "tiff");
    }
}
