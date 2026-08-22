#include <ptiff/logging/logger.hpp>

#include <catch2/catch_test_macros.hpp>

TEST_CASE("Logger level can be set and read back", "[logger]") {
    ptiff::Logger::instance().setLevel(ptiff::LogLevel::Debug);

    REQUIRE(ptiff::Logger::instance().level() == ptiff::LogLevel::Debug);

    // Restore to the default -- Logger is a process-wide singleton and tests may run in any order.
    ptiff::Logger::instance().setLevel(ptiff::LogLevel::Info);
}

TEST_CASE("Logger::log does not throw", "[logger]") {
    REQUIRE_NOTHROW(ptiff::Logger::instance().log(ptiff::LogLevel::Info, "test message"));
}
