#include <array>
#include <cstddef>
#include <filesystem>
#include <fstream>
#include <span>
#include <vector>

#include <ptiff/io/file_binary_writer.hpp>

#include <catch2/catch_test_macros.hpp>

namespace {

std::vector<std::byte> readWholeFile(const std::filesystem::path& path) {
    std::ifstream stream(path, std::ios::binary);
    std::vector<char> chars{std::istreambuf_iterator<char>(stream),
                            std::istreambuf_iterator<char>()};
    std::vector<std::byte> bytes;
    bytes.reserve(chars.size());
    for (char c : chars) {
        bytes.push_back(static_cast<std::byte>(static_cast<unsigned char>(c)));
    }
    return bytes;
}

} // namespace

TEST_CASE("FileBinaryWriter::create fails for an unusable path", "[file-binary-writer]") {
    auto result = ptiff::io::FileBinaryWriter::create("/no/such/directory/does-not-exist.bin");
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("FileBinaryWriter writes bytes sequentially", "[file-binary-writer]") {
    auto path = std::filesystem::temp_directory_path() / "ptiff_file_binary_writer_test_basic.bin";
    std::filesystem::remove(path);

    auto result = ptiff::io::FileBinaryWriter::create(path.string());
    REQUIRE(result.has_value());
    auto& writer = **result;

    const std::array<std::byte, 4> content{std::byte{1}, std::byte{2}, std::byte{3}, std::byte{4}};
    auto written = writer.write(content);
    REQUIRE(written.has_value());
    REQUIRE(*written == 4);

    auto flushResult = writer.flush();
    REQUIRE(flushResult.has_value());

    auto onDisk = readWholeFile(path);
    REQUIRE(onDisk.size() == 4);
    REQUIRE(onDisk[0] == std::byte{1});
    REQUIRE(onDisk[3] == std::byte{4});

    std::filesystem::remove(path);
}

TEST_CASE("FileBinaryWriter seek moves the cursor and position() reflects it",
          "[file-binary-writer]") {
    auto path = std::filesystem::temp_directory_path() / "ptiff_file_binary_writer_test_seek.bin";
    std::filesystem::remove(path);

    auto result = ptiff::io::FileBinaryWriter::create(path.string());
    REQUIRE(result.has_value());
    auto& writer = **result;

    const std::array<std::byte, 4> zeros{};
    REQUIRE(writer.write(zeros).has_value());

    auto seekResult = writer.seek(1);
    REQUIRE(seekResult.has_value());
    auto pos = writer.position();
    REQUIRE(pos.has_value());
    REQUIRE(*pos == 1);

    const std::array<std::byte, 2> patch{std::byte{0xAA}, std::byte{0xBB}};
    REQUIRE(writer.write(patch).has_value());
    REQUIRE(writer.flush().has_value());

    auto onDisk = readWholeFile(path);
    REQUIRE(onDisk.size() == 4);
    REQUIRE(onDisk[1] == std::byte{0xAA});
    REQUIRE(onDisk[2] == std::byte{0xBB});

    std::filesystem::remove(path);
}
