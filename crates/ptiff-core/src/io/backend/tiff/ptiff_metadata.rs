//! PTIFF extension metadata (RFC-7002): the versioned binary payload carried
//! by the five private TIFF tags 65001-65005.
//!
//! Mirrors `ptiff::io::backend::tiff::ptiff_metadata.{hpp,cpp}`.

use crate::io::StorageModel;
use crate::{Error, Result};

/// PTIFF extension payload format version. Readers encountering a version they
/// do not know MUST NOT fail the whole file -- they skip the tag (treating the
/// domain as absent), exactly as a non-PTIFF reader already ignores the private
/// tags.
pub const K_PTIFF_METADATA_VERSION: u16 = 1;

/// ASCII magic bytes `PTIFF` written at the start of every private-tag payload
/// so the decoder can distinguish a PTIFF extension payload from arbitrary
/// bytes left in a private tag by other writers.
pub const K_PTIFF_MAGIC: [u8; 5] = *b"PTIFF";

/// One human-readable (key, value) record.
///
/// Keys select the well-known per-domain field names; unknown keys are
/// preserved verbatim so a future writer's additional fields survive a
/// read-modify-write round trip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataRecord {
    /// The record key (domain field name).
    pub key: String,
    /// The record value.
    pub value: String,
}

impl MetadataRecord {
    /// Builds a new record.
    #[must_use]
    pub const fn new(key: String, value: String) -> Self {
        Self { key, value }
    }
}

fn write_u16(out: &mut Vec<u8>, v: u16) {
    out.push((v & 0xFF) as u8);
    out.push(((v >> 8) & 0xFF) as u8);
}

fn write_u32(out: &mut Vec<u8>, v: u32) {
    out.push((v & 0xFF) as u8);
    out.push(((v >> 8) & 0xFF) as u8);
    out.push(((v >> 16) & 0xFF) as u8);
    out.push(((v >> 24) & 0xFF) as u8);
}

fn append_ascii(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(s.as_bytes());
}

fn read_u16(s: &[u8]) -> u16 {
    let mut v: u16 = 0;
    for (i, &b) in s.iter().take(2).enumerate() {
        v |= u16::from(b) << (8 * i);
    }
    v
}

fn read_u32(s: &[u8]) -> u32 {
    let mut v: u32 = 0;
    for (i, &b) in s.iter().take(4).enumerate() {
        v |= u32::from(b) << (8 * i);
    }
    v
}

fn has_magic(data: &[u8]) -> bool {
    data.len() >= K_PTIFF_MAGIC.len() && data[..K_PTIFF_MAGIC.len()] == K_PTIFF_MAGIC
}

/// Encodes a set of (key, value) records into a byte payload for the given tag.
///
/// Records are sorted by key (ascending, byte-wise) so the encoding is
/// canonical/deterministic: the same logical metadata always yields the same
/// bytes, which is what the golden digest depends on.
///
/// # Errors
///
/// Returns [`crate::ErrorCode::InvalidArgument`] on key/value length overflow
/// (> 65535 / > 4 GiB respectively) or an excessive record count.
pub fn encode_metadata_payload(records: &[MetadataRecord]) -> Result<Vec<u8>> {
    // Canonical order: sort by key (byte-wise), keeping ties stable so the
    // encoding is deterministic even for duplicate keys.
    let mut sorted = records.to_vec();
    sorted.sort_by(|a, b| a.key.cmp(&b.key));

    let mut out = Vec::with_capacity(7 + 8 * sorted.len());
    out.extend_from_slice(&K_PTIFF_MAGIC);
    write_u16(&mut out, K_PTIFF_METADATA_VERSION);
    if sorted.len() > u32::MAX as usize {
        return Err(Error::invalid_argument(
            "encodeMetadataPayload: too many records",
        ));
    }
    write_u32(&mut out, sorted.len() as u32);
    for rec in &sorted {
        if rec.key.len() > u16::MAX as usize {
            return Err(Error::invalid_argument(
                "encodeMetadataPayload: metadata key longer than 65535 bytes",
            ));
        }
        if rec.value.len() > u32::MAX as usize {
            return Err(Error::invalid_argument(
                "encodeMetadataPayload: metadata value longer than 4 GiB",
            ));
        }
        write_u16(&mut out, rec.key.len() as u16);
        append_ascii(&mut out, &rec.key);
        write_u32(&mut out, rec.value.len() as u32);
        append_ascii(&mut out, &rec.value);
    }
    Ok(out)
}

/// Decodes a byte payload back into its (key, value) records.
///
/// # Errors
///
/// Returns [`crate::ErrorCode::InvalidArgument`] if the payload is empty, lacks
/// the PTIFF magic, has a version > [`K_PTIFF_METADATA_VERSION`], is
/// malformed/truncated, or contains a non-byte value. A payload validated here
/// is guaranteed to round-trip.
pub fn decode_metadata_payload(byte_values: &[u64]) -> Result<Vec<MetadataRecord>> {
    // Converge the reader's `u64` byte values into a plain byte buffer.
    let mut bytes = Vec::with_capacity(byte_values.len());
    for &v in byte_values {
        if v > 0xFF {
            return Err(Error::invalid_argument(
                "decodeMetadataPayload: non-byte value in PTIFF extension tag",
            ));
        }
        bytes.push(v as u8);
    }
    let data = &bytes;

    if !has_magic(data) {
        return Err(Error::invalid_argument(
            "decodeMetadataPayload: missing PTIFF magic in extension tag payload",
        ));
    }
    let mut cursor = K_PTIFF_MAGIC.len();
    if data.len() < cursor + 2 {
        return Err(Error::invalid_argument(
            "decodeMetadataPayload: truncated PTIFF extension payload header",
        ));
    }
    let version = read_u16(&data[cursor..cursor + 2]);
    cursor += 2;
    if version > K_PTIFF_METADATA_VERSION {
        // A payload from a future writer: do not mis-parse.
        return Err(Error::invalid_argument(
            "decodeMetadataPayload: unsupported PTIFF extension payload version",
        ));
    }
    if data.len() < cursor + 4 {
        return Err(Error::invalid_argument(
            "decodeMetadataPayload: truncated PTIFF extension payload header",
        ));
    }
    let rec_count = read_u32(&data[cursor..cursor + 4]);
    cursor += 4;

    let mut records = Vec::with_capacity(rec_count as usize);
    for _ in 0..rec_count {
        if data.len() < cursor + 2 {
            return Err(Error::invalid_argument(
                "decodeMetadataPayload: truncated PTIFF extension record key length",
            ));
        }
        let key_len = read_u16(&data[cursor..cursor + 2]) as usize;
        cursor += 2;
        if data.len() < cursor + key_len {
            return Err(Error::invalid_argument(
                "decodeMetadataPayload: truncated PTIFF extension record key",
            ));
        }
        let key = String::from_utf8_lossy(&data[cursor..cursor + key_len]).into_owned();
        cursor += key_len;

        if data.len() < cursor + 4 {
            return Err(Error::invalid_argument(
                "decodeMetadataPayload: truncated PTIFF extension record value length",
            ));
        }
        let val_len = read_u32(&data[cursor..cursor + 4]) as usize;
        cursor += 4;
        if data.len() < cursor + val_len {
            return Err(Error::invalid_argument(
                "decodeMetadataPayload: truncated PTIFF extension record value",
            ));
        }
        let value = String::from_utf8_lossy(&data[cursor..cursor + val_len]).into_owned();
        cursor += val_len;

        records.push(MetadataRecord::new(key, value));
    }
    Ok(records)
}

/// Converts the (key, value) archive of a PTIFF extension tag into a
/// [`StorageModel`] node's fields, or reverse: reads every `ptiff.<domain>.*`
/// field of `node`, strips the prefix, and returns the ordered record list to
/// hand to [`encode_metadata_payload`]. `prefix` is e.g. `"spice"` (so the
/// StorageModel holds `ptiff.spice.frame`).
///
/// [`StorageModel`]: crate::io::StorageModel
#[must_use]
pub fn records_from_storage_model(node: &StorageModel, prefix: &str) -> Vec<MetadataRecord> {
    // StorageModel fields are already stored in a BTreeMap (ascending key
    // order). Collect any field whose key starts with `ptiff.<prefix>.` and
    // strip that prefix; the read path writes with the same prefix so the
    // roundtrip is symmetric.
    let pfx = format!("ptiff.{prefix}.");
    let mut records = Vec::new();
    node.for_each_field(|key, value| {
        if key.len() > pfx.len() && key.starts_with(&pfx) {
            records.push(MetadataRecord::new(
                key[pfx.len()..].to_string(),
                value.to_string(),
            ));
        }
    });
    // Preserves the stable ascending key order from the StorageModel map.
    records
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::StorageModel;

    fn rec(key: &str, value: &str) -> MetadataRecord {
        MetadataRecord::new(key.to_string(), value.to_string())
    }

    #[test]
    fn encode_is_canonical_and_sorted() {
        let payload =
            encode_metadata_payload(&[rec("b", "2"), rec("a", "1"), rec("c", "3")]).unwrap();
        // Header: magic(5) + version(2) + count(4) = 11.
        // 11 header bytes + 3 records * 8 bytes each (klen2+k1+vlen4+v1) = 35.
        assert_eq!(payload.len(), 35);
        assert_eq!(&payload[0..5], b"PTIFF");
        assert_eq!(read_u16(&payload[5..7]), 1);
        assert_eq!(read_u32(&payload[7..11]), 3);
        // First record: klen (u16 LE) = 1 at [11..13], key 'a' at [13].
        assert_eq!(read_u16(&payload[11..13]), 1); // klen
        assert_eq!(payload[13], b'a');
    }

    #[test]
    fn encode_rejects_oversized_key() {
        let long_key = "k".repeat(65536);
        let err = encode_metadata_payload(&[rec(&long_key, "v")]).unwrap_err();
        assert_eq!(err.code(), crate::ErrorCode::InvalidArgument);
    }

    #[test]
    fn decode_round_trip() {
        let records = vec![rec("frame", "MOUNT_SPACECRAFT"), rec("target", "MARS")];
        let payload = encode_metadata_payload(&records).unwrap();
        let byte_values: Vec<u64> = payload.iter().map(|&b| u64::from(b)).collect();
        let decoded = decode_metadata_payload(&byte_values).unwrap();
        // Sorted ascending by key -> frame, target.
        assert_eq!(
            decoded,
            vec![rec("frame", "MOUNT_SPACECRAFT"), rec("target", "MARS")]
        );
    }

    #[test]
    fn decode_rejects_non_byte_value() {
        let err = decode_metadata_payload(&[0, 0x100]).unwrap_err();
        assert_eq!(err.code(), crate::ErrorCode::InvalidArgument);
    }

    #[test]
    fn decode_rejects_missing_magic() {
        let err = decode_metadata_payload(&[0, 1, 2, 3, 4, 5]).unwrap_err();
        assert_eq!(err.code(), crate::ErrorCode::InvalidArgument);
    }

    #[test]
    fn decode_rejects_future_version() {
        // Valid magic, version = 99.
        let mut payload = K_PTIFF_MAGIC.to_vec();
        write_u16(&mut payload, 99);
        write_u32(&mut payload, 0);
        let byte_values: Vec<u64> = payload.iter().map(|&b| u64::from(b)).collect();
        let err = decode_metadata_payload(&byte_values).unwrap_err();
        assert_eq!(err.code(), crate::ErrorCode::InvalidArgument);
    }

    #[test]
    fn decode_rejects_truncated() {
        let records = vec![rec("a", "longvalue")];
        let payload = encode_metadata_payload(&records).unwrap();
        let byte_values: Vec<u64> = payload.iter().map(|&b| u64::from(b)).collect();
        // Truncate the trailing value.
        let err = decode_metadata_payload(&byte_values[..byte_values.len() - 3]).unwrap_err();
        assert_eq!(err.code(), crate::ErrorCode::InvalidArgument);
    }

    #[test]
    fn records_from_storage_model_strips_prefix() {
        let mut m = StorageModel::new();
        m.set_field("ptiff.spice.frame", "MOUNT");
        m.set_field("ptiff.spice.target", "MARS");
        m.set_field("imageWidth", "64"); // unrelated
        let records = records_from_storage_model(&m, "spice");
        assert_eq!(records, vec![rec("frame", "MOUNT"), rec("target", "MARS")]);
    }

    #[test]
    fn records_from_storage_model_ignores_other_domains() {
        let mut m = StorageModel::new();
        m.set_field("ptiff.camera.k", "fx");
        let records = records_from_storage_model(&m, "spice");
        assert!(records.is_empty());
    }
}
