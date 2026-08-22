#pragma once

#include <array>
#include <cstddef>
#include <cstdint>
#include <string>
#include <vector>

#include <ptiff/core/result.hpp>
#include <ptiff/io/storage_model.hpp>

namespace ptiff::io::backend::tiff {

/// PTIFF extension metadata (RFC-7002): the versioned binary payload carried by the five
/// private TIFF tags 65001-65005.
///
/// Each tag holds a small, self-describing, versioned map of string key/value pairs. The
/// payload encodes the scientific domain for that tag (SPICE, camera geometry, CRS,
/// scientific layers, provenance) as an ordered list of (key, value) UTF-8 records. Binary
/// layout (all integers little-endian, values as decimal strings):
///
///   magic  "PTIFF"           (5 bytes)
///   ver    u16               payload format version; currently 1
///   nrec   u32               number of (key, value) records
///   per record:
///     klen u16, key bytes
///     vlen u32, value bytes
///
/// Readers that encounter a version they do not know MUST NOT fail the whole file: they may
/// skip the tag (treating the domain as absent) exactly as a non-PTIFF reader already ignores
/// the private tags. Versioning is what keeps this scheme forward-compatible while a writer may
/// freely add keys in a new version.
inline constexpr std::uint16_t kPtiffMetadataVersion = 1;

/// ASCII magic bytes `PTIFF` written at the start of every private-tag payload so the decoder
/// can distinguish a PTIFF extension payload from arbitrary bytes left in a private tag by
/// other writers (and reject the latter cleanly rather than mis-parsing).
inline constexpr std::array<std::byte, 5> kPtiffMagic = {
    std::byte{'P'}, std::byte{'T'}, std::byte{'I'}, std::byte{'F'}, std::byte{'F'}};

/// One human-readable (key, value) record. Keys select the well-known per-domain field names
/// (see the per-domain tables in ptiff_metadata.cpp); unknown keys are preserved verbatim so a
/// future writer's additional fields survive a read-modify-write round trip.
struct MetadataRecord {
    std::string key;
    std::string value;
};

/// Encodes a set of (key, value) records into a byte payload for the given tag. Records are
/// sorted by key (ascending, byte-wise) so the encoding is canonical/deterministic: the same
/// logical metadata always yields the same bytes, which is what the golden digest depends on.
/// Error::InvalidArgument on key/value length overflow (> 65535 / > 4 GiB respectively).
[[nodiscard]] Result<std::vector<std::byte>>
encodeMetadataPayload(const std::vector<MetadataRecord>& records);

/// Decodes a byte payload back into its (key, value) records. Error::InvalidArgument if the
/// payload is empty, lacks the PTIFF magic, has a version > kPtiffMetadataVersion, or is
/// malformed/truncated. A payload validated here is guaranteed to round-trip.
[[nodiscard]] Result<std::vector<MetadataRecord>>
decodeMetadataPayload(const std::vector<std::uint64_t>& byteValues);

/// Convenience: converts the (key, value) archive of a PTIFF extension tag into a
/// StorageModel node's fields (key-prefixed `ptiff.<domain>.<key>`), or reverse.
///
/// `encodeMetadataFields(node)` reads every `ptiff.<domain>.*` field of `node`, strips the
/// prefix, and returns the ordered record list to hand to encodeMetadataPayload. `prefix`
/// is e.g. `"spice"` (so the StorageModel holds `ptiff.spice.frame`).
[[nodiscard]] std::vector<MetadataRecord> recordsFromStorageModel(const StorageModel& node,
                                                                  std::string_view prefix);

} // namespace ptiff::io::backend::tiff
