#pragma once

#include <memory>
#include <source_location>
#include <string_view>

#include <ptiff/export.hpp>

namespace ptiff {

/// @brief Log severity threshold.
///
/// Levels are ordered: a @ref ptiff::Logger "Logger" set to a given level emits (and forwards to
/// the sink) only messages at or above that level.
enum class LogLevel {
    Trace,    ///< Finest-grained diagnostic detail.
    Debug,    ///< Debug/diagnostic messages.
    Info,     ///< Normal informational messages.
    Warn,     ///< Warning messages (recoverable but noteworthy).
    Error,    ///< Error messages (operation failed but the process continues).
    Critical, ///< Critical messages (process may be in an unsafe state).
    Off,      ///< Disable all logging.
};

/// @brief Process-wide logging facade.
///
/// A singleton, thread-safe logging service giving the library a single place to report
/// diagnostics. Never exposes the backing logging library (spdlog) through this header -- see
/// `src/logging/detail/spdlog_logger.hpp` for the backend.
///
/// @section logger_example Example
///
/// @code{.cpp}
/// using ptiff::Logger;
/// using ptiff::LogLevel;
///
/// Logger::instance().setLevel(LogLevel::Debug);
/// Logger::instance().log(LogLevel::Info, "opening dataset");
///
/// auto lvl = Logger::instance().level();  // LogLevel::Debug
/// @endcode
///
/// @note The level is process-wide: there is exactly one Logger instance per process.
class PTIFF_EXPORT Logger {
public:
    /// @brief Returns the process-wide logger instance.
    ///
    /// @return A reference to the singleton.
    static Logger& instance();

    Logger(const Logger&) = delete;
    Logger& operator=(const Logger&) = delete;
    Logger(Logger&&) = delete;
    Logger& operator=(Logger&&) = delete;
    ~Logger();

    /// @brief Sets the log threshold level; messages below it are suppressed.
    ///
    /// @param level The new threshold @ref ptiff::LogLevel "LogLevel".
    void setLevel(LogLevel level) noexcept;

    /// @brief Returns the current log threshold level.
    ///
    /// @return The current @ref ptiff::LogLevel "LogLevel".
    [[nodiscard]] LogLevel level() const noexcept;

    /// @brief Emits a message at the given level.
    ///
    /// The message is only forwarded to the sink if \p level is at or above the configured
    /// threshold. The source location defaults to the call site.
    ///
    /// @param level    The severity of the message.
    /// @param message  The message text.
    /// @param location The source location of the call (defaults to the invocation site).
    void log(LogLevel level,
             std::string_view message,
             std::source_location location = std::source_location::current());

private:
    Logger();

    struct Impl;
    std::unique_ptr<Impl> impl_;
};

} // namespace ptiff
