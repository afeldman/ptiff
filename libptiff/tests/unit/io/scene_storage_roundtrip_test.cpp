#include <ptiff/image.hpp>
#include <ptiff/image/image_descriptor.hpp>
#include <ptiff/io/scene_deserializer.hpp>
#include <ptiff/io/scene_serializer.hpp>
#include <ptiff/io/storage_model.hpp>
#include <ptiff/scene.hpp>

#include <catch2/catch_test_macros.hpp>

using ptiff::CompressionKind;
using ptiff::ImageDescriptor;
using ptiff::PixelType;
using ptiff::Scene;
using ptiff::io::SceneDeserializer;
using ptiff::io::SceneSerializer;

namespace {
Scene buildScene() {
    Scene scene;
    ImageDescriptor gs;
    gs.width = 640;
    gs.height = 480;
    gs.pixelType = PixelType::UInt8;
    gs.channelCount = 1;
    gs.compression = CompressionKind::Lzw;
    (void)scene.addImage(gs);

    ImageDescriptor rgb;
    rgb.width = 32;
    rgb.height = 32;
    rgb.pixelType = PixelType::Float32;
    rgb.channelCount = 3;
    (void)scene.addImage(rgb);
    return scene;
}
} // namespace

TEST_CASE("Scene survives serialize->deserialize round trip for supported fields",
          "[scene-roundtrip]") {
    Scene original = buildScene();

    SceneSerializer ser;
    auto model = ser.serialize(original);
    REQUIRE(model.has_value());

    SceneDeserializer deser;
    auto restored = deser.deserialize(*model);
    REQUIRE(restored.has_value());
    REQUIRE(*restored->imageCount() == 2);

    auto img0 = restored->imageAt(0);
    REQUIRE(img0.has_value());
    REQUIRE(img0->get().width() == 640);
    REQUIRE(img0->get().height() == 480);
    REQUIRE(img0->get().pixelType() == PixelType::UInt8);
    REQUIRE(img0->get().channelCount() == 1);
    REQUIRE(img0->get().compression().has_value());
    REQUIRE(*img0->get().compression() == CompressionKind::Lzw);

    auto img1 = restored->imageAt(1);
    REQUIRE(img1.has_value());
    REQUIRE(img1->get().width() == 32);
    REQUIRE(img1->get().height() == 32);
    REQUIRE(img1->get().pixelType() == PixelType::Float32);
    REQUIRE(img1->get().channelCount() == 3);
}
