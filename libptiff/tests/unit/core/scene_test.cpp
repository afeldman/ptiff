#include <cstdint>
#include <limits>
#include <type_traits>

#include <ptiff/image.hpp>
#include <ptiff/image/image_descriptor.hpp>
#include <ptiff/scene.hpp>

#include <catch2/catch_test_macros.hpp>

using ptiff::Image;
using ptiff::ImageDescriptor;
using ptiff::ImageId;
using ptiff::Scene;

namespace {
ImageDescriptor grayscale(std::uint32_t w, std::uint32_t h) {
    ImageDescriptor d;
    d.width = w;
    d.height = h;
    d.pixelType = ptiff::PixelType::UInt8;
    d.channelCount = 1;
    return d;
}
} // namespace

static_assert(!std::is_copy_constructible_v<Scene>);
static_assert(!std::is_copy_assignable_v<Scene>);
static_assert(std::is_move_constructible_v<Scene>);
static_assert(std::is_move_assignable_v<Scene>);

TEST_CASE("empty scene reports zero images", "[scene]") {
    Scene scene;
    REQUIRE(scene.imageCount().has_value());
    REQUIRE(*scene.imageCount() == 0);
}

TEST_CASE("addImage assigns increasing ids and imageCount tracks it", "[scene]") {
    Scene scene;
    auto id0 = scene.addImage(grayscale(64, 32));
    auto id1 = scene.addImage(grayscale(16, 16));
    REQUIRE(id0.has_value());
    REQUIRE(id1.has_value());
    REQUIRE(*id0 == ImageId{0});
    REQUIRE(*id1 == ImageId{1});
    REQUIRE(*scene.imageCount() == 2);
}

TEST_CASE("image(id) returns the right image; unknown id is NotFound", "[scene]") {
    Scene scene;
    auto id = scene.addImage(grayscale(10, 20));
    REQUIRE(id.has_value());

    auto img = scene.image(*id);
    REQUIRE(img.has_value());
    REQUIRE(img->get().width() == 10);
    REQUIRE(img->get().height() == 20);
    REQUIRE(img->get().pixelType() == ptiff::PixelType::UInt8);

    auto missing = scene.image(ImageId{999u});
    REQUIRE_FALSE(missing.has_value());
    REQUIRE(missing.error().code() == ptiff::ErrorCode::NotFound);
}

TEST_CASE("imageAt(index) returns images in scene order; out of range is OutOfRange", "[scene]") {
    Scene scene;
    REQUIRE(scene.addImage(grayscale(1, 1)).has_value());
    REQUIRE(scene.addImage(grayscale(2, 2)).has_value());

    auto first = scene.imageAt(0);
    REQUIRE(first.has_value());
    REQUIRE(first->get().width() == 1);

    auto second = scene.imageAt(1);
    REQUIRE(second.has_value());
    REQUIRE(second->get().width() == 2);

    auto oob = scene.imageAt(2);
    REQUIRE_FALSE(oob.has_value());
    REQUIRE(oob.error().code() == ptiff::ErrorCode::OutOfRange);
}

TEST_CASE("move preserves images and empties source", "[scene]") {
    Scene a;
    REQUIRE(a.addImage(grayscale(5, 5)).has_value());
    Scene b = std::move(a);
    REQUIRE(*b.imageCount() == 1);
    REQUIRE(b.imageAt(0).has_value());
    REQUIRE(*a.imageCount() == 0);
}
