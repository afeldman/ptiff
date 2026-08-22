#include <ptiff/core/version.hpp>

#include <catch2/catch_test_macros.hpp>

TEST_CASE("compile-time and runtime version agree", "[version]") {
    REQUIRE(ptiff::compileTimeVersion() == ptiff::runtimeVersion());
}

TEST_CASE("version numbers are non-negative", "[version]") {
    auto version = ptiff::compileTimeVersion();

    REQUIRE(version.major >= 0);
    REQUIRE(version.minor >= 0);
    REQUIRE(version.patch >= 0);
}
