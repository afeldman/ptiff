#include <cstddef>
#include <cstdint>
#include <limits>

#include <ptiff/core/error.hpp>
#include <ptiff/io/backend/tiff/checked_arithmetic.hpp>

#include <catch2/catch_test_macros.hpp>

namespace {

using ptiff::ErrorCode;
using ptiff::io::backend::tiff::checkedAddSize;
using ptiff::io::backend::tiff::checkedAddU64;
using ptiff::io::backend::tiff::checkedMulSize;
using ptiff::io::backend::tiff::checkedMulU64;

TEST_CASE("checkedMulU64 returns the product for non-overflowing inputs", "[io][tiff][checked]") {
    const auto r = checkedMulU64(6, 7);
    REQUIRE(r.has_value());
    REQUIRE(*r == 42);
}

TEST_CASE("checkedMulU64 rejects overflow instead of wrapping", "[io][tiff][checked]") {
    const auto r = checkedMulU64(std::numeric_limits<std::uint64_t>::max(), 2);
    REQUIRE_FALSE(r.has_value());
    REQUIRE(r.error().code() == ErrorCode::InvalidArgument);
}

TEST_CASE("checkedAddU64 returns the sum for non-overflowing inputs", "[io][tiff][checked]") {
    const auto r = checkedAddU64(40, 2);
    REQUIRE(r.has_value());
    REQUIRE(*r == 42);
}

TEST_CASE("checkedAddU64 rejects overflow instead of wrapping", "[io][tiff][checked]") {
    const auto r = checkedAddU64(std::numeric_limits<std::uint64_t>::max(), 1);
    REQUIRE_FALSE(r.has_value());
    REQUIRE(r.error().code() == ErrorCode::InvalidArgument);
}

TEST_CASE("checkedMulSize returns the product for non-overflowing inputs", "[io][tiff][checked]") {
    const auto r = checkedMulSize(std::size_t{6}, std::size_t{7});
    REQUIRE(r.has_value());
    REQUIRE(*r == 42);
}

TEST_CASE("checkedMulSize rejects overflow instead of wrapping", "[io][tiff][checked]") {
    const auto r = checkedMulSize(std::numeric_limits<std::size_t>::max(), std::size_t{2});
    REQUIRE_FALSE(r.has_value());
    REQUIRE(r.error().code() == ErrorCode::InvalidArgument);
}

TEST_CASE("checkedAddSize returns the sum for non-overflowing inputs", "[io][tiff][checked]") {
    const auto r = checkedAddSize(std::size_t{40}, std::size_t{2});
    REQUIRE(r.has_value());
    REQUIRE(*r == 42);
}

TEST_CASE("checkedAddSize rejects overflow instead of wrapping", "[io][tiff][checked]") {
    const auto r = checkedAddSize(std::numeric_limits<std::size_t>::max(), std::size_t{1});
    REQUIRE_FALSE(r.has_value());
    REQUIRE(r.error().code() == ErrorCode::InvalidArgument);
}

} // namespace
