// ptiff-cpp round-trip + API-parity test.
//
// Exercises the full ptiff-cpp surface over the Rust C ABI:
//   - version accessors
//   - Image / ImageDescriptor / Scene (in-memory model)
//   - Writer::create + write(scene, provider) -> real tiled TIFF
//   - Reader::open + scene() + imageSource() -> tile reads
//   - Camera::readFrom / Camera::toC structured camera round-trip
//   - ErrorCode / Error / Result mapping (negative-path)
//
// Build+run: `make test` in bindings/cpp (links the Rust-built libptiff_c).

#include <cassert>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <memory>
#include <string>
#include <vector>

#include <ptiff/ptiff.hpp>

using namespace ptiff;

namespace {

int g_failures = 0;

#define CHECK(cond, msg)                                                        \
    do {                                                                        \
        if (!(cond)) {                                                          \
            std::fprintf(stderr, "FAIL %s:%d: %s\n", __FILE__, __LINE__, (msg)); \
            ++g_failures;                                                       \
        }                                                                       \
    } while (0)

// A provider that yields each tile filled with a (row*m + col+1) byte pattern,
// matching the C-ABI test's convention so values are easy to verify.
class PatternProvider final : public TileProvider {
public:
    PatternProvider(std::uint32_t cols, std::uint32_t rows, std::size_t bytes)
        : cols_(cols), rows_(rows), bytes_(bytes) {}
    std::size_t tileBytes() const noexcept override { return bytes_; }
    bool tile(std::uint32_t column, std::uint32_t row,
                std::vector<uint8_t>& out) override {
        if (column >= cols_ || row >= rows_) return false;
        out.assign(bytes_, static_cast<uint8_t>(row * cols_ + column + 1));
        return true;
    }
private:
    std::uint32_t cols_, rows_;
    std::size_t bytes_;
};

const char* OUT_PATH = "./ptiff_cpp_roundtrip.tif";

void test_version() {
    Version ct = compileTimeVersion();
    Version rt = runtimeVersion();
    CHECK(ct.major == 1, "compile-time major == 1");
    CHECK(rt.major == ct.major && rt.minor == ct.minor && rt.patch == ct.patch,
            "runtime == compile-time version");
    CHECK(ct.toString() == "1.0.0", "version string == 1.0.0");
}

void test_error_code_mapping() {
    CHECK(cErrorCodeToEnum(0) == ErrorCode::NotImplemented, "0 -> NotImplemented");
    CHECK(cErrorCodeToEnum(-1) == ErrorCode::InvalidArgument, "-1 -> InvalidArgument");
    CHECK(cErrorCodeToEnum(-2) == ErrorCode::OutOfRange, "-2 -> OutOfRange");
    CHECK(cErrorCodeToEnum(-3) == ErrorCode::NotFound, "-3 -> NotFound");
    CHECK(cErrorCodeToEnum(-99) == ErrorCode::Unknown, "other -> Unknown");
}

void test_image_and_scene() {
    ImageDescriptor d;
    d.width = 40;
    d.height = 30;
    d.pixelType = PixelType::UInt16;
    d.channelCount = 3;
    d.groundSampleDistanceMeters = 1.25;
    d.compression = CompressionKind::Lzw;

    Scene scene;
    auto id = scene.addImage(d);
    CHECK(id.has_value(), "addImage ok");
    CHECK((*scene.imageCount()) == 1, "scene has 1 image");

    auto img = scene.imageAt(0);
    CHECK(img.has_value(), "imageAt(0) ok");
    CHECK(img->get().width() == 40, "image width == 40");
    CHECK(img->get().height() == 30, "image height == 30");
    CHECK(img->get().pixelType() == PixelType::UInt16, "image pixel type == UInt16");
    CHECK(img->get().channelCount() == 3, "image channel count == 3");
    CHECK(img->get().groundSampleDistanceMeters() && *img->get().groundSampleDistanceMeters() == 1.25,
            "image GSD == 1.25");
    CHECK(!img->get().tileInfo().has_value(), "tileInfo absent (untiled)");
    CHECK(img->get().compression() && *img->get().compression() == CompressionKind::Lzw,
            "image compression == Lzw");

    // Negative path: id out of range.
    auto bad = scene.image(ImageId{5});
    CHECK(!bad.has_value() && bad.error().code() == ErrorCode::OutOfRange,
            "image(out-of-range) -> OutOfRange");
}

void test_write_read_roundtrip() {
    ImageDescriptor d;
    d.width = 34;
    d.height = 34;
    d.pixelType = PixelType::UInt8;
    d.channelCount = 1;
    d.tileInfo = TileInfo{16, 16}; // 3x3 grid

    PatternProvider provider(3, 3, 16 * 16);

    auto writer = Writer::create(OUT_PATH);
    CHECK(writer.has_value(), "Writer::create ok");
    {
        Scene scene;
        auto ok = scene.addImage(d);
        CHECK(ok.has_value(), "addImage ok");
        auto r = (*writer)->write(scene, provider);
        CHECK(r.has_value(), "write(scene, provider) ok");
    }

    // Read back via Reader.
    auto reader = Reader::open(OUT_PATH);
    CHECK(reader.has_value(), "Reader::open ok");

    auto scene = (*reader)->scene();
    CHECK(scene.has_value(), "reader.scene() ok");
    CHECK((*scene).imageCount().value() == 1, "read scene has 1 image");
    auto img = (*scene).imageAt(0);
    CHECK(img->get().width() == 34 && img->get().height() == 34, "read image 34x34");
    CHECK(img->get().pixelType() == PixelType::UInt8, "read image pixel type UInt8");
    CHECK(img->get().tileInfo().has_value() &&
                img->get().tileInfo()->tileWidth == 16 &&
                img->get().tileInfo()->tileHeight == 16,
            "read image tile info 16x16");

    auto source = (*reader)->imageSource(ImageId{0});
    CHECK(source.has_value(), "reader.imageSource(0) ok");
    CHECK((*source)->tileColumns() == 3 && (*source)->tileRows() == 3, "source grid 3x3");
    CHECK((*source)->tileBytes() == 16 * 16, "source tile bytes == 256");

    // Verify pattern values.
    for (std::uint32_t r = 0; r < 3; ++r) {
        for (std::uint32_t c = 0; c < 3; ++c) {
            auto tile = (*source)->readTile(c, r);
            CHECK(tile.has_value(), "readTile ok");
            CHECK(tile->size() == 16 * 16, "tile size == 256");
            bool allOk = true;
            for (uint8_t b : *tile) {
                if (b != static_cast<uint8_t>(r * 3 + c + 1)) { allOk = false; break; }
            }
            std::string msg = "tile (" + std::to_string(c) + "," + std::to_string(r) + ") content";
            CHECK(allOk, msg.c_str());
        }
    }

    // Wrong image id -> NotFound.
    auto badSource = (*reader)->imageSource(ImageId{9});
    CHECK(!badSource.has_value() && badSource.error().code() == ErrorCode::NotFound,
            "imageSource(9) -> NotFound");

    // Negative path: reading a missing file -> NotFound.
    auto missing = Reader::open("./does_not_exist_ptiff_cpp.tif");
    CHECK(!missing.has_value() && missing.error().code() == ErrorCode::NotFound,
            "open(missing) -> NotFound");
}

void test_camera_roundtrip() {
    ImageDescriptor d;
    d.width = 34;
    d.height = 34;
    d.pixelType = PixelType::UInt8;
    d.channelCount = 1;
    d.tileInfo = TileInfo{16, 16};

    Camera cam(900.0, 901.0, 512.5, 384.25,
                0.7, 0.1, 0.2, 0.3,
                1.0, 2.0, 3.0, "");

    // Write a file with a camera using the C-ABI sink_create_camera directly.
    {
        auto cdesc = detail::toCDescriptor(d);
        auto ccam = cam.toC();
        detail::ptiff_sink* sink = detail::ptiff_sink_create_camera(OUT_PATH, &cdesc, &ccam);
        CHECK(sink != nullptr, "sink_create_camera ok");
        if (sink) {
            uint8_t tile[16 * 16];
            std::memset(tile, 5, sizeof(tile));
            for (std::uint32_t r = 0; r < 3; ++r)
                for (std::uint32_t c = 0; c < 3; ++c)
                    detail::ptiff_sink_write_tile(sink, c, r, tile, sizeof(tile));
            detail::ptiff_sink_close(sink);
        }
    }

    auto readback = Camera::readFrom(OUT_PATH);
    CHECK(readback.has_value(), "Camera::readFrom ok");
    if (readback.has_value()) {
        CHECK(readback->hasIntrinsics() && readback->hasExtrinsics(), "read camera has both");
        CHECK(readback->focalLengthX() == 900.0, "read fx == 900");
        CHECK(readback->principalY() == 384.25, "read cy == 384.25");
        CHECK(readback->rotationW() == 0.7, "read rw == 0.7");
        CHECK(readback->positionZ() == 3.0, "read pz == 3.0");
    }
}

} // namespace

int main() {
    std::printf("ptiff_cpp: round-trip + API-parity test over the Rust C ABI\n");
    test_version();
    test_error_code_mapping();
    test_image_and_scene();
    test_camera_roundtrip();
    test_write_read_roundtrip();
    if (g_failures) {
        std::fprintf(stderr, "ptiff_cpp: %d FAILURE(S)\n", g_failures);
        return 1;
    }
    std::printf("ptiff_cpp: ALL OK\n");
    std::remove(OUT_PATH);
    return 0;
}
