#include <ptiff/core/result.hpp>

#include <catch2/catch_test_macros.hpp>

TEST_CASE("Result holds a success value", "[result]") {
    auto getSuccess = []() -> ptiff::Result<int> { return 42; };

    auto result = getSuccess();

    REQUIRE(result.has_value());
    REQUIRE(result.value() == 42);
}

TEST_CASE("Result holds an error", "[result]") {
    auto getError = []() -> ptiff::Result<int> {
        return std::unexpected(ptiff::Error{ptiff::ErrorCode::NotImplemented, "nope"});
    };

    auto result = getError();

    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::NotImplemented);
}
