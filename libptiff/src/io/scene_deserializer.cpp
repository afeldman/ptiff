#include <cstddef>
#include <cstdint>
#include <limits>
#include <stdexcept>
#include <string>

#include <ptiff/image/image_descriptor.hpp>
#include <ptiff/io/scene_deserializer.hpp>
#include <ptiff/scene.hpp>

namespace ptiff::io {

namespace {
Result<std::uint32_t> parseU32(const StorageModel& node, const char* key) {
    auto field = node.field(key);
    if (!field.has_value()) {
        return std::unexpected(Error{ErrorCode::InvalidArgument,
                                     "SceneDeserializer: missing \"" + std::string{key} + "\""});
    }
    try {
        std::size_t pos = 0;
        auto value = std::stoul(*field, &pos, 10);
        if (pos != field->size()) {
            return std::unexpected(
                Error{ErrorCode::InvalidArgument,
                      "SceneDeserializer: non-numeric \"" + std::string{key} + "\""});
        }
        if (value > std::numeric_limits<std::uint32_t>::max()) {
            return std::unexpected(
                Error{ErrorCode::InvalidArgument,
                      "SceneDeserializer: \"" + std::string{key} + "\" out of range"});
        }
        return static_cast<std::uint32_t>(value);
    } catch (const std::exception&) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument,
                  "SceneDeserializer: non-numeric \"" + std::string{key} + "\""});
    }
}

Result<PixelType> parsePixelType(const std::string& value) {
    if (value == "UInt8") {
        return PixelType::UInt8;
    }
    if (value == "UInt16") {
        return PixelType::UInt16;
    }
    if (value == "UInt32") {
        return PixelType::UInt32;
    }
    if (value == "Float32") {
        return PixelType::Float32;
    }
    if (value == "Float64") {
        return PixelType::Float64;
    }
    return std::unexpected(
        Error{ErrorCode::InvalidArgument, "SceneDeserializer: unrecognized pixelType"});
}

Result<CompressionKind> parseCompression(const std::string& value) {
    if (value == "None") {
        return CompressionKind::None;
    }
    if (value == "LZW" || value == "Lzw") {
        return CompressionKind::Lzw;
    }
    return std::unexpected(
        Error{ErrorCode::InvalidArgument, "SceneDeserializer: unrecognized compression"});
}
} // namespace

Result<Scene> SceneDeserializer::deserialize(const StorageModel& model) const {
    Scene scene;
    for (const auto& node : model.children()) {
        auto width = parseU32(node, "imageWidth");
        if (!width.has_value()) {
            return std::unexpected(width.error());
        }
        auto height = parseU32(node, "imageHeight");
        if (!height.has_value()) {
            return std::unexpected(height.error());
        }
        auto spp = parseU32(node, "samplesPerPixel");
        if (!spp.has_value()) {
            return std::unexpected(spp.error());
        }

        auto pixelTypeField = node.field("pixelType");
        if (!pixelTypeField.has_value()) {
            return std::unexpected(
                Error{ErrorCode::InvalidArgument, "SceneDeserializer: missing \"pixelType\""});
        }
        auto pixelType = parsePixelType(*pixelTypeField);
        if (!pixelType.has_value()) {
            return std::unexpected(pixelType.error());
        }

        ImageDescriptor d;
        d.width = *width;
        d.height = *height;
        d.channelCount = *spp;
        d.pixelType = *pixelType;

        if (auto compField = node.field("compression"); compField.has_value()) {
            auto comp = parseCompression(*compField);
            if (!comp.has_value()) {
                return std::unexpected(comp.error());
            }
            d.compression = *comp;
        } else {
            d.compression = CompressionKind::None;
        }

        auto result = scene.addImage(d);
        if (!result.has_value()) {
            return std::unexpected(result.error());
        }
    }
    return scene;
}

} // namespace ptiff::io
