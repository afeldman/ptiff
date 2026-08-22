#include <cstdint>
#include <string>

#include <ptiff/image.hpp>
#include <ptiff/image/image_descriptor.hpp>
#include <ptiff/io/scene_serializer.hpp>

namespace ptiff::io {

namespace {
constexpr const char* kPixelTypeValue(PixelType t) noexcept {
    switch (t) {
    case PixelType::UInt8:
        return "UInt8";
    case PixelType::UInt16:
        return "UInt16";
    case PixelType::UInt32:
        return "UInt32";
    case PixelType::Float32:
        return "Float32";
    case PixelType::Float64:
        return nullptr;
    }
    return nullptr;
}
} // namespace

Result<StorageModel> SceneSerializer::serialize(const Scene& scene) const {
    StorageModel root;

    auto count = scene.imageCount();
    if (!count.has_value()) {
        return std::unexpected(count.error());
    }

    for (std::size_t i = 0; i < *count; ++i) {
        auto imgResult = scene.imageAt(i);
        if (!imgResult.has_value()) {
            return std::unexpected(imgResult.error());
        }
        const Image& image = imgResult->get();

        StorageModel child;
        child.setField("imageWidth", std::to_string(image.width()));
        child.setField("imageHeight", std::to_string(image.height()));

        const std::uint32_t channels = image.channelCount();
        if (channels != 1 && channels != 3) {
            return std::unexpected(
                Error{ErrorCode::InvalidArgument, "SceneSerializer: unsupported samplesPerPixel"});
        }
        child.setField("samplesPerPixel", std::to_string(channels));

        const char* pixelTypeValue = kPixelTypeValue(image.pixelType());
        if (pixelTypeValue == nullptr) {
            return std::unexpected(Error{ErrorCode::InvalidArgument,
                                         "SceneSerializer: unsupported pixelType Float64"});
        }
        child.setField("pixelType", pixelTypeValue);

        if (auto compression = image.compression(); compression.has_value()) {
            if (*compression == CompressionKind::Jpeg && image.pixelType() != PixelType::UInt8) {
                return std::unexpected(
                    Error{ErrorCode::InvalidArgument,
                          "SceneSerializer: Jpeg compression requires UInt8 pixelType"});
            }
            switch (*compression) {
            case CompressionKind::None:
                child.setField("compression", "None");
                break;
            case CompressionKind::Lzw:
                child.setField("compression", "LZW");
                break;
            case CompressionKind::Deflate:
                child.setField("compression", "Deflate");
                break;
            case CompressionKind::Jpeg:
                child.setField("compression", "Jpeg");
                break;
            }
        } else {
            child.setField("compression", "None");
        }

        if (auto tileInfo = image.tileInfo(); tileInfo.has_value()) {
            child.setField("tileWidth", std::to_string(tileInfo->tileWidth));
            child.setField("tileHeight", std::to_string(tileInfo->tileHeight));
        }

        root.addChild(std::move(child));
    }

    return root;
}

} // namespace ptiff::io
