#include <algorithm>
#include <cstddef>
#include <cstdint>
#include <limits>
#include <span>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#include <ptiff/core/error.hpp>
#include <ptiff/core/result.hpp>
#include <ptiff/io/backend/tiff/ptiff_metadata.hpp>
#include <ptiff/io/storage_model.hpp>

namespace ptiff::io::backend::tiff {

namespace {

void writeU16(std::vector<std::byte>& out, std::uint16_t v) {
    out.push_back(static_cast<std::byte>(v & 0xFF));
    out.push_back(static_cast<std::byte>((v >> 8) & 0xFF));
}

void writeU32(std::vector<std::byte>& out, std::uint32_t v) {
    for (unsigned i = 0; i < 4; ++i) {
        out.push_back(static_cast<std::byte>((v >> (8 * i)) & 0xFF));
    }
}

void appendAscii(std::vector<std::byte>& out, std::string_view s) {
    for (char c : s) {
        out.push_back(static_cast<std::byte>(static_cast<unsigned char>(c)));
    }
}

std::uint16_t readU16(std::span<const std::byte> s) {
    std::uint16_t v = 0;
    for (unsigned i = 0; i < 2; ++i) {
        v |= static_cast<std::uint16_t>(static_cast<uint8_t>(s[i])) << (8 * i);
    }
    return v;
}

std::uint32_t readU32(std::span<const std::byte> s) {
    std::uint32_t v = 0;
    for (unsigned i = 0; i < 4; ++i) {
        v |= static_cast<std::uint32_t>(static_cast<uint8_t>(s[i])) << (8 * i);
    }
    return v;
}

std::string asciiOf(std::span<const std::byte> s) {
    std::string out;
    out.reserve(s.size());
    for (const auto b : s) {
        out.push_back(static_cast<char>(static_cast<unsigned char>(b)));
    }
    return out;
}

bool hasMagic(std::span<const std::byte> data) {
    if (data.size() < kPtiffMagic.size()) {
        return false;
    }
    for (std::size_t i = 0; i < kPtiffMagic.size(); ++i) {
        if (data[i] != kPtiffMagic[i]) {
            return false;
        }
    }
    return true;
}

} // namespace

Result<std::vector<std::byte>> encodeMetadataPayload(const std::vector<MetadataRecord>& records) {
    // Canonical order: sort by key (byte-wise). Value ties keep a stable relative order via
    // std::stable_sort so the encoding is deterministic even for duplicate keys.
    auto sorted = records;
    std::stable_sort(
        sorted.begin(), sorted.end(), [](const MetadataRecord& a, const MetadataRecord& b) {
            return a.key < b.key;
        });

    std::vector<std::byte> out;
    out.reserve(7 + 8 * sorted.size());
    for (auto b : kPtiffMagic) {
        out.push_back(b);
    }
    writeU16(out, kPtiffMetadataVersion);
    if (sorted.size() > std::numeric_limits<std::uint32_t>::max()) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "encodeMetadataPayload: too many records"});
    }
    writeU32(out, static_cast<std::uint32_t>(sorted.size()));
    for (const auto& rec : sorted) {
        if (rec.key.size() > std::numeric_limits<std::uint16_t>::max()) {
            return std::unexpected(
                Error{ErrorCode::InvalidArgument,
                      "encodeMetadataPayload: metadata key longer than 65535 bytes"});
        }
        if (rec.value.size() > std::numeric_limits<std::uint32_t>::max()) {
            return std::unexpected(
                Error{ErrorCode::InvalidArgument,
                      "encodeMetadataPayload: metadata value longer than 4 GiB"});
        }
        writeU16(out, static_cast<std::uint16_t>(rec.key.size()));
        appendAscii(out, rec.key);
        writeU32(out, static_cast<std::uint32_t>(rec.value.size()));
        appendAscii(out, rec.value);
    }
    return out;
}

Result<std::vector<MetadataRecord>>
decodeMetadataPayload(const std::vector<std::uint64_t>& byteValues) {
    // Converge the reader's `vector<uint64_t>` byte values into a `span<const byte>`.
    std::vector<std::byte> bytes;
    bytes.reserve(byteValues.size());
    for (const auto v : byteValues) {
        if (v > 0xFF) {
            return std::unexpected(
                Error{ErrorCode::InvalidArgument,
                      "decodeMetadataPayload: non-byte value in PTIFF extension tag"});
        }
        bytes.push_back(static_cast<std::byte>(v & 0xFF));
    }
    const std::span<const std::byte> data{bytes};

    if (!hasMagic(data)) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument,
                  "decodeMetadataPayload: missing PTIFF magic in extension tag payload"});
    }
    std::size_t cursor = kPtiffMagic.size();
    if (data.size() < cursor + 2) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument,
                  "decodeMetadataPayload: truncated PTIFF extension payload header"});
    }
    const auto version = readU16(data.subspan(cursor, 2));
    cursor += 2;
    if (version > kPtiffMetadataVersion) {
        // A payload from a future writer: do not mis-parse. Return "unsupported version" so the
        // caller can choose to treat the tag as absent (it must not fail the whole file).
        return std::unexpected(
            Error{ErrorCode::InvalidArgument,
                  "decodeMetadataPayload: unsupported PTIFF extension payload version"});
    }
    if (data.size() < cursor + 4) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument,
                  "decodeMetadataPayload: truncated PTIFF extension payload header"});
    }
    const auto recCount = readU32(data.subspan(cursor, 4));
    cursor += 4;

    std::vector<MetadataRecord> records;
    records.reserve(recCount);
    for (std::uint32_t i = 0; i < recCount; ++i) {
        if (data.size() < cursor + 2) {
            return std::unexpected(
                Error{ErrorCode::InvalidArgument,
                      "decodeMetadataPayload: truncated PTIFF extension record key length"});
        }
        const auto keyLen = readU16(data.subspan(cursor, 2));
        cursor += 2;
        if (data.size() < cursor + keyLen) {
            return std::unexpected(
                Error{ErrorCode::InvalidArgument,
                      "decodeMetadataPayload: truncated PTIFF extension record key"});
        }
        const std::string key = asciiOf(data.subspan(cursor, keyLen));
        cursor += keyLen;

        if (data.size() < cursor + 4) {
            return std::unexpected(
                Error{ErrorCode::InvalidArgument,
                      "decodeMetadataPayload: truncated PTIFF extension record value length"});
        }
        const auto valLen = readU32(data.subspan(cursor, 4));
        cursor += 4;
        if (data.size() < cursor + valLen) {
            return std::unexpected(
                Error{ErrorCode::InvalidArgument,
                      "decodeMetadataPayload: truncated PTIFF extension record value"});
        }
        const std::string value = asciiOf(data.subspan(cursor, valLen));
        cursor += valLen;

        records.push_back(MetadataRecord{std::move(key), std::move(value)});
    }
    return records;
}

std::vector<MetadataRecord> recordsFromStorageModel(const StorageModel& node,
                                                    std::string_view prefix) {
    // StorageModel fields are already stored in a std::map (ascending key order). Collect any
    // field whose key starts with `ptiff.<prefix>.` and strip that prefix. The read path writes
    // with the same prefix so the roundtrip is symmetric.
    const std::string pfx = "ptiff." + std::string(prefix) + ".";
    std::vector<MetadataRecord> records;
    node.for_each_field([&](std::string_view key, std::string_view value) {
        if (key.size() > pfx.size() && key.substr(0, pfx.size()) == pfx) {
            records.push_back(
                MetadataRecord{std::string{key.substr(pfx.size())}, std::string{value}});
        }
    });
    // recordsFromStorageModel preserves the stable ascending key order from the StorageModel map.
    return records;
}

} // namespace ptiff::io::backend::tiff
