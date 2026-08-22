#include <spdlog/spdlog.h>

#include <ptiff/logging/detail/spdlog_logger.hpp>

namespace ptiff::detail {

namespace {

spdlog::level::level_enum toSpdlogLevel(LogLevel level) noexcept {
    switch (level) {
    case LogLevel::Trace:
        return spdlog::level::trace;
    case LogLevel::Debug:
        return spdlog::level::debug;
    case LogLevel::Info:
        return spdlog::level::info;
    case LogLevel::Warn:
        return spdlog::level::warn;
    case LogLevel::Error:
        return spdlog::level::err;
    case LogLevel::Critical:
        return spdlog::level::critical;
    case LogLevel::Off:
        return spdlog::level::off;
    }
    return spdlog::level::info;
}

LogLevel fromSpdlogLevel(spdlog::level::level_enum level) noexcept {
    switch (level) {
    case spdlog::level::trace:
        return LogLevel::Trace;
    case spdlog::level::debug:
        return LogLevel::Debug;
    case spdlog::level::info:
        return LogLevel::Info;
    case spdlog::level::warn:
        return LogLevel::Warn;
    case spdlog::level::err:
        return LogLevel::Error;
    case spdlog::level::critical:
        return LogLevel::Critical;
    default:
        return LogLevel::Off;
    }
}

} // namespace

struct SpdlogLogger::Impl {
    std::shared_ptr<spdlog::logger> logger = spdlog::default_logger();
};

SpdlogLogger::SpdlogLogger() : impl_(std::make_unique<Impl>()) {}
SpdlogLogger::~SpdlogLogger() = default;

void SpdlogLogger::setLevel(LogLevel level) noexcept {
    impl_->logger->set_level(toSpdlogLevel(level));
}

LogLevel SpdlogLogger::level() const noexcept {
    return fromSpdlogLevel(impl_->logger->level());
}

void SpdlogLogger::log(LogLevel level, std::string_view message, std::string_view locationText) {
    impl_->logger->log(toSpdlogLevel(level), "{} [{}]", message, locationText);
}

} // namespace ptiff::detail
