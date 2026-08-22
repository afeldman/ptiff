#include <cstddef>
#include <cstdint>
#include <memory>
#include <span>
#include <string_view>
#include <type_traits>

#include <ptiff/io/binary_reader.hpp>
#include <ptiff/io/binary_writer.hpp>
#include <ptiff/io/storage_backend.hpp>

#include <catch2/catch_test_macros.hpp>

namespace {

class NullBinaryReader final : public ptiff::io::BinaryReader {
public:
    ptiff::Result<std::size_t> read(std::span<std::byte>) override { return 0; }
    ptiff::Result<void> seek(std::uint64_t) override { return {}; }
    ptiff::Result<std::uint64_t> position() const override { return 0; }
    ptiff::Result<std::uint64_t> size() const override { return 0; }
};

class NullBinaryWriter final : public ptiff::io::BinaryWriter {
public:
    ptiff::Result<std::size_t> write(std::span<const std::byte>) override { return 0; }
    ptiff::Result<void> seek(std::uint64_t) override { return {}; }
    ptiff::Result<std::uint64_t> position() const override { return 0; }
    ptiff::Result<void> flush() override { return {}; }
};

class StubStorageBackend final : public ptiff::io::StorageBackend {
public:
    [[nodiscard]] std::string_view name() const noexcept override { return "stub"; }
    [[nodiscard]] ptiff::io::BackendCapabilities capabilities() const noexcept override {
        return {};
    }

    [[nodiscard]] ptiff::Result<std::unique_ptr<ptiff::io::ImageSource>>
    openImageSource(ptiff::io::BinaryReader&) const override {
        return std::unexpected(ptiff::Error{ptiff::ErrorCode::NotImplemented, "not implemented"});
    }
    [[nodiscard]] ptiff::Result<std::unique_ptr<ptiff::io::ImageSink>>
    openImageSink(ptiff::io::BinaryWriter&, const ptiff::io::StorageModel&) const override {
        return std::unexpected(ptiff::Error{ptiff::ErrorCode::NotImplemented, "not implemented"});
    }
    [[nodiscard]] ptiff::Result<ptiff::io::StorageModel>
    deserializeModel(ptiff::io::BinaryReader&) const override {
        return std::unexpected(ptiff::Error{ptiff::ErrorCode::NotImplemented, "not implemented"});
    }
    [[nodiscard]] ptiff::Result<void> serializeModel(const ptiff::io::StorageModel&,
                                                     ptiff::io::BinaryWriter&) const override {
        return std::unexpected(ptiff::Error{ptiff::ErrorCode::NotImplemented, "not implemented"});
    }
};

static_assert(!std::is_copy_constructible_v<ptiff::io::StorageBackend>);

} // namespace

TEST_CASE("BackendCapabilities defaults to no capabilities", "[storage-backend]") {
    ptiff::io::BackendCapabilities caps;
    REQUIRE_FALSE(caps.supportsTiling);
    REQUIRE_FALSE(caps.supportsStreaming);
    REQUIRE_FALSE(caps.supportsRandomAccess);
    REQUIRE_FALSE(caps.supportsCloudStreaming);
}

TEST_CASE("StorageBackend stub reports NotImplemented for every operation", "[storage-backend]") {
    StubStorageBackend backend;
    NullBinaryReader reader;
    NullBinaryWriter writer;
    ptiff::io::StorageModel model;

    REQUIRE(backend.name() == "stub");
    REQUIRE_FALSE(backend.openImageSource(reader).has_value());
    REQUIRE_FALSE(backend.openImageSink(writer, model).has_value());
    REQUIRE_FALSE(backend.deserializeModel(reader).has_value());
    REQUIRE_FALSE(backend.serializeModel(model, writer).has_value());
}
