#include <memory>
#include <span>

#include <ptiff/io/backend/zarr/zarr_document.hpp>
#include <ptiff/io/backend/zarr/zarr_image_sink.hpp>
#include <ptiff/io/backend/zarr/zarr_image_source.hpp>
#include <ptiff/io/backend/zarr_backend.hpp>
#include <ptiff/io/backend_factory.hpp>

namespace ptiff::io::backend {

namespace {

// The Writer facade passes either a flat per-image model directly, or a root model whose first
// child carries the image fields. Normalize to the flat image model used by buildLayout.
const StorageModel* imageModel(const StorageModel& model) {
    return model.children().empty() ? &model : &model.children().front();
}

} // namespace

std::string_view ZarrBackend::name() const noexcept {
    return "zarr";
}

BackendCapabilities ZarrBackend::capabilities() const noexcept {
    BackendCapabilities caps;
    caps.supportsTiling = true;
    caps.supportsStreaming = true;
    caps.supportsRandomAccess = false; // chunk slots are implicitly indexed by linear order
    return caps;
}

Result<std::unique_ptr<ImageSource>> ZarrBackend::openImageSource(BinaryReader& reader) const {
    auto doc = zarr::readDocument(reader);
    if (!doc.has_value()) {
        return std::unexpected(doc.error());
    }
    const std::uint64_t pixelRegion = zarr::kHeaderSize + doc->header.size();
    return std::make_unique<zarr::ZarrImageSource>(reader, std::move(*doc), pixelRegion);
}

Result<std::unique_ptr<ImageSink>> ZarrBackend::openImageSink(BinaryWriter& writer,
                                                              const StorageModel& model) const {
    const StorageModel* img = imageModel(model);
    auto layoutResult = zarr::buildLayout(*img);
    if (!layoutResult.has_value()) {
        return std::unexpected(layoutResult.error());
    }
    const std::uint64_t pixelRegion = zarr::kHeaderSize + layoutResult->header.size();
    auto seekResult = writer.seek(pixelRegion);
    if (!seekResult.has_value()) {
        return std::unexpected(seekResult.error());
    }
    return std::make_unique<zarr::ZarrImageSink>(writer, std::move(*layoutResult), pixelRegion);
}

Result<StorageModel> ZarrBackend::deserializeModel(BinaryReader& reader) const {
    auto doc = zarr::readDocument(reader);
    if (!doc.has_value()) {
        return std::unexpected(doc.error());
    }
    StorageModel root;
    root.addChild(std::move(doc->image));
    return root;
}

Result<void> ZarrBackend::serializeModel(const StorageModel& model, BinaryWriter& writer) const {
    const StorageModel* img = imageModel(model);
    auto layoutResult = zarr::buildLayout(*img);
    if (!layoutResult.has_value()) {
        return std::unexpected(layoutResult.error());
    }
    if (layoutResult->header.size() > (1ull << 24)) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "zarr: header too large to encode"});
    }

    std::vector<std::byte> headerSizeBytes(zarr::kHeaderSize);
    std::uint64_t len = layoutResult->header.size();
    for (std::size_t i = 0; i < zarr::kHeaderSize; ++i) {
        headerSizeBytes[i] = static_cast<std::byte>((len >> (8 * i)) & 0xFF);
    }
    auto hw = writer.write(std::span<const std::byte>{headerSizeBytes});
    if (!hw.has_value()) {
        return std::unexpected(hw.error());
    }
    return writer.write(std::span<const std::byte>{layoutResult->header})
        .and_then([&](std::size_t written) -> Result<void> {
            if (written != layoutResult->header.size()) {
                return std::unexpected(
                    Error{ErrorCode::InvalidArgument, "zarr: short header write"});
            }
            return {};
        });
}

namespace {
const bool registered = [] {
    return BackendFactory::instance()
        .registerBackend("zarr", [] { return std::make_unique<ZarrBackend>(); })
        .has_value();
}();
} // namespace

} // namespace ptiff::io::backend
