#include <memory>

#include <ptiff/io/backend/openexr/openexr_document.hpp>
#include <ptiff/io/backend/openexr/openexr_image_sink.hpp>
#include <ptiff/io/backend/openexr/openexr_image_source.hpp>
#include <ptiff/io/backend/openexr_backend.hpp>
#include <ptiff/io/backend_factory.hpp>

namespace ptiff::io::backend {

namespace {

// The Writer facade passes either a flat per-image model directly, or (via serializeModel on a
// full scene root) a root model whose first child carries the image fields. Normalize both to
// the flat image model used by openexr::imageInfoFromModel.
const StorageModel* imageModel(const StorageModel& model) {
    return model.children().empty() ? &model : &model.children().front();
}

} // namespace

std::string_view OpenExrBackend::name() const noexcept {
    return "openexr";
}

BackendCapabilities OpenExrBackend::capabilities() const noexcept {
    BackendCapabilities caps;
    // The whole image is always one tile this phase (see openexr/openexr_document.hpp), so this
    // backend does not advertise real tile-by-tile serving; the document is otherwise seekable.
    caps.supportsRandomAccess = true;
    return caps;
}

Result<std::unique_ptr<ImageSource>> OpenExrBackend::openImageSource(BinaryReader& reader) const {
    auto info = openexr::readHeaderInfo(reader);
    if (!info.has_value()) {
        return std::unexpected(info.error());
    }
    return std::make_unique<openexr::OpenExrImageSource>(reader, *info);
}

Result<std::unique_ptr<ImageSink>> OpenExrBackend::openImageSink(BinaryWriter& writer,
                                                                 const StorageModel& model) const {
    const StorageModel* img = imageModel(model);
    auto info = openexr::imageInfoFromModel(*img);
    if (!info.has_value()) {
        return std::unexpected(info.error());
    }
    return std::make_unique<openexr::OpenExrImageSink>(writer, *info);
}

Result<StorageModel> OpenExrBackend::deserializeModel(BinaryReader& reader) const {
    auto info = openexr::readHeaderInfo(reader);
    if (!info.has_value()) {
        return std::unexpected(info.error());
    }
    // Scene convention: root model with one child (this image).
    StorageModel root;
    root.addChild(openexr::modelFromImageInfo(*info));
    return root;
}

Result<void> OpenExrBackend::serializeModel(const StorageModel& model, BinaryWriter& writer) const {
    const StorageModel* img = imageModel(model);
    return openexr::writeHeaderOnly(*img, writer);
}

namespace {
const bool registered = [] {
    return BackendFactory::instance()
        .registerBackend("openexr", [] { return std::make_unique<OpenExrBackend>(); })
        .has_value();
}();
} // namespace

} // namespace ptiff::io::backend
