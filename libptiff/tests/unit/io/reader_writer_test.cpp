#include <cstdio>
#include <filesystem>
#include <string>

#include <ptiff/image.hpp>
#include <ptiff/image/image_descriptor.hpp>
#include <ptiff/io/http_range_binary_reader.hpp>
#include <ptiff/io/reader.hpp>
#include <ptiff/io/writer.hpp>
#include <ptiff/scene.hpp>

#include <catch2/catch_test_macros.hpp>

using ptiff::CompressionKind;
using ptiff::ImageDescriptor;
using ptiff::PixelType;
using ptiff::Reader;
using ptiff::Scene;
using ptiff::Writer;

namespace {

class TempFile {
public:
    TempFile()
        : path_(std::filesystem::temp_directory_path() /
                ("ptiff_reader_writer_" + std::to_string(counter_++) + ".ptiff")) {}
    ~TempFile() { std::remove(path_.c_str()); }
    const std::string& path() const { return path_; }

private:
    static int counter_;
    std::string path_;
};
int TempFile::counter_ = 0;

Scene buildScene() {
    Scene scene;
    ImageDescriptor gs;
    gs.width = 16;
    gs.height = 16;
    gs.pixelType = PixelType::UInt8;
    gs.channelCount = 1;
    gs.compression = CompressionKind::Lzw;
    (void)scene.addImage(gs);
    return scene;
}
} // namespace

TEST_CASE("Writer::create + write then Reader::open reproduces scene fields", "[reader-writer]") {
    TempFile tmp;

    auto writer = Writer::create(tmp.path());
    REQUIRE(writer.has_value());
    REQUIRE(static_cast<bool>(*writer));

    Scene scene = buildScene();
    auto writeResult = (*writer)->write(scene);
    REQUIRE(writeResult.has_value());

    auto reader = Reader::open(tmp.path());
    REQUIRE(reader.has_value());
    REQUIRE(static_cast<bool>(*reader));

    auto sceneResult = (*reader)->scene();
    REQUIRE(sceneResult.has_value());
    const Scene* readScene = *sceneResult;
    REQUIRE(readScene != nullptr);
    REQUIRE(*readScene->imageCount() == 1);
    auto img = readScene->imageAt(0);
    REQUIRE(img.has_value());
    REQUIRE(img->get().width() == 16);
    REQUIRE(img->get().height() == 16);
    REQUIRE(img->get().pixelType() == PixelType::UInt8);
    REQUIRE(img->get().channelCount() == 1);
    REQUIRE(img->get().compression().has_value());
    REQUIRE(*img->get().compression() == CompressionKind::Lzw);
}

TEST_CASE("Reader::open of a missing file returns NotFound", "[reader-writer]") {
    auto reader = Reader::open("/nonexistent/ptiff/no_such_file.ptiff");
    REQUIRE_FALSE(reader.has_value());
    REQUIRE(reader.error().code() == ptiff::ErrorCode::NotFound);
}

TEST_CASE("Writer::create into a nonexistent directory returns InvalidArgument",
          "[reader-writer]") {
    auto writer = Writer::create("/nonexistent/ptiff_dir/out.ptiff");
    REQUIRE_FALSE(writer.has_value());
    REQUIRE(writer.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("Reader::open on a non-http(s) path uses the local file transport unchanged",
          "[reader-writer]") {
    TempFile tmp;
    auto writer = Writer::create(tmp.path());
    REQUIRE(writer.has_value());
    Scene scene = buildScene();
    REQUIRE((*writer)->write(scene).has_value());

    auto reader = Reader::open(tmp.path());
    REQUIRE(reader.has_value());
    auto sceneResult = (*reader)->scene();
    REQUIRE(sceneResult.has_value());
}

TEST_CASE("Reader::open rejects an unreachable http(s) URL instead of treating it as a filename",
          "[reader-writer]") {
    // Port 1 is a well-known reserved/unused port -- connection is refused immediately, proving
    // Reader::open actually attempted an HTTP transport rather than silently falling back to
    // treating this string as a local filename (which would fail with NotFound for a different
    // reason and defeat the point of this test).
    auto reader = Reader::open("http://127.0.0.1:1/nonexistent");
    REQUIRE_FALSE(reader.has_value());
    REQUIRE(reader.error().code() == ptiff::ErrorCode::Unknown);
}
