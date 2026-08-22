#include <array>
#include <cstddef>
#include <cstdint>
#include <optional>
#include <span>
#include <type_traits>
#include <vector>

#include <ptiff/io/tile/tile_cache.hpp>
#include <ptiff/io/tile/tile_iterator.hpp>
#include <ptiff/io/tile/tile_storage.hpp>

#include <catch2/catch_test_macros.hpp>

namespace {

using ptiff::io::tile::Tile;
using ptiff::io::tile::TileIndex;

class InMemoryTileCache final : public ptiff::io::tile::TileCache {
public:
    [[nodiscard]] std::optional<Tile> find(ptiff::TileId id) const override {
        for (const auto& tile : tiles_) {
            if (tile.id() == id) {
                return tile;
            }
        }
        return std::nullopt;
    }
    void insert(Tile tile) override { tiles_.push_back(tile); }
    void clear() override { tiles_.clear(); }

private:
    std::vector<Tile> tiles_;
};

class EmptyTileStorage final : public ptiff::io::tile::TileStorage {
public:
    [[nodiscard]] ptiff::Result<Tile> load(const TileIndex&) override {
        return std::unexpected(ptiff::Error{ptiff::ErrorCode::NotImplemented, "not implemented"});
    }
    [[nodiscard]] ptiff::Result<void> store(const Tile&) override {
        return std::unexpected(ptiff::Error{ptiff::ErrorCode::NotImplemented, "not implemented"});
    }
};

class RangeTileIterator final : public ptiff::io::tile::TileIterator {
public:
    explicit RangeTileIterator(std::uint32_t count) : count_(count) {}

    [[nodiscard]] bool hasNext() const noexcept override { return next_ < count_; }
    [[nodiscard]] ptiff::Result<TileIndex> next() override {
        if (!hasNext()) {
            return std::unexpected(ptiff::Error{ptiff::ErrorCode::OutOfRange, "no more tiles"});
        }
        return TileIndex{.column = next_++, .row = 0, .level = 0};
    }
    void reset() noexcept override { next_ = 0; }

private:
    std::uint32_t count_;
    std::uint32_t next_ = 0;
};

static_assert(!std::is_copy_constructible_v<ptiff::io::tile::TileCache>);
static_assert(!std::is_move_constructible_v<ptiff::io::tile::TileCache>);
static_assert(!std::is_copy_constructible_v<ptiff::io::tile::TileStorage>);
static_assert(!std::is_move_constructible_v<ptiff::io::tile::TileStorage>);
static_assert(!std::is_copy_constructible_v<ptiff::io::tile::TileIterator>);
static_assert(!std::is_move_constructible_v<ptiff::io::tile::TileIterator>);

} // namespace

TEST_CASE("TileCache stores and finds by id", "[tile-interfaces]") {
    InMemoryTileCache cache;
    std::array<std::byte, 1> byte{std::byte{0}};
    cache.insert(Tile{ptiff::TileId{5}, TileIndex{}, {}, std::span<const std::byte>{byte}});

    REQUIRE(cache.find(ptiff::TileId{5}).has_value());
    REQUIRE_FALSE(cache.find(ptiff::TileId{6}).has_value());

    cache.clear();
    REQUIRE_FALSE(cache.find(ptiff::TileId{5}).has_value());
}

TEST_CASE("TileStorage stub reports NotImplemented", "[tile-interfaces]") {
    EmptyTileStorage storage;
    auto result = storage.load(TileIndex{});
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::NotImplemented);
}

TEST_CASE("TileIterator walks a range and resets", "[tile-interfaces]") {
    RangeTileIterator iterator{3};

    std::uint32_t count = 0;
    while (iterator.hasNext()) {
        REQUIRE(iterator.next().has_value());
        ++count;
    }
    REQUIRE(count == 3);
    REQUIRE_FALSE(iterator.next().has_value());

    iterator.reset();
    REQUIRE(iterator.hasNext());
}
