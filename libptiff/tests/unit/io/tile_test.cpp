#include <array>
#include <cstddef>

#include <ptiff/io/tile/tile.hpp>

#include <catch2/catch_test_macros.hpp>

TEST_CASE("Tile exposes identity, position, and a non-owning data view", "[tile]") {
    std::array<std::byte, 4> bytes{std::byte{1}, std::byte{2}, std::byte{3}, std::byte{4}};

    ptiff::io::tile::Tile tile{
        ptiff::TileId{7},
        ptiff::io::tile::TileIndex{.column = 1, .row = 0, .level = 0},
        ptiff::io::tile::TileRegion{.x = 256, .y = 0, .extent = {.width = 256, .height = 256}},
        std::span<const std::byte>{bytes}};

    REQUIRE(tile.id() == ptiff::TileId{7});
    REQUIRE(tile.index().column == 1);
    REQUIRE(tile.region().x == 256);
    REQUIRE(tile.data().size() == 4);
    REQUIRE(tile.data()[0] == std::byte{1});
}

TEST_CASE("Tile is cheap to copy (span, not owning buffer)", "[tile]") {
    std::array<std::byte, 2> bytes{std::byte{9}, std::byte{9}};
    ptiff::io::tile::Tile original{ptiff::TileId{1},
                                   ptiff::io::tile::TileIndex{},
                                   ptiff::io::tile::TileRegion{},
                                   std::span<const std::byte>{bytes}};

    ptiff::io::tile::Tile copy = original;
    REQUIRE(copy.data().data() == original.data().data());
}
