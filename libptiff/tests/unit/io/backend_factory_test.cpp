#include <algorithm>
#include <memory>
#include <string_view>

#include <ptiff/io/backend_factory.hpp>

#include <catch2/catch_test_macros.hpp>

namespace {

class StubBackend final : public ptiff::io::StorageBackend {
public:
    [[nodiscard]] std::string_view name() const noexcept override { return "stub-factory-test"; }
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

} // namespace

TEST_CASE("BackendFactory registers and creates a backend by name", "[backend-factory]") {
    auto& factory = ptiff::io::BackendFactory::instance();

    auto registered = factory.registerBackend("stub-factory-test",
                                              [] { return std::make_unique<StubBackend>(); });
    REQUIRE(registered.has_value());

    auto backend = factory.create("stub-factory-test");
    REQUIRE(backend.has_value());
    REQUIRE((*backend)->name() == "stub-factory-test");

    const auto names = factory.registeredBackends();
    REQUIRE(std::ranges::find(names, "stub-factory-test") != names.end());
}

TEST_CASE("BackendFactory rejects duplicate registration", "[backend-factory]") {
    auto& factory = ptiff::io::BackendFactory::instance();
    (void)factory.registerBackend("duplicate-test", [] { return std::make_unique<StubBackend>(); });

    auto second =
        factory.registerBackend("duplicate-test", [] { return std::make_unique<StubBackend>(); });
    REQUIRE_FALSE(second.has_value());
    REQUIRE(second.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("BackendFactory::create reports NotFound for an unregistered name", "[backend-factory]") {
    auto& factory = ptiff::io::BackendFactory::instance();

    auto backend = factory.create("does-not-exist");
    REQUIRE_FALSE(backend.has_value());
    REQUIRE(backend.error().code() == ptiff::ErrorCode::NotFound);
}
