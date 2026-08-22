#pragma once

#include <cstddef>
#include <cstdint>
#include <span>

#include <ptiff/core/result.hpp>
#include <ptiff/export.hpp>

namespace ptiff::io {

/// @brief Abstraction over raw sequential byte input.
///
/// A BinaryReader models any read-only byte transport -- a local file, an in-memory region, a
/// stream, or (planned) a memory-mapped region. It exposes an imperative cursor API
/// (read/seek/position) on top of the underlying transport, which is exactly what
/// @ref ptiff::io::StorageBackend "StorageBackend" implementations need to pull bytes out of a
/// file without caring about *how* those bytes are eventually produced.
///
/// Concrete subclasses are the join point where a backend's chosen transport actually moves
/// bytes; write the byte transport once in a subclass and reuse it across every backend that
/// reads from the same kind of input.
///
/// @section binary_reader_thread_safety Thread-safety
///
/// This interface is **not** thread-safe: it holds a mutable read cursor, so concurrent callers
/// must each hold their own instance. Sharing one reader across threads is use-after-data-race.
///
/// @section binary_reader_example Example
///
/// @code{.cpp}
/// using ptiff::io::BinaryReader;
/// using ptiff::io::FileBinaryReader;
///
/// // Concrete readers are obtained from a factory (here: a local file reader) and
/// // consumed through the abstract interface below.
/// auto reader = FileBinaryReader::open("image.ptiff");
/// assert(reader.has_value());
///
/// // Hand the owning pointer to code that only knows the abstract BinaryReader.
/// std::unique_ptr<BinaryReader> base = std::move(*reader);
/// assert(base->size().has_value());
/// @endcode
///
/// @see @ref ptiff::io::BinaryWriter "BinaryWriter" (mirror, write side),
///      @ref ptiff::io::FileBinaryReader "FileBinaryReader" (a concrete implementation),
///      @ref ptiff::io::StorageBackend "StorageBackend".
class PTIFF_EXPORT BinaryReader {
public:
    virtual ~BinaryReader() = default;
    BinaryReader(const BinaryReader&) = delete;
    BinaryReader& operator=(const BinaryReader&) = delete;
    BinaryReader(BinaryReader&&) = delete;
    BinaryReader& operator=(BinaryReader&&) = delete;

    /// @brief Reads up to `destination.size()` bytes into \p destination.
    ///
    /// @param destination The span to fill with bytes read from the current cursor position.
    /// @return The number of bytes actually read on success. May be less than requested at
    ///         end-of-input (and `0` at end-of-input); a negative-sized result never occurs.
    ///
    /// @code{.cpp}
    /// std::array<std::byte, 4> buf{};
    /// // `rd` is some BinaryReader; reads up to 4 bytes from the current cursor.
    /// auto n = rd.read(buf);
    /// assert(n.has_value() && *n <= buf.size());
    /// @endcode
    [[nodiscard]] virtual Result<std::size_t> read(std::span<std::byte> destination) = 0;
    /// @brief Moves the read cursor to an absolute byte \p offset.
    ///
    /// @param offset The absolute, 0-based byte offset to seek to.
    /// @return `Result<void>` success on success, or
    ///         @ref ptiff::ErrorCode::OutOfRange "OutOfRange" if \p offset is beyond the input.
    /// @see @ref position for the current-cursor query.
    [[nodiscard]] virtual Result<void> seek(std::uint64_t offset) = 0;
    /// @brief Reports the current absolute byte offset of the read cursor.
    /// @return The current 0-based read cursor position on success.
    [[nodiscard]] virtual Result<std::uint64_t> position() const = 0;
    /// @brief Reports the total size in bytes of the underlying input, if known.
    /// @return The input's total size in bytes on success, or
    ///         @ref ptiff::ErrorCode::Unknown "Unknown" if the transport cannot determine it
    ///         (e.g. an unbounded stream).
    [[nodiscard]] virtual Result<std::uint64_t> size() const = 0;

protected:
    BinaryReader() = default;
};

} // namespace ptiff::io
