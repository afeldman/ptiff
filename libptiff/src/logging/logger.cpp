#include <fmt/format.h>

#include <ptiff/logging/detail/spdlog_logger.hpp>
#include <ptiff/logging/logger.hpp>

namespace ptiff {

struct Logger::Impl {
    detail::SpdlogLogger backend;
};

Logger::Logger() : impl_(std::make_unique<Impl>()) {}
Logger::~Logger() = default;

Logger& Logger::instance() {
    static Logger logger;
    return logger;
}

void Logger::setLevel(LogLevel level) noexcept {
    impl_->backend.setLevel(level);
}

LogLevel Logger::level() const noexcept {
    return impl_->backend.level();
}

void Logger::log(LogLevel level, std::string_view message, std::source_location location) {
    const std::string locationText = fmt::format("{}:{}", location.file_name(), location.line());
    impl_->backend.log(level, message, locationText);
}

} // namespace ptiff
