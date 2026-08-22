#include <type_traits>

#include <ptiff/io/deserializer.hpp>
#include <ptiff/io/serializer.hpp>
#include <ptiff/scene.hpp>

#include <catch2/catch_test_macros.hpp>

namespace {

class StubSerializer final : public ptiff::io::Serializer {
public:
    [[nodiscard]] ptiff::Result<ptiff::io::StorageModel>
    serialize(const ptiff::Scene&) const override {
        return std::unexpected(ptiff::Error{ptiff::ErrorCode::NotImplemented, "not implemented"});
    }
};

class StubDeserializer final : public ptiff::io::Deserializer {
public:
    [[nodiscard]] ptiff::Result<ptiff::Scene>
    deserialize(const ptiff::io::StorageModel&) const override {
        return std::unexpected(ptiff::Error{ptiff::ErrorCode::NotImplemented, "not implemented"});
    }
};

static_assert(!std::is_copy_constructible_v<ptiff::io::Serializer>);
static_assert(!std::is_copy_constructible_v<ptiff::io::Deserializer>);

} // namespace

TEST_CASE("Serializer stub reports NotImplemented", "[serializer]") {
    StubSerializer serializer;
    ptiff::Scene scene;

    auto result = serializer.serialize(scene);
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::NotImplemented);
}

TEST_CASE("Deserializer stub reports NotImplemented", "[serializer]") {
    StubDeserializer deserializer;
    ptiff::io::StorageModel model;

    auto result = deserializer.deserialize(model);
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::NotImplemented);
}
