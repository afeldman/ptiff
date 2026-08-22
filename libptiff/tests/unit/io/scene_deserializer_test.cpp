#include <ptiff/image.hpp>
#include <ptiff/io/scene_deserializer.hpp>
#include <ptiff/io/storage_model.hpp>
#include <ptiff/scene.hpp>

#include <catch2/catch_test_macros.hpp>

using ptiff::CompressionKind;
using ptiff::PixelType;
using ptiff::Scene;
using ptiff::io::SceneDeserializer;
using ptiff::io::StorageModel;

namespace {
StorageModel imageNode(
    const char* w, const char* h, const char* spp, const char* pt, const char* comp = "None") {
    StorageModel m;
    m.setField("imageWidth", w);
    m.setField("imageHeight", h);
    m.setField("samplesPerPixel", spp);
    m.setField("pixelType", pt);
    m.setField("compression", comp);
    return m;
}
} // namespace

TEST_CASE("SceneDeserializer rebuilds a scene with fields and order intact",
          "[scene-deserializer]") {
    StorageModel root;
    root.addChild(imageNode("64", "32", "1", "UInt8"));
    root.addChild(imageNode("128", "64", "3", "Float32"));

    SceneDeserializer deser;
    auto scene = deser.deserialize(root);
    REQUIRE(scene.has_value());
    REQUIRE(*scene->imageCount() == 2);

    auto first = scene->imageAt(0);
    REQUIRE(first.has_value());
    REQUIRE(first->get().width() == 64);
    REQUIRE(first->get().height() == 32);
    REQUIRE(first->get().channelCount() == 1);
    REQUIRE(first->get().pixelType() == PixelType::UInt8);
    REQUIRE(first->get().compression().has_value());
    REQUIRE(*first->get().compression() == CompressionKind::None);

    auto second = scene->imageAt(1);
    REQUIRE(second.has_value());
    REQUIRE(second->get().width() == 128);
    REQUIRE(second->get().height() == 64);
    REQUIRE(second->get().channelCount() == 3);
    REQUIRE(second->get().pixelType() == PixelType::Float32);
}

TEST_CASE("SceneDeserializer accepts both LZW and Lzw spelling for compression",
          "[scene-deserializer]") {
    {
        StorageModel root;
        root.addChild(imageNode("10", "10", "1", "UInt8", "LZW"));
        SceneDeserializer deser;
        auto scene = deser.deserialize(root);
        REQUIRE(scene.has_value());
        auto img = scene->imageAt(0);
        REQUIRE(img.has_value());
        REQUIRE(*img->get().compression() == CompressionKind::Lzw);
    }
    {
        StorageModel root;
        root.addChild(imageNode("10", "10", "1", "UInt8", "Lzw")); // read-side spelling
        SceneDeserializer deser;
        auto scene = deser.deserialize(root);
        REQUIRE(scene.has_value());
        auto img = scene->imageAt(0);
        REQUIRE(img.has_value());
        REQUIRE(*img->get().compression() == CompressionKind::Lzw);
    }
}

TEST_CASE("SceneDeserializer rejects unknown compression and missing required fields",
          "[scene-deserializer]") {
    {
        StorageModel root;
        root.addChild(imageNode("10", "10", "1", "UInt8", "Bogus"));
        SceneDeserializer deser;
        auto scene = deser.deserialize(root);
        REQUIRE_FALSE(scene.has_value());
        REQUIRE(scene.error().code() == ptiff::ErrorCode::InvalidArgument);
    }
    {
        StorageModel m;
        m.setField("imageWidth", "10"); // missing imageHeight
        StorageModel root;
        root.addChild(std::move(m));
        SceneDeserializer deser;
        auto scene = deser.deserialize(root);
        REQUIRE_FALSE(scene.has_value());
        REQUIRE(scene.error().code() == ptiff::ErrorCode::InvalidArgument);
    }
}

TEST_CASE("SceneDeserializer on empty model produces empty scene", "[scene-deserializer]") {
    StorageModel root;
    SceneDeserializer deser;
    auto scene = deser.deserialize(root);
    REQUIRE(scene.has_value());
    REQUIRE(*scene->imageCount() == 0);
}
