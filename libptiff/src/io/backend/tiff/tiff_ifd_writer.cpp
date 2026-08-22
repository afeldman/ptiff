#include <algorithm>
#include <array>
#include <cstddef>
#include <span>

#include <ptiff/io/backend/tiff/tiff_endian.hpp>
#include <ptiff/io/backend/tiff/tiff_ifd_writer.hpp>

namespace ptiff::io::backend::tiff {

namespace {

constexpr std::uint64_t kClassicEntrySize = 12;
constexpr std::uint64_t kBigTiffEntrySize = 20;
constexpr std::uint8_t kClassicValueAreaSize = 4;
constexpr std::uint8_t kBigTiffValueAreaSize = 8;
constexpr std::uint64_t kClassicCountFieldSize = 2;
constexpr std::uint64_t kBigTiffCountFieldSize = 8;
constexpr std::uint64_t kClassicNextIfdSize = 4;
constexpr std::uint64_t kBigTiffNextIfdSize = 8;

std::uint64_t entrySize(bool isBigTiff) noexcept {
    return isBigTiff ? kBigTiffEntrySize : kClassicEntrySize;
}

std::uint8_t valueAreaSize(bool isBigTiff) noexcept {
    return isBigTiff ? kBigTiffValueAreaSize : kClassicValueAreaSize;
}

std::uint64_t countFieldSize(bool isBigTiff) noexcept {
    return isBigTiff ? kBigTiffCountFieldSize : kClassicCountFieldSize;
}

std::uint64_t nextIfdSize(bool isBigTiff) noexcept {
    return isBigTiff ? kBigTiffNextIfdSize : kClassicNextIfdSize;
}

std::uint64_t entryValueByteSize(const TiffIfdEntryToWrite& entry) noexcept {
    return static_cast<std::uint64_t>(fieldTypeSize(entry.fieldType)) * entry.values.size();
}

void writeElement(std::span<std::byte> out, FieldType fieldType, std::uint32_t value) {
    switch (fieldType) {
    case FieldType::Byte: {
        out[0] = static_cast<std::byte>(value & 0xFF);
        break;
    }
    case FieldType::Short: {
        writeU16(out, static_cast<std::uint16_t>(value), Endian::Little);
        break;
    }
    case FieldType::Long: {
        writeU32(out, value, Endian::Little);
        break;
    }
    case FieldType::Long8: {
        writeU64(out, value, Endian::Little);
        break;
    }
    }
}

void writeElements(std::span<std::byte> out, const TiffIfdEntryToWrite& entry) {
    const auto elementSize = fieldTypeSize(entry.fieldType);
    std::uint64_t cursor = 0;
    for (auto value : entry.values) {
        writeElement(out.subspan(cursor, elementSize), entry.fieldType, value);
        cursor += elementSize;
    }
}

} // namespace

std::uint64_t tiffIfdByteSize(const std::vector<TiffIfdEntryToWrite>& entries,
                              bool isBigTiff) noexcept {
    std::uint64_t size =
        countFieldSize(isBigTiff) + entries.size() * entrySize(isBigTiff) + nextIfdSize(isBigTiff);
    const auto valueArea = valueAreaSize(isBigTiff);
    for (const auto& entry : entries) {
        const auto valueBytes = entryValueByteSize(entry);
        if (valueBytes > valueArea) {
            size += valueBytes;
        }
    }
    return size;
}

Result<void> writeTiffIfd(BinaryWriter& writer,
                          std::vector<TiffIfdEntryToWrite> entries,
                          bool isBigTiff,
                          std::uint64_t nextIfdOffset) {
    std::ranges::sort(entries, {}, &TiffIfdEntryToWrite::tagId);

    auto ifdStartResult = writer.position();
    if (!ifdStartResult.has_value()) {
        return std::unexpected(ifdStartResult.error());
    }
    const std::uint64_t ifdStart = *ifdStartResult;
    const auto recordSize = entrySize(isBigTiff);
    const auto valueArea = valueAreaSize(isBigTiff);
    const std::uint64_t fixedSize =
        countFieldSize(isBigTiff) + entries.size() * recordSize + nextIfdSize(isBigTiff);

    std::vector<std::uint64_t> outOfLineOffsets(entries.size(), 0);
    std::uint64_t outOfLineOffset = ifdStart + fixedSize;
    for (std::size_t i = 0; i < entries.size(); ++i) {
        const auto valueBytes = entryValueByteSize(entries[i]);
        if (valueBytes > valueArea) {
            outOfLineOffsets[i] = outOfLineOffset;
            outOfLineOffset += valueBytes;
        }
    }

    if (isBigTiff) {
        std::array<std::byte, 8> countBytes{};
        writeU64(countBytes, static_cast<std::uint64_t>(entries.size()), Endian::Little);
        auto countWrite = writer.write(countBytes);
        if (!countWrite.has_value()) {
            return std::unexpected(countWrite.error());
        }
    } else {
        std::array<std::byte, 2> countBytes{};
        writeU16(countBytes, static_cast<std::uint16_t>(entries.size()), Endian::Little);
        auto countWrite = writer.write(countBytes);
        if (!countWrite.has_value()) {
            return std::unexpected(countWrite.error());
        }
    }

    for (std::size_t i = 0; i < entries.size(); ++i) {
        const auto& entry = entries[i];
        std::vector<std::byte> record(recordSize, std::byte{0});
        writeU16(std::span<std::byte>{record}.first(2), entry.tagId, Endian::Little);
        writeU16(std::span<std::byte>{record}.subspan(2, 2),
                 static_cast<std::uint16_t>(entry.fieldType),
                 Endian::Little);

        const auto valueBytes = entryValueByteSize(entry);
        if (isBigTiff) {
            writeU64(std::span<std::byte>{record}.subspan(4, 8),
                     static_cast<std::uint64_t>(entry.values.size()),
                     Endian::Little);
            if (valueBytes <= valueArea) {
                writeElements(std::span<std::byte>{record}.subspan(12, valueBytes), entry);
            } else {
                writeU64(std::span<std::byte>{record}.subspan(12, 8),
                         outOfLineOffsets[i],
                         Endian::Little);
            }
        } else {
            writeU32(std::span<std::byte>{record}.subspan(4, 4),
                     static_cast<std::uint32_t>(entry.values.size()),
                     Endian::Little);
            if (valueBytes <= valueArea) {
                writeElements(std::span<std::byte>{record}.subspan(8, valueBytes), entry);
            } else {
                writeU32(std::span<std::byte>{record}.subspan(8, 4),
                         static_cast<std::uint32_t>(outOfLineOffsets[i]),
                         Endian::Little);
            }
        }

        auto entryWrite = writer.write(record);
        if (!entryWrite.has_value()) {
            return std::unexpected(entryWrite.error());
        }
    }

    // Next-IFD offset: the absolute offset of the following IFD in a multi-image file, or 0 if
    // this is the last (only) IFD. Width is 4 bytes classic / 8 bytes BigTIFF, little-endian.
    if (isBigTiff) {
        std::array<std::byte, 8> nextIfdBytes{};
        writeU64(nextIfdBytes, nextIfdOffset, Endian::Little);
        auto nextIfdWrite = writer.write(nextIfdBytes);
        if (!nextIfdWrite.has_value()) {
            return std::unexpected(nextIfdWrite.error());
        }
    } else {
        std::array<std::byte, 4> nextIfdBytes{};
        writeU32(nextIfdBytes, static_cast<std::uint32_t>(nextIfdOffset), Endian::Little);
        auto nextIfdWrite = writer.write(nextIfdBytes);
        if (!nextIfdWrite.has_value()) {
            return std::unexpected(nextIfdWrite.error());
        }
    }

    for (const auto& entry : entries) {
        const auto valueBytes = entryValueByteSize(entry);
        if (valueBytes > valueArea) {
            std::vector<std::byte> buffer(valueBytes);
            writeElements(buffer, entry);
            auto valueWrite = writer.write(buffer);
            if (!valueWrite.has_value()) {
                return std::unexpected(valueWrite.error());
            }
        }
    }

    return {};
}

} // namespace ptiff::io::backend::tiff
