#include <algorithm>
#include <cstddef>
#include <cstdint>
#include <filesystem>
#include <span>
#include <string>

#include <ptiff/io/backend/isis_backend.hpp>
#include <ptiff/io/backend/memory_backend.hpp>
#include <ptiff/io/backend/openexr_backend.hpp>
#include <ptiff/io/backend/pds4_backend.hpp>
#include <ptiff/io/backend/tiff_backend.hpp>
#include <ptiff/io/backend/zarr_backend.hpp>
#include <ptiff/io/backend_factory.hpp>
#include <ptiff/io/file_binary_writer.hpp>
#include <ptiff/io/storage_model.hpp>

#include <catch2/catch_test_macros.hpp>

TEST_CASE("all six format backends self-register", "[backends]") {
    const auto names = ptiff::io::BackendFactory::instance().registeredBackends();

    for (const auto* expected : {"tiff", "memory", "pds4", "isis", "zarr", "openexr"}) {
        INFO("looking for backend: " << expected);
        REQUIRE(std::ranges::find(names, std::string{expected}) != names.end());
    }
}

namespace {

class NullBinaryReader final : public ptiff::io::BinaryReader {
public:
    ptiff::Result<std::size_t> read(std::span<std::byte>) override { return 0; }
    ptiff::Result<void> seek(std::uint64_t) override { return {}; }
    ptiff::Result<std::uint64_t> position() const override { return 0; }
    ptiff::Result<std::uint64_t> size() const override { return 0; }
};

} // namespace

TEST_CASE("zarr backend rejects malformed input instead of reporting NotImplemented",
          "[backends]") {
    auto& factory = ptiff::io::BackendFactory::instance();
    auto backend = factory.create("zarr");
    REQUIRE(backend.has_value());
    REQUIRE((*backend)->name() == std::string{"zarr"});

    // An empty reader is not a valid Zarr document: the header scan fails as InvalidArgument.
    NullBinaryReader reader;
    auto model = (*backend)->deserializeModel(reader);
    REQUIRE_FALSE(model.has_value());
    REQUIRE(model.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("openexr backend rejects malformed input instead of reporting NotImplemented",
          "[backends]") {
    auto& factory = ptiff::io::BackendFactory::instance();
    auto backend = factory.create("openexr");
    REQUIRE(backend.has_value());
    REQUIRE((*backend)->name() == std::string{"openexr"});

    // An empty reader is not a valid .exr document: OpenEXR's own magic-number read fails,
    // which readHeaderInfo translates to InvalidArgument, never NotImplemented.
    NullBinaryReader reader;
    auto model = (*backend)->deserializeModel(reader);
    REQUIRE_FALSE(model.has_value());
    REQUIRE(model.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("isis backend rejects malformed input instead of reporting NotImplemented",
          "[backends]") {
    auto& factory = ptiff::io::BackendFactory::instance();
    auto backend = factory.create("isis");
    REQUIRE(backend.has_value());
    REQUIRE((*backend)->name() == std::string{"isis"});

    // An empty reader is not a valid ISIS3 document: the label scan fails as InvalidArgument, so
    // malformed/empty input is never reported as NotImplemented.
    NullBinaryReader reader;
    auto model = (*backend)->deserializeModel(reader);
    REQUIRE_FALSE(model.has_value());
    REQUIRE(model.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("pds4 backend rejects malformed input instead of reporting NotImplemented",
          "[backends]") {
    auto& factory = ptiff::io::BackendFactory::instance();
    auto backend = factory.create("pds4");
    REQUIRE(backend.has_value());
    REQUIRE((*backend)->name() == std::string{"pds4"});

    // An empty reader is not a valid PDS4 document: the seek header read should fail as
    // InvalidArgument (truncated input), never NotImplemented.
    NullBinaryReader reader;
    auto model = (*backend)->deserializeModel(reader);
    REQUIRE_FALSE(model.has_value());
    REQUIRE(model.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("memory backend rejects malformed input instead of reporting NotImplemented",
          "[backends]") {
    auto& factory = ptiff::io::BackendFactory::instance();
    auto backend = factory.create("memory");
    REQUIRE(backend.has_value());
    REQUIRE((*backend)->name() == std::string{"memory"});

    NullBinaryReader reader;
    auto model = (*backend)->deserializeModel(reader);
    REQUIRE_FALSE(model.has_value());
    REQUIRE(model.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("tiff backend rejects malformed input instead of reporting NotImplemented",
          "[backends]") {
    auto& factory = ptiff::io::BackendFactory::instance();
    auto backend = factory.create("tiff");
    REQUIRE(backend.has_value());
    REQUIRE((*backend)->name() == std::string{"tiff"});

    NullBinaryReader reader;
    auto model = (*backend)->deserializeModel(reader);
    REQUIRE_FALSE(model.has_value());
    REQUIRE(model.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("TiffBackend::serializeModel writes real bytes instead of reporting NotImplemented",
          "[backends]") {
    auto& factory = ptiff::io::BackendFactory::instance();
    auto backend = factory.create("tiff");
    REQUIRE(backend.has_value());

    ptiff::io::StorageModel model;
    model.setField("imageWidth", "2");
    model.setField("imageHeight", "1");
    model.setField("samplesPerPixel", "1");
    model.setField("pixelType", "UInt8");

    auto path = std::filesystem::temp_directory_path() / "ptiff_backends_test_serialize.tif";
    auto writerResult = ptiff::io::FileBinaryWriter::create(path.string());
    REQUIRE(writerResult.has_value());

    auto result = (*backend)->serializeModel(model, **writerResult);
    REQUIRE(result.has_value());

    std::filesystem::remove(path);
}

TEST_CASE(
    "TiffBackend::openImageSink reports InvalidArgument for a malformed model, not NotImplemented",
    "[backends]") {
    auto& factory = ptiff::io::BackendFactory::instance();
    auto backend = factory.create("tiff");
    REQUIRE(backend.has_value());

    ptiff::io::StorageModel emptyModel;
    auto path = std::filesystem::temp_directory_path() / "ptiff_backends_test_sink.tif";
    auto writerResult = ptiff::io::FileBinaryWriter::create(path.string());
    REQUIRE(writerResult.has_value());

    auto result = (*backend)->openImageSink(**writerResult, emptyModel);
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ptiff::ErrorCode::InvalidArgument);

    std::filesystem::remove(path);
}
