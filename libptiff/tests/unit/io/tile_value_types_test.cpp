#include <ptiff/core/id.hpp>
#include <ptiff/io/tile/tile_extent.hpp>
#include <ptiff/io/tile/tile_index.hpp>
#include <ptiff/io/tile/tile_region.hpp>

#include <catch2/catch_test_macros.hpp>

TEST_CASE("TileId is a distinct strong type", "[tile]") {
    ptiff::TileId a{1};
    ptiff::TileId b{2};

    REQUIRE(a == ptiff::TileId{1});
    REQUIRE_FALSE(a == b);
    REQUIRE(a.value() == 1);
}

TEST_CASE("TileIndex equality is field-wise", "[tile]") {
    ptiff::io::tile::TileIndex first{.column = 1, .row = 2, .level = 0};
    ptiff::io::tile::TileIndex same{.column = 1, .row = 2, .level = 0};
    ptiff::io::tile::TileIndex different{.column = 1, .row = 2, .level = 1};

    REQUIRE(first == same);
    REQUIRE_FALSE(first == different);
}

TEST_CASE("TileRegion composes TileExtent", "[tile]") {
    ptiff::io::tile::TileRegion region{.x = 256, .y = 512, .extent = {.width = 256, .height = 256}};

    REQUIRE(region.x == 256);
    REQUIRE(region.extent.width == 256);
    REQUIRE(region == region);
}
