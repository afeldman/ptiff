//! P0-01 (WP-P0-01): BigTIFF offset correctness through the real public
//! serialization path.
//!
//! Regression coverage for the truncation defect: the TIFF writer stored
//! every offset/byte count in 32-bit `u32` values even for BigTIFF, so an
//! offset or byte count beyond `0xFFFF_FFFF` was silently truncated
//! (`as u32` in directory_writer.rs). These tests prove:
//!
//! - classic TIFF rejects anything it cannot represent (typed guard, never a
//!   silent truncation);
//! - BigTIFF keeps offsets and byte counts as genuine 64-bit LONG8 values
//!   through allocation, serialization and re-reading;
//! - pixel data round-trips through BigTIFF (including the compressed-tiled
//!   back-patch path that writes 8-byte IFD slots).
//!
//! The > 4 GiB boundary is exercised through the real file-layout machinery
//! without allocating gigabytes: offsets and byte counts are derived from
//! declared image/tile geometry, so a model whose second tile starts beyond
//! 4 GiB produces real file bytes whose IFD contains a > `u32::MAX` offset.

use ptiff_core::io::backend::tiff::{
    interpret_tiff_ifd, plan_tiff_write, read_tiff_header, read_tiff_ifd, TagId, TiffBackend,
};
use ptiff_core::io::{MemoryBinaryReader, MemoryBinaryWriter, StorageBackend, StorageModel};
use ptiff_core::tile::{Tile, TileExtent, TileIndex, TileRegion};
use ptiff_core::{ErrorCode, TileId};

/// 10^10 bytes: beyond the classic 32-bit range, trivial as metadata.
const HUGE_STRIP_BYTES: u64 = 10_000_000_000;

fn huge_strip_model(container: &str) -> StorageModel {
    let mut m = StorageModel::new();
    m.set_field("imageWidth", "100000");
    m.set_field("imageHeight", "100000");
    m.set_field("samplesPerPixel", "1");
    m.set_field("pixelType", "UInt8");
    m.set_field("container", container.to_string());
    m
}

/// Tiled model whose two Float64 tiles each hold 2^30 x 16 samples: every
/// tile's byte count is 2^37 (~137 GiB) and the second tile's file offset
/// exceeds `u32::MAX` — without allocating a single pixel.
fn huge_tile_model() -> StorageModel {
    let mut m = StorageModel::new();
    m.set_field("imageWidth", "1073741824"); // 2^30
    m.set_field("imageHeight", "32");
    m.set_field("samplesPerPixel", "1");
    m.set_field("pixelType", "Float64");
    m.set_field("tileWidth", "1073741824"); // 2^30 (multiple of 16)
    m.set_field("tileHeight", "16");
    m.set_field("compression", "None");
    m.set_field("container", "BigTiff");
    m
}

/// Serializes a model (header + IFD, no pixel payload) and returns the bytes.
fn serialize_model_bytes(model: &StorageModel) -> Vec<u8> {
    let mut writer = MemoryBinaryWriter::new();
    TiffBackend
        .serialize_model(model, &mut writer)
        .expect("serialize model");
    writer.take_buffer()
}

/// Re-parses the first IFD of a serialized file through the real TIFF reader.
fn parse_first_ifd(
    bytes: &[u8],
) -> (
    ptiff_core::io::backend::tiff::TiffHeader,
    ptiff_core::io::backend::tiff::TiffIfd,
) {
    let mut reader = MemoryBinaryReader::from_slice(bytes);
    let header = read_tiff_header(&mut reader).expect("read header");
    let ifd = read_tiff_ifd(
        &mut reader,
        header.first_ifd_offset,
        header.endian,
        header.is_big_tiff,
    )
    .expect("read IFD");
    (header, ifd)
}

/// Classic TIFF must fail with a typed error when a byte count needs more
/// than 32 bits — never silently truncate it.
#[test]
fn classic_tiff_rejects_strip_byte_count_beyond_4gib() {
    let model = huge_strip_model("Classic");
    let mut writer = MemoryBinaryWriter::new();
    let err = TiffBackend
        .serialize_model(&model, &mut writer)
        .expect_err("classic TIFF cannot carry a 10^10-byte strip");
    assert_eq!(err.code(), ErrorCode::InvalidArgument);
    assert!(
        err.message().contains("classic TIFF"),
        "error must name the container: {}",
        err.message()
    );
}

/// The same huge strip model serializes under BigTIFF with the full 64-bit
/// byte count on disk (LONG8), and reading the file back yields the count
/// unchanged. The old implementation truncated the count to `u32` here.
#[test]
fn bigtiff_strip_byte_count_beyond_4gib_survives_serialization_and_readback() {
    let bytes = serialize_model_bytes(&huge_strip_model("BigTiff"));

    let (header, ifd) = parse_first_ifd(&bytes);
    assert!(header.is_big_tiff, "container must be BigTIFF");
    assert_eq!(
        ifd.tag(TagId::StripByteCounts.as_u16()).unwrap().to_vec(),
        vec![HUGE_STRIP_BYTES],
        "StripByteCounts must round-trip as a 64-bit value"
    );
    // StripOffsets itself stays small (the data would start right after the
    // IFD), but must be readable.
    let offsets = ifd.tag(TagId::StripOffsets.as_u16()).unwrap().to_vec();
    assert_eq!(offsets.len(), 1);
    assert!(offsets[0] < 1024);

    // The interpreted directory keeps the full 64-bit count.
    let mut reader = MemoryBinaryReader::from_slice(&bytes);
    let header2 = read_tiff_header(&mut reader).unwrap();
    let ifd2 = read_tiff_ifd(
        &mut reader,
        header2.first_ifd_offset,
        header2.endian,
        header2.is_big_tiff,
    )
    .unwrap();
    let directory = interpret_tiff_ifd(&ifd2).expect("interpret directory");
    assert_eq!(directory.tile_byte_ranges[0].byte_count, HUGE_STRIP_BYTES);
}

/// Genuine > 4 GiB file offsets: the second of two Float64 tiles starts at
/// `first + 2^37` (~137 GiB). Serializing this BigTIFF file (header + IFD
/// only — no pixel bytes needed) must place the full 64-bit offset into the
/// real IFD bytes, and re-reading must return it unchanged. The old
/// implementation truncated `offset as u32` to zero here.
#[test]
fn bigtiff_tile_offsets_beyond_4gib_survive_serialization_and_readback() {
    let model = huge_tile_model();
    let plan = plan_tiff_write(&model).expect("BigTIFF huge-tile plan");
    assert!(plan.is_big_tiff);
    assert_eq!(plan.directory.offset_value_bytes, 8);

    let bytes = serialize_model_bytes(&model);

    let (header, ifd) = parse_first_ifd(&bytes);
    assert!(header.is_big_tiff);
    let offsets = ifd.tag(TagId::TileOffsets.as_u16()).unwrap().to_vec();
    assert_eq!(offsets.len(), 2);
    assert!(
        offsets[0] < 1024,
        "first tile starts near the file head, got {}",
        offsets[0]
    );
    assert_eq!(
        offsets[1],
        offsets[0] + (1u64 << 37),
        "second tile offset must survive as a genuine 64-bit value"
    );
    assert!(
        offsets[1] > u64::from(u32::MAX),
        "test must exercise an offset beyond 32 bits"
    );
    let counts = ifd.tag(TagId::TileByteCounts.as_u16()).unwrap().to_vec();
    assert_eq!(counts, vec![1u64 << 37, 1 << 37]);

    // The interpreted directory agrees.
    let mut reader = MemoryBinaryReader::from_slice(&bytes);
    let header2 = read_tiff_header(&mut reader).unwrap();
    let ifd2 = read_tiff_ifd(
        &mut reader,
        header2.first_ifd_offset,
        header2.endian,
        header2.is_big_tiff,
    )
    .unwrap();
    let directory = interpret_tiff_ifd(&ifd2).expect("interpret directory");
    assert_eq!(directory.tile_byte_ranges[1].offset, offsets[1]);
    assert_eq!(directory.tile_byte_ranges[1].byte_count, 1u64 << 37);
}

/// Pixel round-trip through a real BigTIFF strip file.
#[test]
fn bigtiff_strip_pixels_round_trip() {
    let (width, height) = (32u32, 16u32);
    let payload: Vec<u8> = (0..(width * height) as usize)
        .map(|i| (i % 251) as u8)
        .collect();

    let mut model = StorageModel::new();
    model.set_field("imageWidth", width.to_string());
    model.set_field("imageHeight", height.to_string());
    model.set_field("samplesPerPixel", "1");
    model.set_field("pixelType", "UInt8");
    model.set_field("container", "BigTiff");

    let mut writer = MemoryBinaryWriter::new();
    TiffBackend.serialize_model(&model, &mut writer).unwrap();
    {
        let mut sink = TiffBackend.open_image_sink(&mut writer, &model).unwrap();
        let layout = *sink.layout();
        let tile = Tile::new(
            TileId::new(0),
            TileIndex::new(0, 0, 0),
            TileRegion::new(
                0,
                0,
                TileExtent::new(layout.tile_size.width, layout.tile_size.height),
            ),
            &payload,
        );
        sink.write_tile(&tile).unwrap();
    }
    let bytes = writer.take_buffer();

    let mut reader = MemoryBinaryReader::from_slice(&bytes);
    let mut source = TiffBackend.open_image_source(&mut reader).unwrap();
    let back = source
        .read_tile(TileIndex::new(0, 0, 0))
        .expect("read BigTIFF strip");
    assert_eq!(back.data(), &payload[..]);

    let (header, _) = parse_first_ifd(&bytes);
    assert!(header.is_big_tiff);
}

/// Compressed-tiled BigTIFF round-trip: the Sink back-patches per-tile
/// TileOffsets/TileByteCounts at 8-byte LONG8 slots; the file must read back
/// with identical pixels.
#[test]
fn bigtiff_tiled_lzw_pixels_round_trip() {
    let (width, height) = (32u32, 48u32); // 2 cols x 3 rows of 16x16 tiles
    let payload: Vec<u8> = (0..(width * height) as usize)
        .map(|i| (((i as u32) / width) % 17) as u8)
        .collect();

    let mut model = StorageModel::new();
    model.set_field("imageWidth", width.to_string());
    model.set_field("imageHeight", height.to_string());
    model.set_field("samplesPerPixel", "1");
    model.set_field("pixelType", "UInt8");
    model.set_field("tileWidth", "16");
    model.set_field("tileHeight", "16");
    model.set_field("compression", "LZW");
    model.set_field("predictor", "None");
    model.set_field("container", "BigTiff");

    let mut writer = MemoryBinaryWriter::new();
    TiffBackend.serialize_model(&model, &mut writer).unwrap();
    {
        let mut sink = TiffBackend.open_image_sink(&mut writer, &model).unwrap();
        let layout = *sink.layout();
        let cols = layout.columns(0);
        let rows = layout.rows(0);
        let tile_bytes = 16u32 * 16;
        for row in 0..rows {
            for col in 0..cols {
                let linear = (row * cols + col) as usize;
                let start = linear * tile_bytes as usize;
                let tile = Tile::new(
                    TileId::new(linear as u64),
                    TileIndex::new(col, row, 0),
                    TileRegion::new(0, 0, TileExtent::new(16, 16)),
                    &payload[start..start + tile_bytes as usize],
                );
                sink.write_tile(&tile).unwrap();
            }
        }
    }
    let bytes = writer.take_buffer();

    let mut reader = MemoryBinaryReader::from_slice(&bytes);
    let mut source = TiffBackend.open_image_source(&mut reader).unwrap();
    let layout = *source.layout();
    let cols = layout.columns(0);
    let rows = layout.rows(0);
    let mut decoded = Vec::new();
    for row in 0..rows {
        for col in 0..cols {
            let t = source.read_tile(TileIndex::new(col, row, 0)).unwrap();
            decoded.extend_from_slice(t.data());
        }
    }
    assert_eq!(decoded, payload, "BigTIFF tiled-LZW pixels must round-trip");

    let (header, _) = parse_first_ifd(&bytes);
    assert!(header.is_big_tiff);
}

/// Classic TIFF pixel output must remain byte-identical for offsets that fit
/// its representation (guarded by the existing golden corpus); this guards
/// the small-file classic path end to end.
#[test]
fn classic_small_strip_pixels_round_trip_unchanged() {
    let (width, height) = (32u32, 16u32);
    let payload: Vec<u8> = (0..(width * height) as usize)
        .map(|i| (i % 251) as u8)
        .collect();

    let mut model = StorageModel::new();
    model.set_field("imageWidth", width.to_string());
    model.set_field("imageHeight", height.to_string());
    model.set_field("samplesPerPixel", "1");
    model.set_field("pixelType", "UInt8");

    let mut writer = MemoryBinaryWriter::new();
    TiffBackend.serialize_model(&model, &mut writer).unwrap();
    {
        let mut sink = TiffBackend.open_image_sink(&mut writer, &model).unwrap();
        let layout = *sink.layout();
        let tile = Tile::new(
            TileId::new(0),
            TileIndex::new(0, 0, 0),
            TileRegion::new(
                0,
                0,
                TileExtent::new(layout.tile_size.width, layout.tile_size.height),
            ),
            &payload,
        );
        sink.write_tile(&tile).unwrap();
    }
    let bytes = writer.take_buffer();

    let mut reader = MemoryBinaryReader::from_slice(&bytes);
    let (header, _) = parse_first_ifd(&bytes);
    assert!(!header.is_big_tiff);
    let mut source = TiffBackend.open_image_source(&mut reader).unwrap();
    let back = source.read_tile(TileIndex::new(0, 0, 0)).unwrap();
    assert_eq!(back.data(), &payload[..]);
}
