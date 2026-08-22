//! TIFF IFD parse: tag → resolved value array.
//!
//! Mirrors `ptiff::io::backend::tiff::tiff_ifd.{hpp,cpp}`.

use std::collections::BTreeMap;

use crate::io::BinaryReader;
use crate::{Error, Result};

use super::checked_arithmetic::{checked_add_u64, checked_mul_u64, K_MAX_TAG_COUNT};
use super::endian::{read_u16, read_u32, read_u64, Endian};
use super::tag::{field_type_size, field_type_size_raw, FieldType, RawTagEntry};

const K_CLASSIC_ENTRY_SIZE: u64 = 12;
const K_BIG_TIFF_ENTRY_SIZE: u64 = 20;
const K_CLASSIC_VALUE_AREA_SIZE: u8 = 4;
const K_BIG_TIFF_VALUE_AREA_SIZE: u8 = 8;

/// One parsed IFD: every entry's tag id mapped to its fully-resolved array of
/// unsigned 64-bit values. `read_tiff_ifd` resolves both inline and
/// offset-indirected entries eagerly, so callers never see the raw on-disk
/// encoding.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TiffIfd {
    tags: BTreeMap<u16, Vec<u64>>,
    next_ifd_offset: u64,
}

impl TiffIfd {
    /// Sets (or replaces) the value array for `tag_id`.
    pub fn set_tag(&mut self, tag_id: u16, values: Vec<u64>) {
        self.tags.insert(tag_id, values);
    }

    /// The full value array for `tag_id`.
    ///
    /// # Errors
    ///
    /// [`crate::ErrorCode::NotFound`] if `tag_id` was never set in this IFD.
    pub fn tag(&self, tag_id: u16) -> Result<&[u64]> {
        self.tags
            .get(&tag_id)
            .map(Vec::as_slice)
            .ok_or_else(|| Error::not_found("TiffIfd::tag: tag not present"))
    }

    /// Convenience for single-value tags.
    ///
    /// # Errors
    ///
    /// [`crate::ErrorCode::NotFound`] if absent, [`crate::ErrorCode::InvalidArgument`]
    /// if the tag is present with zero values.
    pub fn single_value(&self, tag_id: u16) -> Result<u64> {
        let values = self.tag(tag_id)?;
        values
            .first()
            .copied()
            .ok_or_else(|| Error::invalid_argument("TiffIfd::single_value: tag has no values"))
    }

    /// `fallback` if `tag_id` is absent; still errors if present with zero
    /// values.
    pub fn single_value_or(&self, tag_id: u16, fallback: u64) -> Result<u64> {
        match self.single_value(tag_id) {
            Ok(v) => Ok(v),
            Err(e) if e.code() == crate::ErrorCode::NotFound => Ok(fallback),
            Err(e) => Err(e),
        }
    }

    /// Absolute file offset of the following IFD in the chain, or 0 if this is
    /// the last IFD. This is the trailing next-IFD field (a fixed 4/8-byte
    /// trailer, not a tag entry) that `read_tiff_ifd` reads after the entry
    /// table.
    #[must_use]
    pub const fn next_ifd_offset(&self) -> u64 {
        self.next_ifd_offset
    }

    /// Sets the trailing next-IFD field value; used by `read_tiff_ifd`.
    pub fn set_next_ifd_offset(&mut self, offset: u64) {
        self.next_ifd_offset = offset;
    }
}

fn decode_elements(
    bytes: &[u8],
    field_type: FieldType,
    count: u64,
    endian: Endian,
) -> Result<Vec<u64>> {
    let element_size = field_type_size(field_type)
        .ok_or_else(|| Error::invalid_argument("decode_elements: unsupported TIFF field type"))?;
    let element_size = u64::from(element_size);

    // count is untrusted (RFC-0001 §13): it must be representable by the actual
    // backing bytes, and bounded by a sane hard ceiling, before we reserve/iterate.
    if count > K_MAX_TAG_COUNT || count > bytes.len() as u64 / element_size {
        return Err(Error::invalid_argument(
            "decode_elements: tag element count exceeds a safe bound",
        ));
    }

    let mut values = Vec::with_capacity(count as usize);
    for i in 0..count {
        let start = (i * element_size) as usize;
        let element = &bytes[start..start + element_size as usize];
        match field_type {
            FieldType::Byte => values.push(u64::from(element[0])),
            FieldType::Short => values.push(u64::from(read_u16(element, endian))),
            FieldType::Long => values.push(u64::from(read_u32(element, endian))),
            FieldType::Long8 => values.push(read_u64(element, endian)),
        }
    }
    Ok(values)
}

fn read_raw_entry(
    reader: &mut impl BinaryReader,
    endian: Endian,
    is_big_tiff: bool,
) -> Result<RawTagEntry> {
    let entry_size = if is_big_tiff {
        K_BIG_TIFF_ENTRY_SIZE
    } else {
        K_CLASSIC_ENTRY_SIZE
    };
    let mut raw = [0u8; 20];
    let read_result = reader.read(&mut raw[..entry_size as usize])?;
    if read_result != entry_size as usize {
        return Err(Error::invalid_argument(
            "read_raw_entry: truncated IFD entry",
        ));
    }

    let mut entry = RawTagEntry {
        tag_id: read_u16(&raw[..2], endian),
        field_type_raw: read_u16(&raw[2..4], endian),
        count: 0,
        value_area: [0u8; 8],
    };

    if is_big_tiff {
        entry.count = read_u64(&raw[4..12], endian);
        entry.value_area.copy_from_slice(&raw[12..20]);
    } else {
        entry.count = u64::from(read_u32(&raw[4..8], endian));
        entry.value_area[..4].copy_from_slice(&raw[8..12]);
    }
    Ok(entry)
}

/// Converts a raw on-disk field type into a known [`FieldType`].
///
/// Callers must only invoke this after [`field_type_size_raw`] returned `Some`,
/// so the raw value is guaranteed to be one of the four supported types.
#[must_use]
fn field_type_of(entry: &RawTagEntry) -> FieldType {
    match FieldType::from_u16(entry.field_type_raw) {
        Some(ft) => ft,
        None => {
            // Unreachable: this is only called after the size check rejected
            // unsupported raw field types.
            FieldType::Byte
        }
    }
}

fn resolve_entry(
    reader: &mut impl BinaryReader,
    entry: &RawTagEntry,
    endian: Endian,
    is_big_tiff: bool,
) -> Result<Vec<u64>> {
    let element_size = field_type_size_raw(entry.field_type_raw)
        .ok_or_else(|| Error::invalid_argument("resolve_entry: unsupported TIFF field type"))?;
    let field_type = field_type_of(entry);
    let element_size = u64::from(element_size);
    let total_bytes = entry.count.checked_mul(element_size).ok_or_else(|| {
        Error::invalid_argument("resolve_entry: entry count overflows total byte size")
    })?;
    let value_area_size = if is_big_tiff {
        K_BIG_TIFF_VALUE_AREA_SIZE
    } else {
        K_CLASSIC_VALUE_AREA_SIZE
    };

    if total_bytes <= u64::from(value_area_size) {
        decode_elements(
            &entry.value_area[..total_bytes as usize],
            field_type,
            entry.count,
            endian,
        )
    } else {
        let offset = if is_big_tiff {
            read_u64(&entry.value_area, endian)
        } else {
            u64::from(read_u32(&entry.value_area[..4], endian))
        };

        let file_size = reader.size()?;
        if offset > file_size || total_bytes > file_size - offset {
            return Err(Error::invalid_argument(
                "resolve_entry: out-of-line value exceeds file bounds",
            ));
        }

        reader.seek(offset)?;
        let mut buffer = vec![0u8; total_bytes as usize];
        let read_result = reader.read(&mut buffer)?;
        if read_result != total_bytes as usize {
            return Err(Error::invalid_argument(
                "resolve_entry: truncated out-of-line tag value",
            ));
        }
        decode_elements(&buffer, field_type, entry.count, endian)
    }
}

/// Parses the IFD at `ifd_offset` (classic 12-byte or BigTIFF 20-byte entries
/// per `is_big_tiff`), resolving every entry's value eagerly via `reader`.
///
/// # Errors
///
/// [`crate::ErrorCode::InvalidArgument`] on a truncated IFD, an entry whose
/// field type this backend does not understand, or a value/offset read failure.
pub fn read_tiff_ifd(
    reader: &mut impl BinaryReader,
    ifd_offset: u64,
    endian: Endian,
    is_big_tiff: bool,
) -> Result<TiffIfd> {
    reader.seek(ifd_offset)?;

    let count_field_size: u64 = if is_big_tiff { 8 } else { 2 };
    let entry_count = if is_big_tiff {
        let mut count_bytes = [0u8; 8];
        let count_read = reader.read(&mut count_bytes)?;
        if count_read != 8 {
            return Err(Error::invalid_argument(
                "read_tiff_ifd: truncated IFD entry count",
            ));
        }
        read_u64(&count_bytes, endian)
    } else {
        let mut count_bytes = [0u8; 2];
        let count_read = reader.read(&mut count_bytes)?;
        if count_read != 2 {
            return Err(Error::invalid_argument(
                "read_tiff_ifd: truncated IFD entry count",
            ));
        }
        u64::from(read_u16(&count_bytes, endian))
    };

    let entry_size: u64 = if is_big_tiff {
        K_BIG_TIFF_ENTRY_SIZE
    } else {
        K_CLASSIC_ENTRY_SIZE
    };
    // The IFD's offset fields are untrusted (RFC-0001 §13): derive the entry-table
    // end with checked arithmetic and validate it against the underlying file size
    // before iterating, so a wrapped or absurdly large entry_count cannot cause
    // out-of-bounds reads or unbounded work.
    let entry_table_start = checked_add_u64(ifd_offset, count_field_size)?;
    let table_bytes = checked_mul_u64(entry_count, entry_size)?;
    let entry_table_end = checked_add_u64(entry_table_start, table_bytes)?;
    let file_size = reader.size()?;
    if entry_table_end > file_size {
        return Err(Error::invalid_argument(
            "read_tiff_ifd: IFD entry table exceeds file bounds (inflated entry count)",
        ));
    }

    let mut ifd = TiffIfd::default();
    for i in 0..entry_count {
        // Re-seek before every entry: resolve_entry may jump elsewhere in the file
        // to fetch an out-of-line array, so the cursor cannot be assumed to stay at
        // the entry table.
        let entry_offset = checked_add_u64(entry_table_start, checked_mul_u64(i, entry_size)?)?;
        reader.seek(entry_offset)?;
        let raw_entry = read_raw_entry(reader, endian, is_big_tiff)?;

        // Unknown/unsupported field type (e.g. RATIONAL, ASCII, DOUBLE -- common in
        // real-world GeoTIFF extension tags this backend never reads). Skip the entry
        // rather than failing the whole parse: downstream only ever looks up a fixed,
        // known set of baseline tags, so an entry we can't decode only matters if
        // something later actually requires it -- which surfaces as its own
        // "required tag is missing" error.
        if field_type_size_raw(raw_entry.field_type_raw).is_none() {
            continue;
        }
        let values = resolve_entry(reader, &raw_entry, endian, is_big_tiff)?;
        ifd.set_tag(raw_entry.tag_id, values);
    }

    // The trailing next-IFD field sits right after the entry table (a fixed 4-byte
    // classic / 8-byte BigTIFF trailer, not a tag entry). Read it to expose the
    // following IFD in a multi-image chain; 0 means this is the last IFD.
    reader.seek(entry_table_end)?;
    let next_ifd_field_size = if is_big_tiff { 8 } else { 4 };
    let mut next_ifd_bytes = [0u8; 8];
    let next_read = reader.read(&mut next_ifd_bytes[..next_ifd_field_size])?;
    if next_read != next_ifd_field_size {
        return Err(Error::invalid_argument(
            "read_tiff_ifd: truncated next-IFD offset",
        ));
    }
    ifd.set_next_ifd_offset(if is_big_tiff {
        read_u64(&next_ifd_bytes[..8], endian)
    } else {
        u64::from(read_u32(&next_ifd_bytes[..4], endian))
    });

    Ok(ifd)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::memory_binary_reader::MemoryBinaryReader;
    use crate::ErrorCode;

    fn reader(bytes: Vec<u8>) -> MemoryBinaryReader {
        MemoryBinaryReader::from_vec(bytes)
    }

    /// Entry count + next-IFD trailer bytes appended by the helper below.
    fn tag_id(id: u16) -> u16 {
        id
    }

    #[test]
    fn resolve_inline_short_entry() {
        // IFD at offset 0: 1 entry (ImageWidth, SHORT, count=1, value=7 inline), next-IFD=0.
        let mut r = reader(vec![
            0x01, 0x00, //
            0x00, 0x01, 0x03, 0x00, 0x01, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, //
            0x00, 0x00, 0x00, 0x00,
        ]);
        let ifd = read_tiff_ifd(&mut r, 0, Endian::Little, false).unwrap();
        assert_eq!(ifd.single_value(tag_id(256)).unwrap(), 7);
    }

    #[test]
    fn resolve_inline_long_entry() {
        // ImageWidth is 256=0x0100, but here we use StripOffsets 0x0111=273.
        let mut r = reader(vec![
            0x01, 0x00, //
            0x11, 0x01, 0x04, 0x00, 0x01, 0x00, 0x00, 0x00, 0xC8, 0x00, 0x00, 0x00, //
            0x00, 0x00, 0x00, 0x00,
        ]);
        let ifd = read_tiff_ifd(&mut r, 0, Endian::Little, false).unwrap();
        assert_eq!(ifd.single_value(tag_id(273)).unwrap(), 200);
    }

    #[test]
    fn resolve_offset_indirected_array_entry() {
        // BitsPerSample (0x0102), SHORT, count=3 (6 bytes > 4-byte value area) -> offset 30.
        let mut data = vec![
            0x01, 0x00, //
            0x02, 0x01, 0x03, 0x00, 0x03, 0x00, 0x00, 0x00, 0x1E, 0x00, 0x00, 0x00, //
            0x00, 0x00, 0x00, 0x00,
        ];
        data.resize(30, 0);
        data.extend_from_slice(&[0x08, 0x00, 0x08, 0x00, 0x08, 0x00]);

        let mut r = reader(data);
        let ifd = read_tiff_ifd(&mut r, 0, Endian::Little, false).unwrap();
        assert_eq!(ifd.tag(tag_id(258)).unwrap(), &[8, 8, 8]);
    }

    #[test]
    fn resolve_big_tiff_long8_entry() {
        let mut r = reader(vec![
            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // entry count = 1
            0x11, 0x01, 0x10, 0x00, // StripOffsets, LONG8
            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // count = 1
            0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // value = 512
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // next IFD = 0
        ]);
        let ifd = read_tiff_ifd(&mut r, 0, Endian::Little, true).unwrap();
        assert_eq!(ifd.single_value(tag_id(273)).unwrap(), 512);
    }

    #[test]
    fn skip_unsupported_field_type_entry() {
        // ImageWidth (0x0100), FLOAT (11) -- unsupported, must be skipped.
        let mut r = reader(vec![
            0x01, 0x00, //
            0x00, 0x01, 0x0B, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
            0x00, 0x00, 0x00, 0x00,
        ]);
        let ifd = read_tiff_ifd(&mut r, 0, Endian::Little, false).unwrap();
        let e = ifd.tag(0x0100).unwrap_err();
        assert_eq!(e.code(), ErrorCode::NotFound);
    }

    #[test]
    fn resolve_later_entry_after_skipping_unsupported_one() {
        // Entry 1: ImageLength FLOAT (unsupported, skipped).
        // Entry 2: StripOffsets LONG = 200.
        let mut r = reader(vec![
            0x02, 0x00, //
            0x01, 0x01, 0x0B, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
            0x11, 0x01, 0x04, 0x00, 0x01, 0x00, 0x00, 0x00, 0xC8, 0x00, 0x00, 0x00, //
            0x00, 0x00, 0x00, 0x00,
        ]);
        let ifd = read_tiff_ifd(&mut r, 0, Endian::Little, false).unwrap();
        assert_eq!(ifd.single_value(tag_id(273)).unwrap(), 200);
    }

    #[test]
    fn reject_truncated_entry() {
        let mut r = reader(vec![0x01, 0x00, 0x00, 0x01, 0x03, 0x00]);
        let e = read_tiff_ifd(&mut r, 0, Endian::Little, false).unwrap_err();
        assert_eq!(e.code(), ErrorCode::InvalidArgument);
    }

    #[test]
    fn absent_tag_reports_not_found() {
        let mut r = reader(vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
        let ifd = read_tiff_ifd(&mut r, 0, Endian::Little, false).unwrap();
        let e = ifd.tag(tag_id(256)).unwrap_err();
        assert_eq!(e.code(), ErrorCode::NotFound);
    }

    #[test]
    fn single_value_or_falls_back_when_absent() {
        let mut r = reader(vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
        let ifd = read_tiff_ifd(&mut r, 0, Endian::Little, false).unwrap();
        assert_eq!(ifd.single_value_or(tag_id(284), 1).unwrap(), 1);
    }

    #[test]
    fn reject_entry_count_overflows_total_byte_size() {
        // BigTIFF: StripOffsets LONG8, count = UINT64_MAX -> overflow.
        let mut r = reader(vec![
            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
            0x11, 0x01, 0x10, 0x00, //
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, //
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
        ]);
        let e = read_tiff_ifd(&mut r, 0, Endian::Little, true).unwrap_err();
        assert_eq!(e.code(), ErrorCode::InvalidArgument);
    }

    #[test]
    fn reject_out_of_line_value_exceeding_file() {
        // BitsPerSample SHORT count = 0xFFFFFFFF (totalBytes ~8.6GB > file).
        let mut r = reader(vec![
            0x01, 0x00, //
            0x02, 0x01, 0x03, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0x1E, 0x00, 0x00, 0x00, //
            0x00, 0x00, 0x00, 0x00,
        ]);
        let e = read_tiff_ifd(&mut r, 0, Endian::Little, false).unwrap_err();
        assert_eq!(e.code(), ErrorCode::InvalidArgument);
    }

    #[test]
    fn parses_next_ifd_offset_chain() {
        // Two IFDs chained: first has ImageWidth=7 and next-IFD=30; second (at 30)
        // is an empty IFD with next-IFD=0.
        let mut data = vec![
            0x01, 0x00, //
            0x00, 0x01, 0x03, 0x00, 0x01, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, //
            30, 0, 0, 0, //
        ];
        // The second IFD starts at 30 with count=0 and next-IFD=0.
        data.resize(30, 0);
        data.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);

        let mut r = reader(data);
        let first = read_tiff_ifd(&mut r, 0, Endian::Little, false).unwrap();
        assert_eq!(first.single_value(tag_id(256)).unwrap(), 7);
        assert_eq!(first.next_ifd_offset(), 30);

        let second = read_tiff_ifd(&mut r, 30, Endian::Little, false).unwrap();
        assert_eq!(second.next_ifd_offset(), 0);
    }
}
