#pragma once

#include <cstdlib>
#include <iostream>
#include <source_location>

namespace ptiff::detail {

/// @cond INTERNAL
// Shared implementation behind PTIFF_PRECONDITION. Prints the violated expression and the
// provenance, then terminates the process with std::abort(). Never called directly by user code.
[[noreturn]] inline void failPrecondition(const char* expression,
                                          const std::source_location& location) {
    std::cerr << "PTIFF precondition violated: " << expression << " at " << location.file_name()
              << ':' << location.line() << '\n';
    std::abort();
}
/// @endcond

} // namespace ptiff::detail

/// @brief Asserts a programmer-invariant (a precondition) known to hold at this point.
///
/// Unlike the C `assert()` macro, `PTIFF_PRECONDITION` is **NOT compiled out under `NDEBUG`**:
/// a violated precondition is a bug in the program's logic, not a routine situation to silently
/// skip in release builds. When the condition is false, the macro prints the failed expression
/// and its source location, then terminates the process via `std::abort()`.
///
/// @section precondition_when_to_use When to use
///
/// Use it to express *contracts*: "the caller ensured `size >= 2`", "index is valid", "pointer
/// is non-null". These are bugs if they ever fail. For *expected, recoverable* failure (missing
/// file, bad user input, out-of-range tile index that a caller can legitimately hit), use
/// @ref ptiff::Result "Result<T>" + @ref ptiff::Error "Error" instead -- see the project's
/// error-model guidelines.
///
/// @section precondition_example Example
///
/// @code{.cpp}
/// #include <ptiff/core/precondition.hpp>
///
/// int readAt(const char* buffer, std::size_t size, std::size_t i) {
///     PTIFF_PRECONDITION(buffer != nullptr);  // contract: caller must pass a live buffer
///     PTIFF_PRECONDITION(i < size);           // contract: index in range
///     return buffer[i];
/// }
/// @endcode
///
/// @param condition The invariant expression to check. If it evaluates to false, the program
///                  aborts after reporting the violation on `stderr`.
///
/// @warning This terminates the process on failure -- do **not** use it for input validation or
///          any condition you expect to (and want to) handle at runtime. Those belong in
///          @ref ptiff::Result "Result<T>".
///
/// @see @ref ptiff::Result "Result", @ref ptiff::Error "Error", @ref ptiff::ErrorCode "ErrorCode".
#define PTIFF_PRECONDITION(condition)                                                              \
    do {                                                                                           \
        if (!(condition)) {                                                                        \
            ::ptiff::detail::failPrecondition(#condition, std::source_location::current());        \
        }                                                                                          \
    } while (false)
