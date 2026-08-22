#include <functional>
#include <memory>
#include <string>
#include <vector>

#include <ptiff/io/backend_factory.hpp>
#include <ptiff/io/detail/scene_storage_adapter.hpp>
#include <ptiff/io/file_binary_writer.hpp>
#include <ptiff/io/image_sink.hpp>
#include <ptiff/io/scene_serializer.hpp>
#include <ptiff/io/storage_backend.hpp>
#include <ptiff/io/storage_model.hpp>
#include <ptiff/io/tile/tile.hpp>
#include <ptiff/io/tile/tile_index.hpp>
#include <ptiff/io/writer.hpp>
#include <ptiff/scene.hpp>

namespace ptiff {

namespace {

// The Scene Serializer/Deserializer shape is a root model with one child per image, which is
// exactly the shape the multi-image TIFF backend write path takes. Serialize the scene and
// flatten its children into one flat StorageModel per image, in scene order.
[[nodiscard]] Result<std::vector<io::StorageModel>> serializeSceneForBackend(const Scene& scene) {
    io::SceneSerializer serializer;
    auto model = serializer.serialize(scene);
    if (!model.has_value()) {
        return std::unexpected(model.error());
    }
    auto flattenResult = io::detail::flattenChildren(*model);
    if (!flattenResult.has_value()) {
        return std::unexpected(flattenResult.error());
    }
    return std::move(*flattenResult);
}

// Internal facade: a real Writer backed by a StorageBackend + the Scene Serializer.
class FileWriter final : public Writer {
public:
    explicit FileWriter(std::unique_ptr<io::FileBinaryWriter> bytes_in,
                        std::unique_ptr<io::StorageBackend> backend_in)
        : bytes_(std::move(bytes_in)), backend_(std::move(backend_in)) {}

    [[nodiscard]] Result<void> write(const Scene& scene) override {
        auto flat = serializeSceneForBackend(scene);
        if (!flat.has_value()) {
            return std::unexpected(flat.error());
        }
        auto writeResult = backend_->serializeModelList(*flat, *bytes_);
        if (!writeResult.has_value()) {
            return std::unexpected(writeResult.error());
        }

        // The underlying FileBinaryWriter buffers its ofstream; make the bytes visible to a
        // reader that opens the same path before this Writer is destroyed.
        return bytes_->flush();
    }

    [[nodiscard]] Result<void> write(const Scene& scene, io::TileProvider& provider) override {
        std::vector<std::reference_wrapper<io::TileProvider>> providers{std::ref(provider)};
        return write(scene, providers);
    }

    [[nodiscard]] Result<void>
    write(const Scene& scene,
          const std::vector<std::reference_wrapper<io::TileProvider>>& providers) override {
        auto count = scene.imageCount();
        if (!count.has_value()) {
            return std::unexpected(count.error());
        }
        if (providers.size() != *count) {
            return std::unexpected(Error{ErrorCode::InvalidArgument,
                                         "Writer: provider count does not match the scene's "
                                         "image count"});
        }

        auto flat = serializeSceneForBackend(scene);
        if (!flat.has_value()) {
            return std::unexpected(flat.error());
        }

        // Write the header + full IFD chain once, then feed each image's data region.
        auto headResult = backend_->serializeModelList(*flat, *bytes_);
        if (!headResult.has_value()) {
            return std::unexpected(headResult.error());
        }

        for (std::size_t i = 0; i < flat->size(); ++i) {
            auto sinkResult = backend_->openImageSinkAt(*bytes_, *flat, i);
            if (!sinkResult.has_value()) {
                return std::unexpected(sinkResult.error());
            }
            io::ImageSink& sink = **sinkResult;
            io::TileProvider& provider = providers[i].get();

            // The provider's tiling must describe the same image the backend derives from the
            // scene's storage metadata; reject a mismatch before writing any pixels.
            if (provider.layout() != sink.layout()) {
                return std::unexpected(Error{ErrorCode::InvalidArgument,
                                             "Writer: provider layout does not match the "
                                             "backend's derived image layout"});
            }

            const io::tile::TileLayout& layout = sink.layout();
            for (std::uint32_t level = 0; level < layout.levelCount; ++level) {
                for (std::uint32_t row = 0; row < layout.rows(level); ++row) {
                    for (std::uint32_t col = 0; col < layout.columns(level); ++col) {
                        const io::tile::TileIndex index{.column = col, .row = row, .level = level};
                        auto tileResult = provider.provideTile(index);
                        if (!tileResult.has_value()) {
                            return std::unexpected(tileResult.error());
                        }
                        auto tileWriteResult = sink.writeTile(*tileResult);
                        if (!tileWriteResult.has_value()) {
                            return std::unexpected(tileWriteResult.error());
                        }
                    }
                }
            }
        }

        return bytes_->flush();
    }

private:
    std::unique_ptr<io::FileBinaryWriter> bytes_;
    std::unique_ptr<io::StorageBackend> backend_;
};

} // namespace

Result<std::unique_ptr<Writer>> Writer::create(std::string_view path) {
    auto bytes = io::FileBinaryWriter::create(std::string{path});
    if (!bytes.has_value()) {
        return std::unexpected(bytes.error());
    }

    auto backend = io::BackendFactory::instance().create("tiff");
    if (!backend.has_value()) {
        return std::unexpected(backend.error());
    }

    return std::unique_ptr<Writer>(new FileWriter(std::move(*bytes), std::move(*backend)));
}

} // namespace ptiff
