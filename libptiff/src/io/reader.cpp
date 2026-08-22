#include <memory>
#include <string>

#include <ptiff/io/backend_factory.hpp>
#include <ptiff/io/file_binary_reader.hpp>
#include <ptiff/io/http_range_binary_reader.hpp>
#include <ptiff/io/reader.hpp>
#include <ptiff/io/scene_deserializer.hpp>
#include <ptiff/io/storage_backend.hpp>
#include <ptiff/io/storage_model.hpp>
#include <ptiff/scene.hpp>

namespace ptiff {

namespace {

// Internal facade: a real Reader backed by a StorageBackend + the Scene Deserializer.
class FileReader final : public Reader {
public:
    FileReader(std::unique_ptr<io::BinaryReader> bytes_in,
               std::unique_ptr<io::StorageBackend> backend_in,
               Scene scene_in)
        : bytes_(std::move(bytes_in)),
          backend_(std::move(backend_in)),
          scene_(std::move(scene_in)) {}

    [[nodiscard]] Result<const Scene*> scene() const override { return &scene_; }

    [[nodiscard]] Result<std::unique_ptr<io::ImageSource>> imageSource(ImageId imageId) override {
        // The shared binary reader was left positioned past the header by deserializeModel
        // during open(); readDirectoryChain re-reads the header from the reader's current
        // position, so rewind to the file start before handing it to openImageSourceAt.
        auto seekResult = bytes_->seek(0);
        if (!seekResult.has_value()) {
            return std::unexpected(seekResult.error());
        }
        return backend_->openImageSourceAt(*bytes_, imageId.value());
    }

private:
    std::unique_ptr<io::BinaryReader> bytes_;
    std::unique_ptr<io::StorageBackend> backend_;
    Scene scene_;
};

} // namespace

Result<std::unique_ptr<Reader>> Reader::open(std::string_view path, std::string_view bearerToken) {
    std::unique_ptr<io::BinaryReader> bytes;
    if (path.starts_with("http://") || path.starts_with("https://")) {
        auto httpBytes = io::HttpRangeBinaryReader::open(std::string{path}, bearerToken);
        if (!httpBytes.has_value()) {
            return std::unexpected(httpBytes.error());
        }
        bytes = std::move(*httpBytes);
    } else {
        auto fileBytes = io::FileBinaryReader::open(std::string{path});
        if (!fileBytes.has_value()) {
            return std::unexpected(fileBytes.error());
        }
        bytes = std::move(*fileBytes);
    }

    auto backend = io::BackendFactory::instance().create("tiff");
    if (!backend.has_value()) {
        return std::unexpected(backend.error());
    }

    auto model = (*backend)->deserializeModel(*bytes);
    if (!model.has_value()) {
        return std::unexpected(model.error());
    }

    // The TIFF backend's deserializeModel produces a root model with one child per image (one
    // per IFD in the chain), which is exactly the shape the Scene Deserializer expects -- no
    // shape bridging needed.
    io::SceneDeserializer deserializer;
    auto scene = deserializer.deserialize(*model);
    if (!scene.has_value()) {
        return std::unexpected(scene.error());
    }

    return std::unique_ptr<Reader>(
        new FileReader(std::move(bytes), std::move(*backend), std::move(*scene)));
}

} // namespace ptiff
