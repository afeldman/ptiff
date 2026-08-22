#include <cmath>
#include <cstddef>
#include <vector>

#include <ptiff/compression/jpeg.hpp>

#include <catch2/catch_test_macros.hpp>

using ptiff::compression::decodeJpeg;
using ptiff::compression::encodeJpeg;

namespace {

// Tolerance compare -- JPEG is a lossy codec (DCT quantization), never byte-exact.
void requireCloseWithinTolerance(const std::vector<std::byte>& actual,
                                 const std::vector<std::byte>& expected,
                                 int tolerance) {
    REQUIRE(actual.size() == expected.size());
    for (std::size_t i = 0; i < expected.size(); ++i) {
        const int a = static_cast<int>(actual[i]);
        const int e = static_cast<int>(expected[i]);
        REQUIRE(std::abs(a - e) <= tolerance);
    }
}

std::vector<std::byte> gradient(std::size_t count) {
    std::vector<std::byte> data(count);
    for (std::size_t i = 0; i < count; ++i) {
        data[i] = static_cast<std::byte>((i * 7) % 256);
    }
    return data;
}

} // namespace

TEST_CASE("encodeJpeg round-trips decodeJpeg on a 16x16 grayscale gradient", "[jpeg]") {
    constexpr std::uint32_t kWidth = 16, kHeight = 16, kSamples = 1;
    auto data = gradient(static_cast<std::size_t>(kWidth) * kHeight * kSamples);
    auto enc = encodeJpeg(data, kWidth, kHeight, kSamples, 90);
    REQUIRE(enc.has_value());
    auto dec = decodeJpeg(*enc, kWidth, kHeight, kSamples);
    REQUIRE(dec.has_value());
    requireCloseWithinTolerance(*dec, data, 15);
}

TEST_CASE("encodeJpeg round-trips decodeJpeg on a 16x16 RGB gradient", "[jpeg]") {
    constexpr std::uint32_t kWidth = 16, kHeight = 16, kSamples = 3;
    auto data = gradient(static_cast<std::size_t>(kWidth) * kHeight * kSamples);
    auto enc = encodeJpeg(data, kWidth, kHeight, kSamples, 90);
    REQUIRE(enc.has_value());
    auto dec = decodeJpeg(*enc, kWidth, kHeight, kSamples);
    REQUIRE(dec.has_value());
    requireCloseWithinTolerance(*dec, data, 30);
}

TEST_CASE("encodeJpeg rejects a pixel buffer that doesn't match width*height*samplesPerPixel",
          "[jpeg]") {
    std::vector<std::byte> data(16 * 16 * 1);
    auto result = encodeJpeg(data, 16, 16, 3, 90); // claims 3 samples, buffer sized for 1
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("encodeJpeg rejects an out-of-range quality", "[jpeg]") {
    std::vector<std::byte> data(16 * 16, std::byte{0x80});
    auto tooLow = encodeJpeg(data, 16, 16, 1, -1);
    REQUIRE_FALSE(tooLow.has_value());
    REQUIRE(tooLow.error().code() == ptiff::ErrorCode::InvalidArgument);
    auto tooHigh = encodeJpeg(data, 16, 16, 1, 101);
    REQUIRE_FALSE(tooHigh.has_value());
    REQUIRE(tooHigh.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("encodeJpeg rejects an unsupported samplesPerPixel", "[jpeg]") {
    std::vector<std::byte> data(16 * 16 * 2);
    auto result = encodeJpeg(data, 16, 16, 2, 90);
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("decodeJpeg rejects a malformed JPEG stream", "[jpeg]") {
    std::vector<std::byte> garbage{
        std::byte{0x00}, std::byte{0x01}, std::byte{0x02}, std::byte{0x03}};
    auto result = decodeJpeg(garbage, 16, 16, 1);
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("decodeJpeg rejects a dimension mismatch against the encoded stream", "[jpeg]") {
    constexpr std::uint32_t kWidth = 16, kHeight = 16, kSamples = 1;
    auto data = gradient(static_cast<std::size_t>(kWidth) * kHeight * kSamples);
    auto enc = encodeJpeg(data, kWidth, kHeight, kSamples, 90);
    REQUIRE(enc.has_value());
    auto result = decodeJpeg(*enc, 8, 8, 1); // wrong dimensions
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);
}
