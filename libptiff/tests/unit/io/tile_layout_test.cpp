#include <ptiff/image/image_descriptor.hpp>
#include <ptiff/io/tile/tile_layout.hpp>

#include <catch2/catch_test_macros.hpp>

using ptiff::io::tile::TileExtent;
using ptiff::io::tile::TileIndex;
using ptiff::io::tile::TileLayout;

TEST_CASE("TileLayout computes grid dimensions", "[tile-layout]") {
    TileLayout layout{
        .tileSize = {.width = 256, .height = 256}, .imageWidth = 1000, .imageHeight = 600};

    REQUIRE(layout.columns() == 4);
    REQUIRE(layout.rows() == 3);
}

TEST_CASE("TileLayout::regionFor returns the covered rectangle", "[tile-layout]") {
    TileLayout layout{
        .tileSize = {.width = 256, .height = 256}, .imageWidth = 1000, .imageHeight = 600};

    auto result = layout.regionFor(TileIndex{.column = 1, .row = 0, .level = 0});
    REQUIRE(result.has_value());
    REQUIRE(result->x == 256);
    REQUIRE(result->y == 0);
    REQUIRE(result->extent.width == 256);
    REQUIRE(result->extent.height == 256);
}

TEST_CASE("TileLayout::regionFor rejects an out-of-range index", "[tile-layout]") {
    TileLayout layout{
        .tileSize = {.width = 256, .height = 256}, .imageWidth = 1000, .imageHeight = 600};

    auto result = layout.regionFor(TileIndex{.column = 4, .row = 0, .level = 0});
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::OutOfRange);
}

TEST_CASE("TileLayout::indexFor maps a pixel back to its tile", "[tile-layout]") {
    TileLayout layout{
        .tileSize = {.width = 256, .height = 256}, .imageWidth = 1000, .imageHeight = 600};

    auto result = layout.indexFor(300, 10);
    REQUIRE(result.has_value());
    REQUIRE(result->column == 1);
    REQUIRE(result->row == 0);
}

TEST_CASE("TileLayout::regionFor rejects an out-of-range level", "[tile-layout]") {
    TileLayout layout{
        .tileSize = {.width = 256, .height = 256}, .imageWidth = 1000, .imageHeight = 600};

    auto result = layout.regionFor(TileIndex{.column = 0, .row = 0, .level = 1});
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::OutOfRange);
}

TEST_CASE("TileLayout::indexFor rejects an out-of-range level", "[tile-layout]") {
    TileLayout layout{
        .tileSize = {.width = 256, .height = 256}, .imageWidth = 1000, .imageHeight = 600};

    auto result = layout.indexFor(0, 0, 1);
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::OutOfRange);
}

TEST_CASE("TileLayout supports queries at levels beyond the base", "[tile-layout]") {
    TileLayout layout{.tileSize = {.width = 256, .height = 256},
                      .imageWidth = 1000,
                      .imageHeight = 600,
                      .levelCount = 2};

    auto region = layout.regionFor(TileIndex{.column = 0, .row = 0, .level = 1});
    REQUIRE(region.has_value());
    REQUIRE(region->x == 0);
    REQUIRE(region->y == 0);
    REQUIRE(region->extent.width == 256);
    REQUIRE(region->extent.height == 256);

    auto index = layout.indexFor(300, 10, 1);
    REQUIRE(index.has_value());
    REQUIRE(index->column == 1);
    REQUIRE(index->row == 0);
    REQUIRE(index->level == 1);
}

TEST_CASE("TileLayout::fromDescriptor requires tileInfo", "[tile-layout]") {
    ptiff::ImageDescriptor withoutTiles{.width = 100, .height = 100};
    auto missing = TileLayout::fromDescriptor(withoutTiles);
    REQUIRE_FALSE(missing.has_value());
    REQUIRE(missing.error().code() == ptiff::ErrorCode::InvalidArgument);

    ptiff::ImageDescriptor withTiles{.width = 100,
                                     .height = 100,
                                     .tileInfo =
                                         ptiff::TileInfo{.tileWidth = 32, .tileHeight = 32}};
    auto layout = TileLayout::fromDescriptor(withTiles);
    REQUIRE(layout.has_value());
    REQUIRE(layout->tileSize.width == 32);
    REQUIRE(layout->imageWidth == 100);
}
