#include <type_traits>
#include <utility>

#include <ptiff/io/storage_model.hpp>

#include <catch2/catch_test_macros.hpp>

TEST_CASE("StorageModel stores and retrieves fields", "[storage-model]") {
    ptiff::io::StorageModel model;
    model.setField("format", "tiff");

    auto value = model.field("format");
    REQUIRE(value.has_value());
    REQUIRE(*value == "tiff");
}

TEST_CASE("StorageModel::field reports NotFound for an unset key", "[storage-model]") {
    ptiff::io::StorageModel model;

    auto value = model.field("missing");
    REQUIRE_FALSE(value.has_value());
    REQUIRE(value.error().code() == ptiff::ErrorCode::NotFound);
}

TEST_CASE("StorageModel holds child nodes", "[storage-model]") {
    ptiff::io::StorageModel root;
    ptiff::io::StorageModel child;
    child.setField("kind", "image");
    root.addChild(std::move(child));

    REQUIRE(root.children().size() == 1);
    auto kind = root.children()[0].field("kind");
    REQUIRE(kind.has_value());
    REQUIRE(*kind == "image");
}

TEST_CASE("StorageModel is move-only", "[storage-model]") {
    static_assert(!std::is_copy_constructible_v<ptiff::io::StorageModel>);
    static_assert(std::is_move_constructible_v<ptiff::io::StorageModel>);
}
