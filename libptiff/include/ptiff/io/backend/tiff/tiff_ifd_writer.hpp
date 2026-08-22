#pragma once

#include <cstdint>
#include <vector>

#include <ptiff/core/result.hpp>
#include <ptiff/io/backend/tiff/tiff_tag.hpp>
#include <ptiff/io/binary_writer.hpp>

namespace ptiff::io::backend::tiff {

/// One IFD entry to write. Values are plain u32-range unsigned integers, sufficient for every
/// field type this backend's writer emits (SHORT, LONG); written as wider on-disk elements
/// (e.g. LONG8) when `fieldType` calls for it, with no precision loss since values never exceed
/// u32 range.
struct TiffIfdEntryToWrite {
    std::uint16_t tagId = 0;
    FieldType fieldType = FieldType::Short;
    std::vector<std::uint32_t> values;
};

/// Byte size the IFD built from `entries` will occupy on disk. Classic (`isBigTiff == false`): 2
/// (entry count) + entries.size()*12 + 4 (next-IFD offset) + the total size of every entry whose
/// values don't fit the 4-byte inline value area. BigTIFF (`isBigTiff == true`): 8 (entry count) +
/// entries.size()*20 + 8 (next-IFD offset) + the total size of every entry whose values don't fit
/// the 8-byte inline value area. Pure arithmetic -- doesn't touch a BinaryWriter, so callers can
/// compute file offsets (e.g. where pixel data starts) before writing a single byte.
[[nodiscard]] std::uint64_t tiffIfdByteSize(const std::vector<TiffIfdEntryToWrite>& entries,
                                            bool isBigTiff = false) noexcept;

/// Writes a classic 12-byte-entry or BigTIFF 20-byte-entry IFD at `writer`'s current position:
/// entry count, `entries` sorted ascending by tag id (TIFF 6.0 requires ascending order; sorts a
/// local copy rather than trusting the caller), each entry's record, a next-IFD offset (default
/// 0, meaning "no following IFD" -- a multi-image file chains additional IFDs through this
/// field), then every out-of-line value's bytes. Always little-endian. `isBigTiff` selects the
/// variant; defaults to classic.
[[nodiscard]] Result<void> writeTiffIfd(BinaryWriter& writer,
                                        std::vector<TiffIfdEntryToWrite> entries,
                                        bool isBigTiff = false,
                                        std::uint64_t nextIfdOffset = 0);

} // namespace ptiff::io::backend::tiff
