#include <array>
#include <cstddef>
#include <filesystem>
#include <fstream>
#include <span>

#include <ptiff/io/file_binary_reader.hpp>

#include <catch2/catch_test_macros.hpp>

namespace {

std::filesystem::path writeTempFile(const std::string& name, std::span<const std::byte> bytes) {
    auto path = std::filesystem::temp_directory_path() / name;
    std::ofstream stream(path, std::ios::binary | std::ios::trunc);
    stream.write(reinterpret_cast<const char*>(bytes.data()),
                 static_cast<std::streamsize>(bytes.size()));
    stream.close();
    return path;
}

} // namespace

TEST_CASE("FileBinaryReader::open fails for a missing file", "[file-binary-reader]") {
    auto result = ptiff::io::FileBinaryReader::open("/no/such/path/does-not-exist.bin");
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::NotFound);
}

TEST_CASE("FileBinaryReader reports size and reads bytes", "[file-binary-reader]") {
    const std::array<std::byte, 4> content{std::byte{1}, std::byte{2}, std::byte{3}, std::byte{4}};
    const auto path = writeTempFile("ptiff_file_binary_reader_test_basic.bin", content);

    auto result = ptiff::io::FileBinaryReader::open(path.string());
    REQUIRE(result.has_value());
    auto& reader = **result;

    auto size = reader.size();
    REQUIRE(size.has_value());
    REQUIRE(*size == 4);

    std::array<std::byte, 4> buffer{};
    auto readResult = reader.read(buffer);
    REQUIRE(readResult.has_value());
    REQUIRE(*readResult == 4);
    REQUIRE(buffer == content);

    std::filesystem::remove(path);
}

TEST_CASE("FileBinaryReader seek moves the cursor and position() reflects it",
          "[file-binary-reader]") {
    const std::array<std::byte, 4> content{
        std::byte{10}, std::byte{20}, std::byte{30}, std::byte{40}};
    const auto path = writeTempFile("ptiff_file_binary_reader_test_seek.bin", content);

    auto result = ptiff::io::FileBinaryReader::open(path.string());
    REQUIRE(result.has_value());
    auto& reader = **result;

    auto seekResult = reader.seek(2);
    REQUIRE(seekResult.has_value());

    auto pos = reader.position();
    REQUIRE(pos.has_value());
    REQUIRE(*pos == 2);

    std::array<std::byte, 2> buffer{};
    auto readResult = reader.read(buffer);
    REQUIRE(readResult.has_value());
    REQUIRE(*readResult == 2);
    REQUIRE(buffer[0] == std::byte{30});
    REQUIRE(buffer[1] == std::byte{40});

    std::filesystem::remove(path);
}

TEST_CASE("FileBinaryReader::read past end-of-file returns fewer bytes, not an error",
          "[file-binary-reader]") {
    const std::array<std::byte, 2> content{std::byte{5}, std::byte{6}};
    const auto path = writeTempFile("ptiff_file_binary_reader_test_eof.bin", content);

    auto result = ptiff::io::FileBinaryReader::open(path.string());
    REQUIRE(result.has_value());
    auto& reader = **result;

    std::array<std::byte, 8> buffer{};
    auto readResult = reader.read(buffer);
    REQUIRE(readResult.has_value());
    REQUIRE(*readResult == 2);

    std::filesystem::remove(path);
}
