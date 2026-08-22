//! Serializes an IFD entry table (classic or BigTIFF) through a
//! [`crate::io::BinaryWriter`].
//!
//! Mirrors `ptiff::io::backend::tiff::tiff_ifd_writer.{hpp,cpp}`.

use crate::io::backend::tiff::tag::{field_type_size, FieldType};
use crate::io::backend::tiff::{write_u16, write_u32, write_u64, Endian};
use crate::io::BinaryWriter;
use crate::Result;

/// Classic TIFF 6.0 IFD entry record size (bytes).
pub const K_CLASSIC_ENTRY_SIZE: u64 = 12;
/// BigTIFF IFD entry record size (bytes).
pub const K_BIG_TIFF_ENTRY_SIZE: u64 = 20;
/// Classic TIFF inline value area size (bytes).
pub const K_CLASSIC_VALUE_AREA_SIZE: u8 = 4;
/// BigTIFF inline value area size (bytes).
pub const K_BIG_TIFF_VALUE_AREA_SIZE: u8 = 8;
/// Classic TIFF entry-count field size (bytes).
pub const K_CLASSIC_COUNT_FIELD_SIZE: u64 = 2;
/// BigTIFF entry-count field size (bytes).
pub const K_BIG_TIFF_COUNT_FIELD_SIZE: u64 = 8;
/// Classic TIFF next-IFD offset field size (bytes).
pub const K_CLASSIC_NEXT_IFD_SIZE: u64 = 4;
/// BigTIFF next-IFD offset field size (bytes).
pub const K_BIG_TIFF_NEXT_IFD_SIZE: u64 = 8;

/// One IFD entry to write.
///
/// Values are plain `u32`-range unsigned integers, sufficient for every field
/// type this backend's writer emits (SHORT, LONG); written as wider on-disk
/// elements (e.g. LONG8) when `field_type` calls for it, with no precision
/// loss since values never exceed the `u32` range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TiffIfdEntryToWrite {
    /// The 16-bit tag id.
    pub tag_id: u16,
    /// The field type governing how each value is written.
    pub field_type: FieldType,
    /// The values to write (one per on-disk element).
    pub values: Vec<u32>,
}

impl TiffIfdEntryToWrite {
    /// Builds a new entry.
    #[must_use]
    pub const fn new(tag_id: u16, field_type: FieldType, values: Vec<u32>) -> Self {
        Self {
            tag_id,
            field_type,
            values,
        }
    }
}

/// Byte size one `field_type` element occupies on disk. Returns 0 for a field
/// type the writer does not emit (defensive; the writer only ever passes types
/// it can serialize).
#[must_use]
const fn element_size(field_type: FieldType) -> u64 {
    match field_type_size(field_type) {
        Some(size) => size as u64,
        None => 0,
    }
}

/// Byte size of `entry`'s value bytes on disk (elements * per-element size).
#[must_use]
fn entry_value_byte_size(entry: &TiffIfdEntryToWrite) -> u64 {
    element_size(entry.field_type) * entry.values.len() as u64
}

/// Byte size the IFD built from `entries` will occupy on disk. Classic TIFF
/// (`is_big_tiff == false`): 2 (entry count) + `entries.len()*12` + 4 (next-IFD
/// offset) + the total size of every entry whose values don't fit the 4-byte
/// inline value area. BigTIFF (`is_big_tiff == true`): 8 + `len*20` + 8 + the
/// out-of-line total. Pure arithmetic -- doesn't touch a
/// [`crate::io::BinaryWriter`], so callers can compute file offsets (e.g. where
/// pixel data starts) before writing a single byte.
#[must_use]
pub fn tiff_ifd_byte_size(entries: &[TiffIfdEntryToWrite], is_big_tiff: bool) -> u64 {
    let count_field = if is_big_tiff {
        K_BIG_TIFF_COUNT_FIELD_SIZE
    } else {
        K_CLASSIC_COUNT_FIELD_SIZE
    };
    let entry_size = if is_big_tiff {
        K_BIG_TIFF_ENTRY_SIZE
    } else {
        K_CLASSIC_ENTRY_SIZE
    };
    let next_ifd = if is_big_tiff {
        K_BIG_TIFF_NEXT_IFD_SIZE
    } else {
        K_CLASSIC_NEXT_IFD_SIZE
    };
    let value_area = if is_big_tiff {
        K_BIG_TIFF_VALUE_AREA_SIZE
    } else {
        K_CLASSIC_VALUE_AREA_SIZE
    };

    let mut size = count_field + (entries.len() as u64) * entry_size + next_ifd;
    for entry in entries {
        let value_bytes = entry_value_byte_size(entry);
        if value_bytes > u64::from(value_area) {
            size += value_bytes;
        }
    }
    size
}

/// Absolute byte offset, relative to the IFD's first byte (its entry-count
/// field), of every value slot each entry writes on disk.
///
/// Values that fit the inline value area live inside the entry's own record;
/// larger value arrays are written out of line, immediately after the fixed
/// part of the IFD, in ascending-tag order (the same order `write_tiff_ifd`
/// produces). The returned map keys on tag id, each a `Vec` whose `k`-th entry
/// is the offset (from the IFD's count field) of that tag's `k`-th on-disk
/// value. A caller that back-patches a value array after serializing the file
/// (e.g. per-tile compressed offsets/sizes) adds the IFD's absolute start
/// offset to these to reach the value's byte position.
#[must_use]
pub fn value_slot_offsets_relative(
    entries: &[TiffIfdEntryToWrite],
    is_big_tiff: bool,
) -> std::collections::BTreeMap<u16, Vec<u64>> {
    let entry_size = if is_big_tiff {
        K_BIG_TIFF_ENTRY_SIZE
    } else {
        K_CLASSIC_ENTRY_SIZE
    };
    let value_area = if is_big_tiff {
        K_BIG_TIFF_VALUE_AREA_SIZE
    } else {
        K_CLASSIC_VALUE_AREA_SIZE
    };
    let count_field = if is_big_tiff {
        K_BIG_TIFF_COUNT_FIELD_SIZE
    } else {
        K_CLASSIC_COUNT_FIELD_SIZE
    };
    let next_ifd = if is_big_tiff {
        K_BIG_TIFF_NEXT_IFD_SIZE
    } else {
        K_CLASSIC_NEXT_IFD_SIZE
    };
    // Classic entry holds the value area at record[8..12]; BigTIFF at record[12..20].
    let value_offset_in_record = if is_big_tiff { 12u64 } else { 8u64 };

    // Out-of-line value areas are laid out contiguously after the fixed part,
    // in ascending entry order (write_tiff_ifd sorts entries before writing).
    let mut sorted: Vec<&TiffIfdEntryToWrite> = entries.iter().collect();
    sorted.sort_by_key(|e| e.tag_id);
    let fixed_size = count_field + (sorted.len() as u64) * entry_size + next_ifd;

    let mut out_of_line = Vec::with_capacity(sorted.len());
    let mut cursor = fixed_size;
    for entry in &sorted {
        let value_bytes = entry_value_byte_size(entry);
        if value_bytes > u64::from(value_area) {
            out_of_line.push(cursor);
            cursor += value_bytes;
        } else {
            out_of_line.push(u64::MAX); // marker: values are inline
        }
    }

    let mut map: std::collections::BTreeMap<u16, Vec<u64>> = std::collections::BTreeMap::new();
    for (i, entry) in sorted.iter().enumerate() {
        let elem = element_size(entry.field_type);
        let base = if out_of_line[i] == u64::MAX {
            count_field + (i as u64) * entry_size + value_offset_in_record
        } else {
            out_of_line[i]
        };
        let slots: Vec<u64> = (0..entry.values.len() as u64)
            .map(|k| base + k * elem)
            .collect();
        map.insert(entry.tag_id, slots);
    }
    map
}

/// Writes `value` into the first `element_size` bytes of `out`, honoring the
/// given field type (always little-endian).
///
/// # Panics
///
/// Panics if `out.len() < element_size` or the field type is unsupported.
fn write_element(out: &mut [u8], field_type: FieldType, value: u32) {
    match field_type {
        FieldType::Byte => {
            out[0] = (value & 0xFF) as u8;
        }
        FieldType::Short => {
            write_u16(out, value as u16, Endian::Little);
        }
        FieldType::Long => {
            write_u32(out, value, Endian::Little);
        }
        FieldType::Long8 => {
            write_u64(out, u64::from(value), Endian::Little);
        }
    }
}

/// Writes all of `entry`'s values into `out`.
///
/// # Panics
///
/// Panics if `out.len() < entry_value_byte_size(entry)`.
fn write_elements(out: &mut [u8], entry: &TiffIfdEntryToWrite) {
    let esize = entry_value_byte_size(entry) as usize;
    assert!(
        out.len() >= esize,
        "output buffer smaller than entry value bytes"
    );
    let elem = element_size(entry.field_type) as usize;
    let mut cursor = 0;
    for &value in &entry.values {
        write_element(&mut out[cursor..cursor + elem], entry.field_type, value);
        cursor += elem;
    }
}

/// Writes a classic 12-byte-entry or BigTIFF 20-byte-entry IFD at `writer`'s
/// current position: entry count, `entries` sorted ascending by tag id (TIFF
/// 6.0 requires ascending order; sorts a local copy rather than trusting the
/// caller), each entry's record, a next-IFD offset (default 0, meaning "no
/// following IFD" -- a multi-image file chains additional IFDs through this
/// field), then every out-of-line value's bytes. Always little-endian.
/// `is_big_tiff` selects the variant; defaults to classic.
///
/// # Errors
///
/// Returns an error if any underlying write or seek fails.
pub fn write_tiff_ifd<W: BinaryWriter + ?Sized>(
    writer: &mut W,
    mut entries: Vec<TiffIfdEntryToWrite>,
    is_big_tiff: bool,
    next_ifd_offset: u64,
) -> Result<()> {
    // TIFF 6.0 requires entries in ascending tag order.
    entries.sort_by_key(|e| e.tag_id);

    let ifd_start = writer.position()?;
    let entry_size = if is_big_tiff {
        K_BIG_TIFF_ENTRY_SIZE
    } else {
        K_CLASSIC_ENTRY_SIZE
    };
    let value_area = if is_big_tiff {
        K_BIG_TIFF_VALUE_AREA_SIZE
    } else {
        K_CLASSIC_VALUE_AREA_SIZE
    };
    let fixed_size = if is_big_tiff {
        K_BIG_TIFF_COUNT_FIELD_SIZE
    } else {
        K_CLASSIC_COUNT_FIELD_SIZE
    } + (entries.len() as u64) * entry_size
        + if is_big_tiff {
            K_BIG_TIFF_NEXT_IFD_SIZE
        } else {
            K_CLASSIC_NEXT_IFD_SIZE
        };

    // Absolute offsets of the out-of-line value areas, laid out immediately
    // after the fixed part of the IFD.
    let mut out_of_line_offsets = vec![0u64; entries.len()];
    let mut out_of_line_offset = ifd_start + fixed_size;
    for (i, entry) in entries.iter().enumerate() {
        let value_bytes = entry_value_byte_size(entry);
        if value_bytes > u64::from(value_area) {
            out_of_line_offsets[i] = out_of_line_offset;
            out_of_line_offset += value_bytes;
        }
    }

    // Entry count field.
    if is_big_tiff {
        let mut count_bytes = [0u8; 8];
        write_u64(&mut count_bytes, entries.len() as u64, Endian::Little);
        writer.write(&count_bytes)?;
    } else {
        let mut count_bytes = [0u8; 2];
        write_u16(&mut count_bytes, entries.len() as u16, Endian::Little);
        writer.write(&count_bytes)?;
    }

    // Entry records.
    for (i, entry) in entries.iter().enumerate() {
        let mut record = vec![0u8; entry_size as usize];
        write_u16(&mut record[0..2], entry.tag_id, Endian::Little);
        let field_type_raw = match entry.field_type {
            FieldType::Byte => 1u16,
            FieldType::Short => 3,
            FieldType::Long => 4,
            FieldType::Long8 => 16,
        };
        write_u16(&mut record[2..4], field_type_raw, Endian::Little);

        let value_bytes = entry_value_byte_size(entry);
        if is_big_tiff {
            write_u64(
                &mut record[4..12],
                entry.values.len() as u64,
                Endian::Little,
            );
            if value_bytes <= u64::from(value_area) {
                write_elements(&mut record[12..12 + value_bytes as usize], entry);
            } else {
                write_u64(&mut record[12..20], out_of_line_offsets[i], Endian::Little);
            }
        } else {
            write_u32(&mut record[4..8], entry.values.len() as u32, Endian::Little);
            if value_bytes <= u64::from(value_area) {
                write_elements(&mut record[8..8 + value_bytes as usize], entry);
            } else {
                write_u32(
                    &mut record[8..12],
                    out_of_line_offsets[i] as u32,
                    Endian::Little,
                );
            }
        }
        writer.write(&record)?;
    }

    // Next-IFD offset.
    if is_big_tiff {
        let mut next_ifd_bytes = [0u8; 8];
        write_u64(&mut next_ifd_bytes, next_ifd_offset, Endian::Little);
        writer.write(&next_ifd_bytes)?;
    } else {
        let mut next_ifd_bytes = [0u8; 4];
        write_u32(&mut next_ifd_bytes, next_ifd_offset as u32, Endian::Little);
        writer.write(&next_ifd_bytes)?;
    }

    // Out-of-line value bytes.
    for entry in &entries {
        let value_bytes = entry_value_byte_size(entry);
        if value_bytes > u64::from(value_area) {
            let mut buffer = vec![0u8; value_bytes as usize];
            write_elements(&mut buffer, entry);
            writer.write(&buffer)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::backend::tiff::{read_u16, read_u32};
    use crate::io::memory_binary_writer::MemoryBinaryWriter;

    fn long_entry(tag: u16, values: Vec<u32>) -> TiffIfdEntryToWrite {
        TiffIfdEntryToWrite::new(tag, FieldType::Long, values)
    }

    fn short_entry(tag: u16, values: Vec<u32>) -> TiffIfdEntryToWrite {
        TiffIfdEntryToWrite::new(tag, FieldType::Short, values)
    }

    #[test]
    fn value_slot_offsets_relative_match_written_layout() {
        use crate::io::backend::tiff::ifd_writer::value_slot_offsets_relative;
        // TileOffsets (324) with 3 values -> out of line (12 bytes > 4 inline),
        // preceded by ImageWidth (inline). Sorted order: 256, 324, 325.
        let entries = vec![
            long_entry(325, vec![0, 0, 0]), // TileByteCounts (after 324)
            long_entry(256, vec![64]),      // ImageWidth (inline)
            long_entry(324, vec![0, 0, 0]), // TileOffsets
        ];
        // Sorted: 256 (inline), 324 (out-of-line at fixed_size), 325 (out-of-line after 324).
        let fixed_size = 2 + 3 * 12 + 4; // count + 3 entries + next-IFD
        let slots = value_slot_offsets_relative(&entries, false);
        let image_width_slots = &slots[&256];
        assert_eq!(image_width_slots.len(), 1);
        // Inline value slot: count field (2) + first entry record (offset 0) + value offset (8).
        assert_eq!(image_width_slots[0], 10);
        let offsets_slots = &slots[&324];
        // 324 is the second sorted entry; its out-of-line area starts right
        // after the fixed part (fixed_size), which follows the 256 (inline) entry.
        assert_eq!(offsets_slots[0], fixed_size);
        assert_eq!(offsets_slots[1], fixed_size + 4);
        assert_eq!(offsets_slots[2], fixed_size + 8);
        let counts_slots = &slots[&325];
        // 325's out-of-line area follows 324's 12 bytes.
        assert_eq!(counts_slots[0], fixed_size + 12);
        assert_eq!(counts_slots[2], fixed_size + 12 + 8);
        // The helper's offsets must agree byte-for-byte with a real write.
        let mut w = MemoryBinaryWriter::new();
        write_tiff_ifd(&mut w, entries, false, 0).unwrap();
        let buf = w.take_buffer();
        for (tag, expected) in [(324u16, offsets_slots), (325, counts_slots)] {
            for (k, addr) in expected.iter().enumerate() {
                let raw = read_u32(&buf[*addr as usize..*addr as usize + 4], Endian::Little);
                assert_eq!(raw, 0, "tag {tag} slot {k} should be the placeholder value");
            }
        }
    }

    #[test]
    fn tiff_ifd_byte_size_classic_kept_inline() {
        // 2 count + 1*12 entry + 4 next = 18; one LONG value fits inline (4 == 4).
        let entries = vec![long_entry(256, vec![64])];
        assert_eq!(tiff_ifd_byte_size(&entries, false), 18);
    }

    #[test]
    fn tiff_ifd_byte_size_classic_out_of_line() {
        // Two LONG values = 8 bytes > 4 => 2 + 12 + 4 + 8 = 26.
        let entries = vec![long_entry(324, vec![0, 0])];
        assert_eq!(tiff_ifd_byte_size(&entries, false), 26);
    }

    #[test]
    fn tiff_ifd_byte_size_big_tiff_kept_inline() {
        // 8 count + 1*20 + 8 next = 36; any value (even LONG8 = 8) fits inline (8 == 8).
        let entries = vec![short_entry(258, vec![8])];
        assert_eq!(tiff_ifd_byte_size(&entries, true), 36);
    }

    #[test]
    fn tiff_ifd_byte_size_big_tiff_out_of_line() {
        // Three LONG values = 12 > 8 => 8 + 20 + 8 + 12 = 48.
        let entries = vec![long_entry(324, vec![0, 0, 0])];
        assert_eq!(tiff_ifd_byte_size(&entries, true), 48);
    }

    #[test]
    fn write_tiff_ifd_classic_single_inline() {
        let mut w = MemoryBinaryWriter::new();
        let entries = vec![long_entry(256, vec![64])];
        write_tiff_ifd(&mut w, entries, false, 0).unwrap();
        let buf = w.take_buffer();

        // count = 1
        assert_eq!(read_u16(&buf[0..2], Endian::Little), 1);
        // tag = 256
        assert_eq!(read_u16(&buf[2..4], Endian::Little), 256);
        // field type = 4 (LONG)
        assert_eq!(read_u16(&buf[4..6], Endian::Little), 4);
        // count = 1
        assert_eq!(read_u32(&buf[6..10], Endian::Little), 1);
        // inline value = 64
        assert_eq!(read_u32(&buf[10..14], Endian::Little), 64);
        // next-IFD = 0
        assert_eq!(read_u32(&buf[14..18], Endian::Little), 0);
        // total size
        assert_eq!(buf.len(), 18);
    }

    #[test]
    fn write_tiff_ifd_sorts_entries_ascending() {
        let mut w = MemoryBinaryWriter::new();
        // Insert out of order; write must sort ascending by tag id.
        let entries = vec![long_entry(257, vec![32]), long_entry(256, vec![64])];
        write_tiff_ifd(&mut w, entries, false, 0).unwrap();
        let buf = w.take_buffer();

        assert_eq!(read_u16(&buf[0..2], Endian::Little), 2);
        assert_eq!(read_u16(&buf[2..4], Endian::Little), 256);
        assert_eq!(read_u16(&buf[14..16], Endian::Little), 257);
    }

    #[test]
    fn write_tiff_ifd_classic_out_of_line_values() {
        let mut w = MemoryBinaryWriter::new();
        // TileOffsets with 3 values (12 bytes) must go out of line.
        let entries = vec![long_entry(324, vec![100, 200, 300])];
        write_tiff_ifd(&mut w, entries, false, 0).unwrap();
        let buf = w.take_buffer();

        // count = 1
        assert_eq!(read_u16(&buf[0..2], Endian::Little), 1);
        // tag = 324
        assert_eq!(read_u16(&buf[2..4], Endian::Little), 324);
        // count = 3
        assert_eq!(read_u32(&buf[6..10], Endian::Little), 3);
        // value area holds the out-of-line OFFSET (= fixed size of IFD = 18)
        assert_eq!(read_u32(&buf[10..14], Endian::Little), 18);
        // next-IFD = 0
        assert_eq!(read_u32(&buf[14..18], Endian::Little), 0);
        // out-of-line values follow
        assert_eq!(read_u32(&buf[18..22], Endian::Little), 100);
        assert_eq!(read_u32(&buf[22..26], Endian::Little), 200);
        assert_eq!(read_u32(&buf[26..30], Endian::Little), 300);
        assert_eq!(buf.len(), 30);
    }

    #[test]
    fn write_tiff_ifd_big_tiff_out_of_line_long8() {
        let mut w = MemoryBinaryWriter::new();
        // Two LONG values (8 bytes) fit inline in BigTIFF.
        let entries = vec![long_entry(324, vec![1, 2])];
        write_tiff_ifd(&mut w, entries, true, 0).unwrap();
        let buf = w.take_buffer();

        assert_eq!(read_u64(&buf[0..8], Endian::Little), 1);
        assert_eq!(read_u16(&buf[8..10], Endian::Little), 324);
        assert_eq!(read_u16(&buf[10..12], Endian::Little), 4);
        // count = 2 (u64)
        assert_eq!(read_u64(&buf[12..20], Endian::Little), 2);
        // inline values at offset 20 (two u32 little-endian)
        assert_eq!(read_u32(&buf[20..24], Endian::Little), 1);
        assert_eq!(read_u32(&buf[24..28], Endian::Little), 2);
        // next-IFD = 0 (u64)
        assert_eq!(read_u64(&buf[28..36], Endian::Little), 0);
        assert_eq!(buf.len(), 36);
    }

    fn read_u64(bytes: &[u8], endian: Endian) -> u64 {
        crate::io::backend::tiff::read_u64(bytes, endian)
    }

    #[test]
    fn write_then_read_round_trips_inline() {
        let mut w = MemoryBinaryWriter::new();
        let entries = vec![
            short_entry(258, vec![8]),
            long_entry(256, vec![64]),
            long_entry(257, vec![32]),
        ];
        write_tiff_ifd(&mut w, entries, false, 0).unwrap();
        let buf = w.take_buffer();
        let mut r = crate::io::memory_binary_reader::MemoryBinaryReader::from_vec(buf);
        let ifd =
            crate::io::backend::tiff::read_tiff_ifd(&mut r, 0, Endian::Little, false).unwrap();
        assert_eq!(ifd.single_value(258).unwrap(), 8);
        assert_eq!(ifd.single_value(256).unwrap(), 64);
        assert_eq!(ifd.single_value(257).unwrap(), 32);
    }
}
