#include <cstdint>
#include <memory>
#include <span>
#include <vector>

#include <ptiff/io/backend/memory/memory_image_sink.hpp>
#include <ptiff/io/backend/memory/memory_image_source.hpp>
#include <ptiff/io/backend/memory/memory_layout.hpp>
#include <ptiff/io/backend/pds4/pds4_document.hpp>
#include <ptiff/io/backend/pds4/pds4_label.hpp>
#include <ptiff/io/backend/pds4_backend.hpp>
#include <ptiff/io/backend_factory.hpp>

namespace ptiff::io::backend {

namespace {

// The Writer facade passes either a flat per-image model directly, or (via serializeModel on a
// full scene root) a root model whose first child carries the image fields. Normalize both to
// the flat image model used by writeLabel / imageInfoFromModel.
const StorageModel* imageModel(const StorageModel& model) {
    return model.children().empty() ? &model : &model.children().front();
}

} // namespace

std::string_view Pds4Backend::name() const noexcept {
    return "pds4";
}

BackendCapabilities Pds4Backend::capabilities() const noexcept {
    BackendCapabilities caps;
    // PDS4 labels are seekable (label first, pixel block after) and the pixel block is
    // tiled/uncompressed like the memory backend.
    caps.supportsTiling = true;
    caps.supportsRandomAccess = true;
    return caps;
}

Result<std::unique_ptr<ImageSource>> Pds4Backend::openImageSource(BinaryReader& reader) const {
    auto doc = pds4::readDocument(reader);
    if (!doc.has_value()) {
        return std::unexpected(doc.error());
    }
    auto info = memory::imageInfoFromModel(doc->image);
    if (!info.has_value()) {
        return std::unexpected(info.error());
    }
    const std::uint64_t pixelStart = pds4::pixelOrigin(doc->labelBytes);
    return std::make_unique<memory::MemoryImageSource>(reader, std::move(*info), pixelStart);
}

Result<std::unique_ptr<ImageSink>> Pds4Backend::openImageSink(BinaryWriter& writer,
                                                              const StorageModel& model) const {
    const StorageModel* img = imageModel(model);
    auto info = memory::imageInfoFromModel(*img);
    if (!info.has_value()) {
        return std::unexpected(info.error());
    }
    auto label = pds4::writeLabel(*img);
    if (!label.has_value()) {
        return std::unexpected(label.error());
    }
    const std::uint64_t pixelStart = pds4::pixelOrigin(label->size());
    auto seekResult = writer.seek(pixelStart);
    if (!seekResult.has_value()) {
        return std::unexpected(seekResult.error());
    }
    return std::make_unique<memory::MemoryImageSink>(writer, std::move(*info), pixelStart);
}

Result<StorageModel> Pds4Backend::deserializeModel(BinaryReader& reader) const {
    auto doc = pds4::readDocument(reader);
    if (!doc.has_value()) {
        return std::unexpected(doc.error());
    }
    // Scene convention: root model with one child (this image).
    StorageModel root;
    root.addChild(std::move(doc->image));
    return root;
}

Result<void> Pds4Backend::serializeModel(const StorageModel& model, BinaryWriter& writer) const {
    const StorageModel* img = imageModel(model);
    auto label = pds4::writeLabel(*img);
    if (!label.has_value()) {
        return std::unexpected(label.error());
    }
    if (label->size() > (1ull << 24)) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "pds4: label too large to encode"});
    }

    // Seek header (label byte length, LE).
    std::vector<std::byte> header(pds4::kHeaderSize);
    std::uint64_t len = label->size();
    for (std::size_t i = 0; i < pds4::kHeaderSize; ++i) {
        header[i] = static_cast<std::byte>((len >> (8 * i)) & 0xFF);
    }
    auto hw = writer.write(std::span<const std::byte>{header});
    if (!hw.has_value()) {
        return std::unexpected(hw.error());
    }
    return writer.write(std::span<const std::byte>{*label})
        .and_then([&](std::size_t written) -> Result<void> {
            if (written != label->size()) {
                return std::unexpected(
                    Error{ErrorCode::InvalidArgument, "pds4: short label write"});
            }
            return {};
        });
}

namespace {
const bool registered = [] {
    return BackendFactory::instance()
        .registerBackend("pds4", [] { return std::make_unique<Pds4Backend>(); })
        .has_value();
}();
} // namespace

} // namespace ptiff::io::backend
