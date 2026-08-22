#include <cstddef>
#include <vector>

#include <ptiff/compression/deflate.hpp>

#include <catch2/catch_test_macros.hpp>

using ptiff::compression::decodeDeflate;
using ptiff::compression::encodeDeflate;

TEST_CASE("encodeDeflate round-trips decodeDeflate on repetitive data", "[deflate]") {
    std::vector<std::byte> data(256, std::byte{0xAB});
    auto enc = encodeDeflate(data);
    REQUIRE(enc.has_value());
    auto dec = decodeDeflate(*enc, data.size());
    REQUIRE(dec.has_value());
    REQUIRE(*dec == data);
}

TEST_CASE("encodeDeflate round-trips decodeDeflate on changing data", "[deflate]") {
    std::vector<std::byte> data(128);
    for (int i = 0; i < 128; ++i)
        data[static_cast<std::size_t>(i)] = static_cast<std::byte>(i);
    auto enc = encodeDeflate(data);
    REQUIRE(enc.has_value());
    auto dec = decodeDeflate(*enc, data.size());
    REQUIRE(dec.has_value());
    REQUIRE(*dec == data);
}

TEST_CASE("encodeDeflate round-trips decodeDeflate on empty input", "[deflate]") {
    std::vector<std::byte> data;
    auto enc = encodeDeflate(data);
    REQUIRE(enc.has_value());
    auto dec = decodeDeflate(*enc, data.size());
    REQUIRE(dec.has_value());
    REQUIRE(dec->empty());
}

TEST_CASE("decodeDeflate rejects a malformed zlib stream", "[deflate]") {
    std::vector<std::byte> garbage{
        std::byte{0x00}, std::byte{0x01}, std::byte{0x02}, std::byte{0x03}};
    auto result = decodeDeflate(garbage, 16);
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("decodeDeflate rejects an expectedSize too small to hold the decoded stream",
          "[deflate]") {
    std::vector<std::byte> data(64, std::byte{0x42});
    auto enc = encodeDeflate(data);
    REQUIRE(enc.has_value());
    auto result = decodeDeflate(*enc, 32);
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("decodeDeflate rejects an expectedSize larger than the decoded stream", "[deflate]") {
    std::vector<std::byte> data(64, std::byte{0x42});
    auto enc = encodeDeflate(data);
    REQUIRE(enc.has_value());
    auto result = decodeDeflate(*enc, 128);
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);
}
