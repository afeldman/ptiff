#pragma once

#include <memory>
#include <string_view>

#include <ptiff/logging/logger.hpp>

namespace ptiff::detail {

/// spdlog-backed implementation of the ptiff::Logger contract. Kept entirely out of the
/// public header so spdlog is never a compile-time dependency for consumers.
class SpdlogLogger {
public:
    SpdlogLogger();
    ~SpdlogLogger();

    SpdlogLogger(const SpdlogLogger&) = delete;
    SpdlogLogger& operator=(const SpdlogLogger&) = delete;

    void setLevel(LogLevel level) noexcept;
    [[nodiscard]] LogLevel level() const noexcept;
    void log(LogLevel level, std::string_view message, std::string_view locationText);

private:
    struct Impl;
    std::unique_ptr<Impl> impl_;
};

} // namespace ptiff::detail
