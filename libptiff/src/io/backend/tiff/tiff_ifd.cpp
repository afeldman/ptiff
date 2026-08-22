#include <algorithm>
#include <array>
#include <cstddef>
#include <limits>
#include <utility>

#include <ptiff/io/backend/tiff/checked_arithmetic.hpp>
#include <ptiff/io/backend/tiff/tiff_ifd.hpp>
#include <ptiff/io/backend/tiff/tiff_tag.hpp>

namespace ptiff::io::backend::tiff {

void TiffIfd::setTag(std::uint16_t tagId, std::vector<std::uint64_t> values) {
    tags_[tagId] = std::move(values);
}

Result<std::vector<std::uint64_t>> TiffIfd::tag(std::uint16_t tagId) const {
    auto it = tags_.find(tagId);
    if (it == tags_.end()) {
        return std::unexpected(Error{ErrorCode::NotFound, "TiffIfd::tag: tag not present"});
    }
    return it->second;
}

Result<std::uint64_t> TiffIfd::singleValue(std::uint16_t tagId) const {
    auto values = tag(tagId);
    if (!values.has_value()) {
        return std::unexpected(values.error());
    }
    if (values->empty()) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "TiffIfd::singleValue: tag has no values"});
    }
    return (*values)[0];
}

Result<std::uint64_t> TiffIfd::singleValueOr(std::uint16_t tagId, std::uint64_t fallback) const {
    auto value = singleValue(tagId);
    if (!value.has_value()) {
        if (value.error().code() == ErrorCode::NotFound) {
            return fallback;
        }
        return std::unexpected(value.error());
    }
    return *value;
}

namespace {

constexpr std::uint64_t kClassicEntrySize = 12;
constexpr std::uint64_t kBigTiffEntrySize = 20;
constexpr std::uint8_t kClassicValueAreaSize = 4;
constexpr std::uint8_t kBigTiffValueAreaSize = 8;

Result<std::vector<std::uint64_t>> decodeElements(std::span<const std::byte> bytes,
                                                  FieldType fieldType,
                                                  std::uint64_t count,
                                                  Endian endian) {
    const auto elementSize = fieldTypeSize(fieldType);
    if (elementSize == 0) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "decodeElements: unsupported TIFF field type"});
    }
    // count is untrusted (RFC-0001 §13): it must be representable by the actual backing bytes,
    // and bounded by a sane hard ceiling, before we reserve/iterate. Rejecting here keeps a
    // crafted magnitude from turning into excessive allocation or out-of-bounds subspan reads.
    if (count > kMaxTagCount || count > bytes.size() / elementSize) {
        return std::unexpected(Error{ErrorCode::InvalidArgument,
                                     "decodeElements: tag element count exceeds a safe bound"});
    }

    std::vector<std::uint64_t> values;
    values.reserve(static_cast<std::size_t>(count));
    for (std::uint64_t i = 0; i < count; ++i) {
        const auto elementBytes = bytes.subspan(i * elementSize, elementSize);
        switch (fieldType) {
        case FieldType::Byte: {
            values.push_back(std::to_integer<std::uint64_t>(elementBytes[0]));
            break;
        }
        case FieldType::Short: {
            values.push_back(readU16(elementBytes, endian));
            break;
        }
        case FieldType::Long: {
            values.push_back(readU32(elementBytes, endian));
            break;
        }
        case FieldType::Long8: {
            values.push_back(readU64(elementBytes, endian));
            break;
        }
        }
    }
    return values;
}

Result<RawTagEntry> readRawEntry(BinaryReader& reader, Endian endian, bool isBigTiff) {
    std::array<std::byte, kBigTiffEntrySize> raw{};
    const std::uint64_t entrySize = isBigTiff ? kBigTiffEntrySize : kClassicEntrySize;
    auto readResult = reader.read(std::span<std::byte>{raw}.first(entrySize));
    if (!readResult.has_value()) {
        return std::unexpected(readResult.error());
    }
    if (*readResult != entrySize) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "readRawEntry: truncated IFD entry"});
    }

    RawTagEntry entry;
    entry.tagId = readU16(std::span<const std::byte>{raw}.first(2), endian);
    const auto rawFieldType = readU16(std::span<const std::byte>{raw}.subspan(2, 2), endian);
    entry.fieldType = static_cast<FieldType>(rawFieldType);

    if (isBigTiff) {
        entry.count = readU64(std::span<const std::byte>{raw}.subspan(4, 8), endian);
        std::copy_n(raw.begin() + 12, 8, entry.valueArea.begin());
    } else {
        entry.count = readU32(std::span<const std::byte>{raw}.subspan(4, 4), endian);
        std::copy_n(raw.begin() + 8, 4, entry.valueArea.begin());
    }
    return entry;
}

Result<std::vector<std::uint64_t>>
resolveEntry(BinaryReader& reader, const RawTagEntry& entry, Endian endian, bool isBigTiff) {
    const auto elementSize = fieldTypeSize(entry.fieldType);
    if (elementSize == 0) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "resolveEntry: unsupported TIFF field type"});
    }
    const std::uint64_t elementSize64 = static_cast<std::uint64_t>(elementSize);
    if (entry.count > std::numeric_limits<std::uint64_t>::max() / elementSize64) {
        return std::unexpected(Error{ErrorCode::InvalidArgument,
                                     "resolveEntry: entry count overflows total byte size"});
    }
    const std::uint64_t totalBytes = elementSize64 * entry.count;
    const std::uint8_t valueAreaSize = isBigTiff ? kBigTiffValueAreaSize : kClassicValueAreaSize;

    if (totalBytes <= valueAreaSize) {
        return decodeElements(std::span<const std::byte>{entry.valueArea}.first(totalBytes),
                              entry.fieldType,
                              entry.count,
                              endian);
    }

    const std::uint64_t offset =
        isBigTiff ? readU64(std::span<const std::byte>{entry.valueArea}, endian)
                  : readU32(std::span<const std::byte>{entry.valueArea}.first(4), endian);

    auto fileSize = reader.size();
    if (!fileSize.has_value()) {
        return std::unexpected(fileSize.error());
    }
    if (offset > *fileSize || totalBytes > *fileSize - offset) {
        return std::unexpected(Error{ErrorCode::InvalidArgument,
                                     "resolveEntry: out-of-line value exceeds file bounds"});
    }

    auto seekResult = reader.seek(offset);
    if (!seekResult.has_value()) {
        return std::unexpected(seekResult.error());
    }
    std::vector<std::byte> buffer(totalBytes);
    auto readResult = reader.read(std::span<std::byte>{buffer});
    if (!readResult.has_value()) {
        return std::unexpected(readResult.error());
    }
    if (*readResult != totalBytes) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "resolveEntry: truncated out-of-line tag value"});
    }
    return decodeElements(buffer, entry.fieldType, entry.count, endian);
}

} // namespace

Result<TiffIfd>
readTiffIfd(BinaryReader& reader, std::uint64_t ifdOffset, Endian endian, bool isBigTiff) {
    auto seekResult = reader.seek(ifdOffset);
    if (!seekResult.has_value()) {
        return std::unexpected(seekResult.error());
    }

    std::uint64_t entryCount = 0;
    const std::uint64_t countFieldSize = isBigTiff ? 8 : 2;
    std::array<std::byte, 8> countBytes{};
    auto countRead = reader.read(std::span<std::byte>{countBytes}.first(countFieldSize));
    if (!countRead.has_value()) {
        return std::unexpected(countRead.error());
    }
    if (*countRead != countFieldSize) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "readTiffIfd: truncated IFD entry count"});
    }
    entryCount = isBigTiff ? readU64(countBytes, endian) : readU16(countBytes, endian);

    const std::uint64_t entrySize = isBigTiff ? kBigTiffEntrySize : kClassicEntrySize;
    // The IFD's offset fields are untrusted (RFC-0001 §13): derive the entry-table end with
    // checked arithmetic and validate it against the underlying file size before iterating, so a
    // wrapped or absurdly large entryCount cannot cause out-of-bounds reads or unbounded work.
    auto entryTableStart = checkedAddU64(ifdOffset, countFieldSize);
    if (!entryTableStart.has_value()) {
        return std::unexpected(entryTableStart.error());
    }
    auto tableBytes = checkedMulU64(entryCount, entrySize);
    if (!tableBytes.has_value()) {
        return std::unexpected(tableBytes.error());
    }
    auto entryTableEnd = checkedAddU64(*entryTableStart, *tableBytes);
    if (!entryTableEnd.has_value()) {
        return std::unexpected(entryTableEnd.error());
    }
    auto fileSize = reader.size();
    if (!fileSize.has_value()) {
        return std::unexpected(fileSize.error());
    }
    // Require the whole entry table to live inside the file (the trailing next-IFD field is a
    // fixed 4/8 bytes beyond the table, validated below). This both bounds entryCount against a
    // realistic file and rejects offset-wrap attacks, instead of trusting reader.seek.
    if (*entryTableEnd > *fileSize) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument,
                  "readTiffIfd: IFD entry table exceeds file bounds (inflated entry count)"});
    }

    TiffIfd ifd;
    for (std::uint64_t i = 0; i < entryCount; ++i) {
        // Re-seek before every entry: resolveEntry may jump elsewhere in the file to fetch an
        // out-of-line array, so the cursor cannot be assumed to stay at the entry table.
        auto entryOffsetBytes = checkedMulU64(i, entrySize);
        if (!entryOffsetBytes.has_value()) {
            return std::unexpected(entryOffsetBytes.error());
        }
        auto entryOffset = checkedAddU64(*entryTableStart, *entryOffsetBytes);
        if (!entryOffset.has_value()) {
            return std::unexpected(entryOffset.error());
        }
        auto seekEntry = reader.seek(*entryOffset);
        if (!seekEntry.has_value()) {
            return std::unexpected(seekEntry.error());
        }
        auto rawEntry = readRawEntry(reader, endian, isBigTiff);
        if (!rawEntry.has_value()) {
            return std::unexpected(rawEntry.error());
        }
        if (fieldTypeSize(rawEntry->fieldType) == 0) {
            // Unknown/unsupported field type (e.g. RATIONAL, ASCII, DOUBLE -- common in
            // real-world GeoTIFF extension tags this backend never reads: ModelPixelScaleTag,
            // ModelTiepointTag, GeoAsciiParamsTag, GDAL metadata, ...). Skip the entry rather
            // than failing the whole parse: interpretTiffIfd only ever looks up a fixed, known
            // set of baseline tags, so an entry we can't decode only matters if something later
            // actually requires it -- which surfaces as its own "required tag is missing" error.
            continue;
        }
        auto values = resolveEntry(reader, *rawEntry, endian, isBigTiff);
        if (!values.has_value()) {
            return std::unexpected(values.error());
        }
        ifd.setTag(rawEntry->tagId, std::move(*values));
    }

    // The trailing next-IFD field sits right after the entry table (a fixed 4-byte classic /
    // 8-byte BigTIFF trailer, not a tag entry): its value starts *at* `entryTableEnd`. Read it to
    // expose the following IFD in a multi-image chain; 0 means this is the last IFD.
    auto seekNext = reader.seek(*entryTableEnd);
    if (!seekNext.has_value()) {
        return std::unexpected(seekNext.error());
    }
    std::array<std::byte, 8> nextIfdBytes{};
    const std::uint64_t nextIfdFieldSize = isBigTiff ? 8 : 4;
    auto nextRead = reader.read(std::span<std::byte>{nextIfdBytes}.first(nextIfdFieldSize));
    if (!nextRead.has_value()) {
        return std::unexpected(nextRead.error());
    }
    if (*nextRead != nextIfdFieldSize) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "readTiffIfd: truncated next-IFD offset"});
    }
    ifd.setNextIfdOffset(isBigTiff ? readU64(nextIfdBytes, endian) : readU32(nextIfdBytes, endian));

    return ifd;
}

} // namespace ptiff::io::backend::tiff
