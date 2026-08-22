#include <memory>
#include <span>

#include <ptiff/io/backend/isis/isis_document.hpp>
#include <ptiff/io/backend/isis/isis_label.hpp>
#include <ptiff/io/backend/isis_backend.hpp>
#include <ptiff/io/backend/memory/memory_image_sink.hpp>
#include <ptiff/io/backend/memory/memory_image_source.hpp>
#include <ptiff/io/backend/memory/memory_layout.hpp>
#include <ptiff/io/backend_factory.hpp>

namespace ptiff::io::backend {

namespace {

// The Writer facade passes either a flat per-image model directly, or a root model whose first
// child carries the image fields. Normalize to the flat image model used by writeLabel /
// imageInfoFromModel.
const StorageModel* imageModel(const StorageModel& model) {
    return model.children().empty() ? &model : &model.children().front();
}

} // namespace

std::string_view IsisBackend::name() const noexcept {
    return "isis";
}

BackendCapabilities IsisBackend::capabilities() const noexcept {
    BackendCapabilities caps;
    caps.supportsTiling = true;
    caps.supportsRandomAccess = true;
    return caps;
}

Result<std::unique_ptr<ImageSource>> IsisBackend::openImageSource(BinaryReader& reader) const {
    auto doc = isis::readDocument(reader);
    if (!doc.has_value()) {
        return std::unexpected(doc.error());
    }
    auto info = memory::imageInfoFromModel(doc->image);
    if (!info.has_value()) {
        return std::unexpected(info.error());
    }
    const std::uint64_t pixelStart = isis::pixelOrigin(doc->labelBytes);
    return std::make_unique<memory::MemoryImageSource>(reader, std::move(*info), pixelStart);
}

Result<std::unique_ptr<ImageSink>> IsisBackend::openImageSink(BinaryWriter& writer,
                                                              const StorageModel& model) const {
    const StorageModel* img = imageModel(model);
    auto info = memory::imageInfoFromModel(*img);
    if (!info.has_value()) {
        return std::unexpected(info.error());
    }
    auto label = isis::writeLabel(*img);
    if (!label.has_value()) {
        return std::unexpected(label.error());
    }
    const std::uint64_t pixelStart = isis::pixelOrigin(label->size());
    auto seekResult = writer.seek(pixelStart);
    if (!seekResult.has_value()) {
        return std::unexpected(seekResult.error());
    }
    return std::make_unique<memory::MemoryImageSink>(writer, std::move(*info), pixelStart);
}

Result<StorageModel> IsisBackend::deserializeModel(BinaryReader& reader) const {
    auto doc = isis::readDocument(reader);
    if (!doc.has_value()) {
        return std::unexpected(doc.error());
    }
    StorageModel root;
    root.addChild(std::move(doc->image));
    return root;
}

Result<void> IsisBackend::serializeModel(const StorageModel& model, BinaryWriter& writer) const {
    const StorageModel* img = imageModel(model);
    auto label = isis::writeLabel(*img);
    if (!label.has_value()) {
        return std::unexpected(label.error());
    }
    return writer.write(std::span<const std::byte>{*label})
        .and_then([&](std::size_t written) -> Result<void> {
            if (written != label->size()) {
                return std::unexpected(
                    Error{ErrorCode::InvalidArgument, "isis: short label write"});
            }
            return {};
        });
}

namespace {
const bool registered = [] {
    return BackendFactory::instance()
        .registerBackend("isis", [] { return std::make_unique<IsisBackend>(); })
        .has_value();
}();
} // namespace

} // namespace ptiff::io::backend
