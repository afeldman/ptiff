#pragma once

#include <expected>

#include <ptiff/core/error.hpp>

namespace ptiff {

/// @brief Standard return type for every fallible public API call.
///
/// `Result<T>` is an alias of `std::expected<T, ptiff::Error>`. It is the single, uniform way
/// the library reports **recoverable** (domain) failure: a function either *has* a `T` on
/// success or carries a @ref ptiff::Error "Error" describing why it failed. Domain/recoverable
/// failures are values, never exceptions -- see `docs/CODING_GUIDELINES.md` for the full
/// error-model rationale. Exceptions are reserved for *contract violations* (for example a
/// precondition failure, see \c PTIFF_PRECONDITION).
///
/// @section result_usage Usage
///
/// Consumers branch on `has_value()`, then read the value or the error:
///
/// @code{.cpp}
/// using ptiff::Result;
/// using ptiff::Error;
/// using ptiff::ErrorCode;
///
/// Result<int> divide(int a, int b) {
///     if (b == 0) {
///         return std::unexpected(Error{ErrorCode::InvalidArgument, "divide by zero"});
///     }
///     return a / b; // OK: implicit construction of the expected<T, Error> value side
/// }
///
/// auto ok  = divide(10, 2);
/// assert(ok.has_value() && *ok == 5);
///
/// auto bad = divide(10, 0);
/// assert(!bad.has_value());
/// assert(bad.error().code() == ErrorCode::InvalidArgument);
///
/// if (auto r = divide(7, 3); r.has_value()) {
///     std::cout << "7/3 = " << r.value() << '\n';
/// } else {
///     std::cerr << "division failed: " << r.error().message() << '\n';
/// }
/// @endcode
///
/// @section result_producing Producing results inside the library
///
/// Implementation code builds the error side with `std::unexpected(Error{...})`, and can also
/// move an error out of a `Result` with the `.error()` accessor before returning it further up:
///
/// @code{.cpp}
/// Result<int> inner() { return std::unexpected(Error{ErrorCode::NotFound, "entity missing"}); }
/// Result<int> outer() {
///     auto r = inner();
///     if (!r) return std::unexpected(r.error()); // transport the same error upward
///     return *r + 1;
/// }
/// @endcode
///
/// @tparam T The value type carried on success. `void` is supported by `std::expected<void,
///         Error>` and is the common shape for store-style operations that only report failure.
///
/// @note `Result<T>` is *not* the same as `T`; treat it as a sum type (value xor `Error`).
///       `std::expected`'s `bool` conversion reflects `has_value()`.
template <class T> using Result = std::expected<T, Error>;

} // namespace ptiff
