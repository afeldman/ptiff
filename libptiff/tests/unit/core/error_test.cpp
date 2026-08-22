#include <string_view>

#include <ptiff/core/error.hpp>

#include <catch2/catch_test_macros.hpp>

TEST_CASE("Error carries code and message", "[error]") {
    ptiff::Error err{ptiff::ErrorCode::InvalidArgument, "bad input"};

    REQUIRE(err.code() == ptiff::ErrorCode::InvalidArgument);
    REQUIRE(err.message() == "bad input");
}

TEST_CASE("Error captures a source location", "[error]") {
    ptiff::Error err{ptiff::ErrorCode::OutOfRange, "capture location test"};

    REQUIRE(err.location().line() > 0);
    REQUIRE_FALSE(std::string_view{err.location().file_name()}.empty());
}
