#include <array>
#include <type_traits>

#include <ptiff/io/binary_reader.hpp>
#include <ptiff/io/binary_writer.hpp>

#include <catch2/catch_test_macros.hpp>

namespace {

class FakeBinaryReader final : public ptiff::io::BinaryReader {
public:
    ptiff::Result<std::size_t> read(std::span<std::byte> destination) override {
        return destination.size();
    }
    ptiff::Result<void> seek(std::uint64_t) override { return {}; }
    ptiff::Result<std::uint64_t> position() const override { return 0; }
    ptiff::Result<std::uint64_t> size() const override { return 0; }
};

class FakeBinaryWriter final : public ptiff::io::BinaryWriter {
public:
    ptiff::Result<std::size_t> write(std::span<const std::byte> source) override {
        return source.size();
    }
    ptiff::Result<void> seek(std::uint64_t) override { return {}; }
    ptiff::Result<std::uint64_t> position() const override { return 0; }
    ptiff::Result<void> flush() override { return {}; }
};

static_assert(!std::is_copy_constructible_v<ptiff::io::BinaryReader>);
static_assert(!std::is_move_constructible_v<ptiff::io::BinaryReader>);
static_assert(!std::is_copy_constructible_v<ptiff::io::BinaryWriter>);
static_assert(!std::is_move_constructible_v<ptiff::io::BinaryWriter>);

} // namespace

TEST_CASE("BinaryReader dispatches through the interface", "[binary-io]") {
    FakeBinaryReader reader;
    std::array<std::byte, 8> buffer{};

    auto result = reader.read(buffer);
    REQUIRE(result.has_value());
    REQUIRE(result.value() == 8);
}

TEST_CASE("BinaryWriter dispatches through the interface", "[binary-io]") {
    FakeBinaryWriter writer;
    std::array<std::byte, 3> buffer{};

    auto result = writer.write(buffer);
    REQUIRE(result.has_value());
    REQUIRE(result.value() == 3);
    REQUIRE(writer.flush().has_value());
}
