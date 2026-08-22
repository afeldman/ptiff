#include <array>
#include <cstddef>
#include <span>
#include <type_traits>

#include <ptiff/io/image_sink.hpp>
#include <ptiff/io/image_source.hpp>

#include <catch2/catch_test_macros.hpp>

namespace {

using ptiff::io::tile::Tile;
using ptiff::io::tile::TileIndex;
using ptiff::io::tile::TileLayout;

class FakeImageSource final : public ptiff::io::ImageSource {
public:
    explicit FakeImageSource(TileLayout layout) : layout_(layout) {}

    [[nodiscard]] const TileLayout& layout() const noexcept override { return layout_; }
    [[nodiscard]] ptiff::Result<Tile> readTile(const TileIndex&) override {
        return std::unexpected(ptiff::Error{ptiff::ErrorCode::NotImplemented, "not implemented"});
    }

private:
    TileLayout layout_;
};

class FakeImageSink final : public ptiff::io::ImageSink {
public:
    explicit FakeImageSink(TileLayout layout) : layout_(layout) {}

    [[nodiscard]] const TileLayout& layout() const noexcept override { return layout_; }
    [[nodiscard]] ptiff::Result<void> writeTile(const Tile&) override {
        return std::unexpected(ptiff::Error{ptiff::ErrorCode::NotImplemented, "not implemented"});
    }

private:
    TileLayout layout_;
};

static_assert(!std::is_copy_constructible_v<ptiff::io::ImageSource>);
static_assert(!std::is_move_constructible_v<ptiff::io::ImageSource>);
static_assert(!std::is_copy_constructible_v<ptiff::io::ImageSink>);
static_assert(!std::is_move_constructible_v<ptiff::io::ImageSink>);

} // namespace

TEST_CASE("ImageSource exposes its layout and stub tile reads", "[image-source-sink]") {
    TileLayout layout{
        .tileSize = {.width = 64, .height = 64}, .imageWidth = 128, .imageHeight = 128};
    FakeImageSource source{layout};

    REQUIRE(source.layout().imageWidth == 128);
    REQUIRE_FALSE(source.readTile(TileIndex{}).has_value());
}

TEST_CASE("ImageSink exposes its layout and stub tile writes", "[image-source-sink]") {
    TileLayout layout{
        .tileSize = {.width = 64, .height = 64}, .imageWidth = 128, .imageHeight = 128};
    FakeImageSink sink{layout};
    std::array<std::byte, 1> byte{std::byte{0}};
    Tile tile{ptiff::TileId{1}, TileIndex{}, {}, std::span<const std::byte>{byte}};

    REQUIRE(sink.layout().imageHeight == 128);
    REQUIRE_FALSE(sink.writeTile(tile).has_value());
}
