#pragma once

// In-memory ("PMEM") StorageModel tree codec -- internal helper, not installed.
//
// Encodes/decodes the recursive StorageModel tree of the Memory backend's binary format (see
// the memory-backend design spec, decision D1a): each node is a u32le field count followed by
// that many (u32le keyLen, key, u32le valLen, val) records and then a u32le child count followed
// by the recursive child nodes. Values are always strings, matching the StorageModel API.
//
// The tree is encoded in ascending key order (StorageModel::for_each_field is stable and
// sorted), so memoryCodecByteSize / memoryEncode always agree on the emitted byte length; the
// decode side rebuilds a StorageModel, which re-sorts fields on insertion, so round trips are
// stable regardless of input order.

#include <cstddef>
#include <cstdint>
#include <span>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#include <ptiff/core/result.hpp>
#include <ptiff/io/binary_reader.hpp>
#include <ptiff/io/storage_model.hpp>

namespace ptiff::io::backend::memory {

// Hard caps against resource exhaustion from hostile metadata (RFC-0001 §13), analogous to the
// TIFF parser-hardening's kMaxTagCount.
inline constexpr std::uint64_t kMaxNodes = 1'000'000;
inline constexpr std::uint64_t kMaxFieldCount = 1'000'000;

/// @brief Returns the exact byte length `memoryEncode` would write for \p node's subtree.
///
/// Pure length function over the same fields/children that @ref memoryEncode serializes, so the
/// two are always consistent (used by the offset planning in the backend).
[[nodiscard]] inline std::uint64_t memoryCodecByteSize(const io::StorageModel& node) {
    std::uint64_t total = 4; // field count
    node.for_each_field([&](std::string_view key, std::string_view value) {
        total += 4 + key.size() + 4 + value.size();
    });
    total += 4; // child count
    for (const auto& child : node.children()) {
        total += memoryCodecByteSize(child);
    }
    return total;
}

namespace detail {

inline void appendU32le(std::vector<std::byte>& out, std::uint32_t value) {
    out.push_back(static_cast<std::byte>((value >> 0) & 0xFFu));
    out.push_back(static_cast<std::byte>((value >> 8) & 0xFFu));
    out.push_back(static_cast<std::byte>((value >> 16) & 0xFFu));
    out.push_back(static_cast<std::byte>((value >> 24) & 0xFFu));
}

} // namespace detail

/// @brief Encodes \p node's subtree into \p out (a byte buffer the caller owns and will later
///        hand to a BinaryWriter), bounded by a shared node budget against runaway trees.
///
/// @param out        Byte buffer to append the encoding to.
/// @param node       The subtree to encode.
/// @param nodeBudget In/out counter of nodes encoded so far; starts at 0 for the root and is
///                   incremented per node. If it exceeds `kMaxNodes` → InvalidArgument.
/// @return `Result<void>` success on success, or `ErrorCode::InvalidArgument` if the tree
///         exceeds `kMaxNodes` or a node has more than `kMaxFieldCount` fields.
[[nodiscard]] inline Result<void>
memoryEncode(std::vector<std::byte>& out, const io::StorageModel& node, std::uint64_t& nodeBudget) {
    if (++nodeBudget > kMaxNodes) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "memory: model tree exceeds kMaxNodes"});
    }

    std::uint64_t fieldCount = 0;
    node.for_each_field([&](std::string_view, std::string_view) { ++fieldCount; });
    if (fieldCount > kMaxFieldCount) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "memory: model node exceeds kMaxFieldCount"});
    }

    detail::appendU32le(out, static_cast<std::uint32_t>(fieldCount));
    node.for_each_field([&](std::string_view key, std::string_view value) {
        detail::appendU32le(out, static_cast<std::uint32_t>(key.size()));
        out.insert(out.end(),
                   reinterpret_cast<const std::byte*>(key.data()),
                   reinterpret_cast<const std::byte*>(key.data()) + key.size());
        detail::appendU32le(out, static_cast<std::uint32_t>(value.size()));
        out.insert(out.end(),
                   reinterpret_cast<const std::byte*>(value.data()),
                   reinterpret_cast<const std::byte*>(value.data()) + value.size());
    });

    detail::appendU32le(out, static_cast<std::uint32_t>(node.children().size()));
    for (const auto& child : node.children()) {
        auto result = memoryEncode(out, child, nodeBudget);
        if (!result.has_value()) {
            return result;
        }
    }
    return {};
}

/// @brief Returns the byte length @ref memoryEncodeDocument writes for a document whose images
///        are the sibling models \p models (a fieldless root wrapping them).
[[nodiscard]] inline std::uint64_t
memoryDocumentByteSize(std::span<const io::StorageModel> models) {
    std::uint64_t total = 4 + 4; // fieldless-root header: field count + child count
    for (const auto& model : models) {
        total += memoryCodecByteSize(model);
    }
    return total;
}

/// @brief Encodes a document whose image set is the sibling models \p models (a fieldless root
///        wrapping them), appending to \p out. Mirrors `memoryEncode` on a root with \p models
///        as its children, but without requiring ownership of the models (they may be const).
[[nodiscard]] inline Result<void> memoryEncodeDocument(std::vector<std::byte>& out,
                                                       std::span<const io::StorageModel> models,
                                                       std::uint64_t& nodeBudget) {
    if (++nodeBudget > kMaxNodes) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "memory: model tree exceeds kMaxNodes"});
    }
    detail::appendU32le(out, 0);                                         // root field count
    detail::appendU32le(out, static_cast<std::uint32_t>(models.size())); // root child count
    for (const auto& model : models) {
        auto result = memoryEncode(out, model, nodeBudget);
        if (!result.has_value()) {
            return result;
        }
    }
    return {};
}

namespace detail {

[[nodiscard]] inline Result<std::uint32_t> readU32le(const std::vector<std::byte>& bytes,
                                                     std::size_t& cursor) {
    if (cursor + 4 > bytes.size()) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "memory: truncated u32 in model header"});
    }
    const auto at = static_cast<std::size_t>(cursor);
    const std::uint32_t value =
        static_cast<std::uint32_t>((std::to_integer<unsigned int>(bytes[at + 0]) << 0) |
                                   (std::to_integer<unsigned int>(bytes[at + 1]) << 8) |
                                   (std::to_integer<unsigned int>(bytes[at + 2]) << 16) |
                                   (std::to_integer<unsigned int>(bytes[at + 3]) << 24));
    cursor += 4;
    return value;
}

[[nodiscard]] inline Result<std::string> readString(const std::vector<std::byte>& bytes,
                                                    std::size_t& cursor) {
    auto len = readU32le(bytes, cursor);
    if (!len.has_value()) {
        return std::unexpected(len.error());
    }
    if (cursor + static_cast<std::size_t>(*len) > bytes.size()) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "memory: truncated string in model header"});
    }
    const char* begin = reinterpret_cast<const char*>(bytes.data() + cursor);
    std::string result{begin, begin + *len};
    cursor += static_cast<std::size_t>(*len);
    return result;
}

} // namespace detail

/// @brief Decodes one subtree of the model header from \p bytes at \p cursor.
///
/// @param bytes      The model-header bytes produced by @ref memoryEncode.
/// @param cursor     In/out byte offset; starts at the beginning of the subtree.
/// @param nodeBudget In/out node counter (for the `kMaxNodes` cap).
/// @return The decoded @ref io::StorageModel "StorageModel" on success, or
///         `ErrorCode::InvalidArgument` on a truncated/hostile header.
[[nodiscard]] inline Result<io::StorageModel>
memoryDecode(const std::vector<std::byte>& bytes, std::size_t& cursor, std::uint64_t& nodeBudget) {
    if (++nodeBudget > kMaxNodes) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "memory: model tree exceeds kMaxNodes"});
    }

    auto fieldCount = detail::readU32le(bytes, cursor);
    if (!fieldCount.has_value()) {
        return std::unexpected(fieldCount.error());
    }
    if (*fieldCount > kMaxFieldCount) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "memory: model node exceeds kMaxFieldCount"});
    }

    io::StorageModel node;
    for (std::uint32_t i = 0; i < *fieldCount; ++i) {
        auto key = detail::readString(bytes, cursor);
        if (!key.has_value()) {
            return std::unexpected(key.error());
        }
        auto value = detail::readString(bytes, cursor);
        if (!value.has_value()) {
            return std::unexpected(value.error());
        }
        node.setField(*key, std::move(*value));
    }

    auto childCount = detail::readU32le(bytes, cursor);
    if (!childCount.has_value()) {
        return std::unexpected(childCount.error());
    }
    for (std::uint32_t i = 0; i < *childCount; ++i) {
        auto child = memoryDecode(bytes, cursor, nodeBudget);
        if (!child.has_value()) {
            return std::unexpected(child.error());
        }
        node.addChild(std::move(*child));
    }
    return node;
}

} // namespace ptiff::io::backend::memory
