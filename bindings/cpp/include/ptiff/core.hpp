#pragma once

/// @file core.hpp
/// @brief Core error/result + id types for the `ptiff-cpp` wrapper.
///
/// The C ABI (`libptiff_c`, built from `crates/ptiff-c`) is the single
/// interop layer between `ptiff-cpp` and the Rust core. Every fallible C-ABI
/// function returns `0` on success or a negative [`ptiff_error_code`] on
/// failure. This header maps the stable error surface into the same
/// `ptiff::Error / ErrorCode / Result<T>` contract the deleted C++ reference
/// library exposed, so existing C++ consumers keep their type names.
///
/// The wrapper never throws for recoverable (domain) failures: `Result<T>` is
/// `std::expected<T, Error>`, matching the historical library. Exceptions are
/// reserved for contract violations (a precondition bug in the caller).

#include <cstdint>
#include <expected>
#include <source_location>
#include <string>
#include <string_view>

namespace ptiff {

/// @brief Mirror of the C-ABI `ptiff_error_code` ordering (stable, additive).
enum class ErrorCode {
    NotImplemented,   ///< Recognized but not implemented yet.
    InvalidArgument,  ///< A provided argument/value is malformed or unsupported.
    OutOfRange,       ///< An index/offset/coordinate lies outside the legal range.
    NotFound,         ///< The requested entity does not exist.
    Unknown,          ///< Unspecified failure.
};

/// @brief Translates a negative C-ABI code (as returned by `libptiff_c`) into
///        an `ErrorCode`. `0`/positive input is clamped to `Unknown`.
inline ErrorCode cErrorCodeToEnum(int negative_c_code) noexcept {
    switch (negative_c_code) {
        case 0:  return ErrorCode::NotImplemented;
        case -1: return ErrorCode::InvalidArgument;
        case -2: return ErrorCode::OutOfRange;
        case -3: return ErrorCode::NotFound;
        default: return ErrorCode::Unknown;
    }
}

/// @brief A recoverable, descriptive error (code + message + origin).
class Error {
public:
    Error(ErrorCode code,
            std::string message,
            std::source_location location = std::source_location::current())
        : code_(code), message_(std::move(message)), location_(location) {}

    [[nodiscard]] ErrorCode code() const noexcept { return code_; }
    [[nodiscard]] std::string_view message() const noexcept { return message_; }
    [[nodiscard]] const std::source_location& location() const noexcept { return location_; }

private:
    ErrorCode code_;
    std::string message_;
    std::source_location location_;
};

/// @brief The standard return type for every fallible public API call.
template <class T> using Result = std::expected<T, Error>;

namespace detail {
struct ImageIdTag {};
struct CameraIdTag {};
} // namespace detail

/// @brief Strongly-typed integer id.
template <class Tag> class Id {
public:
    constexpr explicit Id(std::uint64_t value) noexcept : value_(value) {}
    [[nodiscard]] constexpr std::uint64_t value() const noexcept { return value_; }
    friend constexpr bool operator==(const Id&, const Id&) = default;
private:
    std::uint64_t value_;
};

using ImageId = Id<detail::ImageIdTag>;
using CameraId = Id<detail::CameraIdTag>;

} // namespace ptiff
