#include <ptiff/image.hpp>
#include <ptiff/image/image_descriptor.hpp>
#include <ptiff/io/scene_serializer.hpp>
#include <ptiff/io/storage_model.hpp>
#include <ptiff/scene.hpp>

#include <catch2/catch_test_macros.hpp>

using ptiff::CompressionKind;
using ptiff::ImageDescriptor;
using ptiff::PixelType;
using ptiff::Scene;
using ptiff::io::SceneSerializer;
using ptiff::io::StorageModel;

namespace {
ImageDescriptor
descriptor(PixelType t, std::uint32_t channels, std::uint32_t w = 64, std::uint32_t h = 32) {
    ImageDescriptor d;
    d.width = w;
    d.height = h;
    d.pixelType = t;
    d.channelCount = channels;
    return d;
}
} // namespace

TEST_CASE("SceneSerializer emits one child per scene image with exact field values",
          "[scene-serializer]") {
    Scene scene;
    REQUIRE(scene.addImage(descriptor(PixelType::UInt8, 1)).has_value());
    REQUIRE(scene.addImage(descriptor(PixelType::Float32, 3, 128, 64)).has_value());

    SceneSerializer ser;
    auto model = ser.serialize(scene);
    REQUIRE(model.has_value());
    REQUIRE(model->children().size() == 2);

    const auto& first = model->children()[0];
    REQUIRE((first.field("imageWidth").has_value() && *first.field("imageWidth") == "64"));
    REQUIRE((first.field("imageHeight").has_value() && *first.field("imageHeight") == "32"));
    REQUIRE((first.field("samplesPerPixel").has_value() && *first.field("samplesPerPixel") == "1"));
    REQUIRE((first.field("pixelType").has_value() && *first.field("pixelType") == "UInt8"));
    REQUIRE((first.field("compression").has_value() && *first.field("compression") == "None"));

    const auto& second = model->children()[1];
    REQUIRE(*second.field("imageWidth") == "128");
    REQUIRE(*second.field("imageHeight") == "64");
    REQUIRE(*second.field("samplesPerPixel") == "3");
    REQUIRE(*second.field("pixelType") == "Float32");
}

TEST_CASE("SceneSerializer maps compression None->None and Lzw->LZW uppercase",
          "[scene-serializer]") {
    Scene scene;
    auto d = descriptor(PixelType::UInt16, 1);
    d.compression = CompressionKind::Lzw;
    REQUIRE(scene.addImage(d).has_value());

    SceneSerializer ser;
    auto model = ser.serialize(scene);
    REQUIRE(model.has_value());
    REQUIRE(*model->children()[0].field("compression") == "LZW"); // uppercase, not "Lzw"
}

TEST_CASE("SceneSerializer emits tileWidth/tileHeight when tileInfo present",
          "[scene-serializer]") {
    Scene scene;
    auto d = descriptor(PixelType::UInt8, 1);
    d.tileInfo = ptiff::TileInfo{16, 16};
    REQUIRE(scene.addImage(d).has_value());

    SceneSerializer ser;
    auto model = ser.serialize(scene);
    REQUIRE(model.has_value());
    REQUIRE(*model->children()[0].field("tileWidth") == "16");
    REQUIRE(*model->children()[0].field("tileHeight") == "16");
}

TEST_CASE("SceneSerializer rejects Float64, Jpeg+non-UInt8, and unsupported channelCount",
          "[scene-serializer]") {
    {
        Scene scene;
        REQUIRE(scene.addImage(descriptor(PixelType::Float64, 1)).has_value());
        SceneSerializer ser;
        auto m = ser.serialize(scene);
        REQUIRE_FALSE(m.has_value());
        REQUIRE(m.error().code() == ptiff::ErrorCode::InvalidArgument);
    }
    {
        Scene scene;
        auto d = descriptor(PixelType::UInt16, 1);
        d.compression = CompressionKind::Jpeg; // Jpeg requires UInt8, not UInt16
        REQUIRE(scene.addImage(d).has_value());
        SceneSerializer ser;
        auto m = ser.serialize(scene);
        REQUIRE_FALSE(m.has_value());
        REQUIRE(m.error().code() == ptiff::ErrorCode::InvalidArgument);
    }
    {
        Scene scene;
        REQUIRE(
            scene.addImage(descriptor(PixelType::UInt8, 2)).has_value()); // 2 samples unsupported
        SceneSerializer ser;
        auto m = ser.serialize(scene);
        REQUIRE_FALSE(m.has_value());
        REQUIRE(m.error().code() == ptiff::ErrorCode::InvalidArgument);
    }
}

TEST_CASE("SceneSerializer maps Deflate->Deflate and Jpeg->Jpeg for UInt8 images",
          "[scene-serializer]") {
    {
        Scene scene;
        auto d = descriptor(PixelType::UInt8, 1);
        d.compression = CompressionKind::Deflate;
        REQUIRE(scene.addImage(d).has_value());
        SceneSerializer ser;
        auto m = ser.serialize(scene);
        REQUIRE(m.has_value());
        REQUIRE(*m->children()[0].field("compression") == "Deflate");
    }
    {
        Scene scene;
        auto d = descriptor(PixelType::UInt8, 3);
        d.compression = CompressionKind::Jpeg;
        REQUIRE(scene.addImage(d).has_value());
        SceneSerializer ser;
        auto m = ser.serialize(scene);
        REQUIRE(m.has_value());
        REQUIRE(*m->children()[0].field("compression") == "Jpeg");
    }
}

TEST_CASE("SceneSerializer on empty scene emits root with no children", "[scene-serializer]") {
    Scene scene;
    SceneSerializer ser;
    auto model = ser.serialize(scene);
    REQUIRE(model.has_value());
    REQUIRE(model->children().empty());
}
