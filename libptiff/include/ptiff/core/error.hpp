#pragma once

#include <source_location>
#include <string>
#include <string_view>
#include <utility>

namespace ptiff {

/// @brief Category of a recoverable error.
///
/// ErrorCode is the coarse, stable category every @ref ptiff::Error "Error" belongs to. It is
/// deliberately small and does not try to enumerate every possible problem; callers switch on it
/// only to branch on broad failure classes, and use @ref ptiff::Error::message "message()" for
/// the specific description.
///
/// The enum only ever **grows**: new failure modes append a new enumerator and are never allowed
/// to reuse, renumber, or remove an existing one once released -- downstream code may switch on
/// them across versions.
///
/// @section error_code_values Values
///
/// | Enumerator | Meaning |
/// |------------|---------|
/// | `NotImplemented` | The requested operation/backend is recognized but not implemented yet. |
/// | `InvalidArgument` | A provided argument/value is malformed or unsupported. |
/// | `OutOfRange` | An index, offset or coordinate lies outside the legal range. |
/// | `NotFound` | The requested entity (tile, field, row, ...) does not exist. |
/// | `Unknown` | An unspecified failure with no more specific category. |
///
/// @see @ref ptiff::Error "Error", @ref ptiff::Result "Result" for how codes are carried.
enum class ErrorCode {
    NotImplemented,
    InvalidArgument,
    OutOfRange,
    NotFound,
    Unknown,
};

/// @brief A recoverable, descriptive error.
///
/// Error carries a stable @ref ptiff::ErrorCode "category", a human-readable \c message, and the
/// \c std::source_location where the error was produced (captured automatically for faster
/// diagnosis; see \c PTIFF_PRECONDITION for the related contract-violation path). It is a small,
/// stable value built on standard-library types only, so it stays a reliable building block for
/// @ref ptiff::Result "Result<T>" across the entire public API.
///
/// @section error_construction Construction & usage
///
/// @note Error is **not** polymorphic and does not carry third-party types; this keeps it ABI
///       and dependency stable for the whole library.
///
/// Construct an Error with a code and message; the source location defaults to the construction
/// site automatically. Errors often travel inside @ref ptiff::Result "Result<T>", produced with
/// `std::unexpected(Error{...})`:
///
/// @code{.cpp}
/// using ptiff::Error;
/// using ptiff::ErrorCode;
/// Error e(ErrorCode::OutOfRange, "tile index 99 is outside the 4x2 grid");
/// assert(e.code() == ErrorCode::OutOfRange);
/// assert(e.message() == "tile index 99 is outside the 4x2 grid");
/// @endcode
///
/// Consumers switch on `code()` to choose a reaction. The `message()` carries the specifics:
///
/// @code{.cpp}
/// if (auto r = someResult(); !r.has_value()) {
///    if (r.error().code() == ErrorCode::InvalidArgument) handleBadInput();
/// }
/// @endcode
class Error {
public:
    /// @brief Builds an error from a category, message and optional origin.
    ///
    /// @param code     The coarse category (@ref ptiff::ErrorCode "ErrorCode").
    /// @param message  A human-readable description of the failure.
    /// @param location The source location where the error arose. Defaults to the construction
    ///                 site via `std::source_location::current()`.
    Error(ErrorCode code,
          std::string message,
          std::source_location location = std::source_location::current())
        : code_(code), message_(std::move(message)), location_(location) {}

    /// @brief Returns the coarse category of this error.
    ///
    /// @return The @ref ptiff::ErrorCode "ErrorCode" this error was created with.
    [[nodiscard]] ErrorCode code() const noexcept { return code_; }

    /// @brief Returns the human-readable description of this error.
    ///
    /// @return A `std::string_view` over the message; valid for the lifetime of this Error.
    [[nodiscard]] std::string_view message() const noexcept { return message_; }

    /// @brief Returns the source location where this error was produced.
    ///
    /// @return A const reference to the `std::source_location` captured at construction.
    [[nodiscard]] const std::source_location& location() const noexcept { return location_; }

private:
    ErrorCode code_;
    std::string message_;
    std::source_location location_;
};

} // namespace ptiff
