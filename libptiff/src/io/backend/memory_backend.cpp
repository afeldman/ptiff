#include <array>
#include <cstddef>
#include <cstdint>
#include <utility>
#include <vector>

#include <ptiff/io/backend/memory/memory_codec.hpp>
#include <ptiff/io/backend/memory/memory_image_sink.hpp>
#include <ptiff/io/backend/memory/memory_image_source.hpp>
#include <ptiff/io/backend/memory/memory_layout.hpp>
#include <ptiff/io/backend/memory_backend.hpp>
#include <ptiff/io/backend_factory.hpp>
#include <ptiff/io/binary_writer.hpp>

namespace ptiff::io::backend {

namespace {

// In-memory ("PMEM") binary header. Layout of byte 0 on:
//   [0..4)  magic "P","M","E","M" (0x50 0x4D 0x45 0x4D)
//   [4..6)  version, little-endian u16 == 1
//   [6..8)  flags, little-endian u16 == 0 (reserved)
//   [8..12) model-header byte length M (little-endian u32)
//   [12..)  model header (see the codec); then the contiguous per-image pixel regions.
constexpr std::array<std::byte, 4> kMagic{
    std::byte{0x50}, std::byte{0x4D}, std::byte{0x45}, std::byte{0x4D}};
constexpr std::uint16_t kVersion = 1;
constexpr std::size_t kHeaderSize = 12;

[[nodiscard]] Result<void> readFully(BinaryReader& reader, std::span<std::byte> destination) {
    std::size_t offset = 0;
    while (offset < destination.size()) {
        auto got = reader.read(destination.subspan(offset));
        if (!got.has_value()) {
            return std::unexpected(got.error());
        }
        if (*got == 0) {
            return std::unexpected(Error{ErrorCode::InvalidArgument, "memory: truncated document"});
        }
        offset += *got;
    }
    return {};
}

[[nodiscard]] Result<std::uint16_t> readU16le(const std::array<std::byte, kHeaderSize>& h,
                                              std::size_t at) {
    return static_cast<std::uint16_t>((std::to_integer<unsigned int>(h[at + 0]) << 0) |
                                      (std::to_integer<unsigned int>(h[at + 1]) << 8));
}

[[nodiscard]] Result<std::uint32_t> readU32le(const std::array<std::byte, kHeaderSize>& h,
                                              std::size_t at) {
    return static_cast<std::uint32_t>((std::to_integer<unsigned int>(h[at + 0]) << 0) |
                                      (std::to_integer<unsigned int>(h[at + 1]) << 8) |
                                      (std::to_integer<unsigned int>(h[at + 2]) << 16) |
                                      (std::to_integer<unsigned int>(h[at + 3]) << 24));
}

/// @brief Writes a little-endian u16 to \p writer at the cursor.
[[nodiscard]] Result<void> writeU16le(BinaryWriter& writer, std::uint16_t value) {
    const std::byte raw[2]{static_cast<std::byte>((value >> 0) & 0xFFu),
                           static_cast<std::byte>((value >> 8) & 0xFFu)};
    return writer.write(std::span<const std::byte>{raw, 2})
        .and_then([](std::size_t) -> Result<void> { return {}; });
}

/// @brief Writes a little-endian u32 to \p writer at the cursor.
[[nodiscard]] Result<void> writeU32le(BinaryWriter& writer, std::uint32_t value) {
    const std::byte raw[4]{static_cast<std::byte>((value >> 0) & 0xFFu),
                           static_cast<std::byte>((value >> 8) & 0xFFu),
                           static_cast<std::byte>((value >> 16) & 0xFFu),
                           static_cast<std::byte>((value >> 24) & 0xFFu)};
    return writer.write(std::span<const std::byte>{raw, 4})
        .and_then([](std::size_t) -> Result<void> { return {}; });
}

/// @brief Parses the 12-byte PMEM header from \p reader (cursor moved to 0), validates it, and
///        reports the length of the model header that follows.
struct DocumentHeader {
    std::uint32_t modelLen = 0;
};

[[nodiscard]] Result<DocumentHeader> readDocumentHeader(BinaryReader& reader) {
    auto seekResult = reader.seek(0);
    if (!seekResult.has_value()) {
        return std::unexpected(seekResult.error());
    }
    auto size = reader.size();
    if (!size.has_value()) {
        return std::unexpected(size.error());
    }
    std::array<std::byte, kHeaderSize> header{};
    auto readResult = readFully(reader, std::span<std::byte>{header});
    if (!readResult.has_value()) {
        return std::unexpected(readResult.error());
    }
    for (std::size_t i = 0; i < kMagic.size(); ++i) {
        if (header[i] != kMagic[i]) {
            return std::unexpected(Error{ErrorCode::InvalidArgument, "memory: bad magic"});
        }
    }
    auto version = readU16le(header, 4);
    if (version != kVersion) {
        return std::unexpected(Error{ErrorCode::InvalidArgument, "memory: unsupported version"});
    }
    auto modelLen = readU32le(header, 8);
    if (static_cast<std::uint64_t>(*modelLen) + kHeaderSize > *size) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "memory: model header exceeds file size"});
    }
    return DocumentHeader{*modelLen};
}

/// @brief Reads and decodes the whole document (header + model tree) from \p reader.
struct ParsedDocument {
    io::StorageModel root;
    std::uint64_t docPrefix = 0; // absolute offset where pixel regions begin
};

[[nodiscard]] Result<ParsedDocument> readDocument(BinaryReader& reader) {
    auto header = readDocumentHeader(reader);
    if (!header.has_value()) {
        return std::unexpected(header.error());
    }
    std::vector<std::byte> model(static_cast<std::size_t>(header->modelLen));
    auto readResult = readFully(reader, std::span<std::byte>{model});
    if (!readResult.has_value()) {
        return std::unexpected(readResult.error());
    }
    std::size_t cursor = 0;
    std::uint64_t nodeBudget = 0;
    auto root = memory::memoryDecode(model, cursor, nodeBudget);
    if (!root.has_value()) {
        return std::unexpected(root.error());
    }
    return ParsedDocument{std::move(*root),
                          static_cast<std::uint64_t>(kHeaderSize) + header->modelLen};
}

/// @brief Writes the 12-byte PMEM header then \p modelBytes (the encoded model tree) at cursor 0.
[[nodiscard]] Result<void> writeHeaderAndModel(BinaryWriter& writer,
                                               std::span<const std::byte> modelBytes) {
    auto seekResult = writer.seek(0);
    if (!seekResult.has_value()) {
        return std::unexpected(seekResult.error());
    }
    auto magicResult = writer.write(kMagic);
    if (!magicResult.has_value()) {
        return std::unexpected(magicResult.error());
    }
    auto versionResult = writeU16le(writer, kVersion);
    if (!versionResult.has_value()) {
        return std::unexpected(versionResult.error());
    }
    auto flagsResult = writeU16le(writer, 0);
    if (!flagsResult.has_value()) {
        return std::unexpected(flagsResult.error());
    }
    auto lenResult = writeU32le(writer, static_cast<std::uint32_t>(modelBytes.size()));
    if (!lenResult.has_value()) {
        return std::unexpected(lenResult.error());
    }
    return writer.write(modelBytes).and_then([](std::size_t) -> Result<void> { return {}; });
}

/// @brief Derives per-image storage info for a set of flat image models.
[[nodiscard]] Result<std::vector<memory::MemoryImageInfo>>
imageInfos(std::span<const io::StorageModel> models) {
    std::vector<memory::MemoryImageInfo> infos;
    infos.reserve(models.size());
    for (const auto& model : models) {
        auto info = memory::imageInfoFromModel(model);
        if (!info.has_value()) {
            return std::unexpected(info.error());
        }
        infos.push_back(std::move(*info));
    }
    return infos;
}

} // namespace

std::string_view MemoryBackend::name() const noexcept {
    return "memory";
}

BackendCapabilities MemoryBackend::capabilities() const noexcept {
    return {.supportsTiling = true,
            .supportsStreaming = false,
            .supportsRandomAccess = true,
            .supportsCloudStreaming = false};
}

Result<std::unique_ptr<ImageSource>> MemoryBackend::openImageSource(BinaryReader& reader) const {
    return openImageSourceAt(reader, 0);
}

Result<std::unique_ptr<ImageSink>> MemoryBackend::openImageSink(BinaryWriter& writer,
                                                                const StorageModel& model) const {
    auto children = model.children();
    if (children.empty()) {
        return std::unexpected(Error{ErrorCode::InvalidArgument,
                                     "memory: openImageSink requires a model with images"});
    }
    auto docPrefix = static_cast<std::uint64_t>(kHeaderSize) + memory::memoryCodecByteSize(model);
    auto info = memory::imageInfoFromModel(children.front());
    if (!info.has_value()) {
        return std::unexpected(info.error());
    }
    auto seekResult = writer.seek(docPrefix);
    if (!seekResult.has_value()) {
        return std::unexpected(seekResult.error());
    }
    return std::make_unique<memory::MemoryImageSink>(writer, std::move(*info), docPrefix);
}

Result<StorageModel> MemoryBackend::deserializeModel(BinaryReader& reader) const {
    auto doc = readDocument(reader);
    if (!doc.has_value()) {
        return std::unexpected(doc.error());
    }
    return std::move(doc->root);
}

Result<void> MemoryBackend::serializeModel(const StorageModel& model, BinaryWriter& writer) const {
    std::vector<std::byte> bytes;
    std::uint64_t nodeBudget = 0;
    auto encodeResult = memory::memoryEncode(bytes, model, nodeBudget);
    if (!encodeResult.has_value()) {
        return std::unexpected(encodeResult.error());
    }
    return writeHeaderAndModel(writer, bytes);
}

Result<void> MemoryBackend::serializeModelList(std::span<const StorageModel> models,
                                               BinaryWriter& writer) const {
    std::vector<std::byte> bytes;
    std::uint64_t nodeBudget = 0;
    auto encodeResult = memory::memoryEncodeDocument(bytes, models, nodeBudget);
    if (!encodeResult.has_value()) {
        return std::unexpected(encodeResult.error());
    }
    return writeHeaderAndModel(writer, bytes);
}

Result<std::unique_ptr<ImageSink>> MemoryBackend::openImageSinkAt(
    BinaryWriter& writer, std::span<const StorageModel> models, std::size_t imageIndex) const {
    if (imageIndex >= models.size()) {
        return std::unexpected(Error{ErrorCode::OutOfRange,
                                     "MemoryBackend::openImageSinkAt: image index out of range"});
    }
    auto infos = imageInfos(models);
    if (!infos.has_value()) {
        return std::unexpected(infos.error());
    }
    const std::uint64_t docPrefix =
        static_cast<std::uint64_t>(kHeaderSize) + memory::memoryDocumentByteSize(models);
    const std::uint64_t pixelStart = memory::imagePixelOffset(*infos, imageIndex, docPrefix);
    auto seekResult = writer.seek(pixelStart);
    if (!seekResult.has_value()) {
        return std::unexpected(seekResult.error());
    }
    return std::make_unique<memory::MemoryImageSink>(
        writer, std::move((*infos)[imageIndex]), pixelStart);
}

Result<std::unique_ptr<ImageSource>>
MemoryBackend::openImageSourceAt(BinaryReader& reader, std::size_t imageIndex) const {
    auto doc = readDocument(reader);
    if (!doc.has_value()) {
        return std::unexpected(doc.error());
    }
    auto children = doc->root.children();
    if (imageIndex >= children.size()) {
        return std::unexpected(Error{ErrorCode::OutOfRange,
                                     "MemoryBackend::openImageSourceAt: image index out of range"});
    }
    auto infos = imageInfos(children);
    if (!infos.has_value()) {
        return std::unexpected(infos.error());
    }
    const std::uint64_t pixelStart = memory::imagePixelOffset(*infos, imageIndex, doc->docPrefix);
    return std::make_unique<memory::MemoryImageSource>(
        reader, std::move((*infos)[imageIndex]), pixelStart);
}

namespace {
const bool registered = [] {
    return BackendFactory::instance()
        .registerBackend("memory", [] { return std::make_unique<MemoryBackend>(); })
        .has_value();
}();
} // namespace

} // namespace ptiff::io::backend
