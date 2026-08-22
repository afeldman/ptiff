#pragma once

#include <cstddef>
#include <cstdint>
#include <span>

#include <ptiff/core/result.hpp>
#include <ptiff/export.hpp>

namespace ptiff::io {

/// @brief Abstraction over raw sequential byte output.
///
/// BinaryWriter is the write-side counterpart to @ref ptiff::io::BinaryReader "BinaryReader":
/// it exposes an imperative cursor API (write/seek/position/flush) over any byte transport --
/// a local file, a memory region, a stream, or (planned) a memory-mapped region. Backend
/// implementations write the bytes of a @ref ptiff::io::StorageModel "StorageModel" through this
/// interface, independent of where those bytes finally land.
///
/// As with its read-side mirror, concrete subclasses are the join point where a backend's
/// chosen transport actually moves bytes.
///
/// @section binary_writer_thread_safety Thread-safety
///
/// This interface is **not** thread-safe for the same reason as
/// @ref ptiff::io::BinaryReader "BinaryReader": it holds a mutable write cursor, so concurrent
/// callers must each hold their own instance.
///
/// @section binary_writer_example Example
///
/// @code{.cpp}
/// using ptiff::io::BinaryWriter;
/// using ptiff::io::FileBinaryWriter;
///
/// // Concrete writers are obtained from a factory (here: a local file writer) and consumed
/// // through the abstract interface below.
/// auto writer = FileBinaryWriter::create("out.ptiff");
/// assert(writer.has_value());
/// std::unique_ptr<BinaryWriter> base = std::move(*writer);
/// assert(base->position().has_value());
/// @endcode
///
/// @see @ref ptiff::io::BinaryReader "BinaryReader" (mirror, read side),
///      @ref ptiff::io::FileBinaryWriter "FileBinaryWriter" (a concrete implementation),
///      @ref ptiff::io::StorageBackend "StorageBackend".
class PTIFF_EXPORT BinaryWriter {
public:
    virtual ~BinaryWriter() = default;
    BinaryWriter(const BinaryWriter&) = delete;
    BinaryWriter& operator=(const BinaryWriter&) = delete;
    BinaryWriter(BinaryWriter&&) = delete;
    BinaryWriter& operator=(BinaryWriter&&) = delete;

    /// @brief Writes `source.size()` bytes starting at the current cursor.
    ///
    /// @param source The span of bytes to write at the current cursor position.
    /// @return The number of bytes actually written on success.
    ///
    /// @code{.cpp}
    /// std::array<std::byte, 4> data{};
    /// auto n = wr.write(data);
    /// assert(n.has_value() && *n == data.size());
    /// @endcode
    [[nodiscard]] virtual Result<std::size_t> write(std::span<const std::byte> source) = 0;
    /// @brief Moves the write cursor to an absolute byte \p offset.
    ///
    /// @param offset The absolute, 0-based byte offset to seek to.
    /// @return `Result<void>` success on success, or
    ///         @ref ptiff::ErrorCode::OutOfRange "OutOfRange" if \p offset is beyond the output.
    /// @see @ref position for the current-cursor query.
    [[nodiscard]] virtual Result<void> seek(std::uint64_t offset) = 0;
    /// @brief Reports the current absolute byte offset of the write cursor.
    /// @return The current 0-based write cursor position on success.
    [[nodiscard]] virtual Result<std::uint64_t> position() const = 0;
    /// @brief Pushes any buffered output to the underlying transport.
    ///
    /// @return `Result<void>` success once pending buffered bytes are committed to the transport.
    ///
    /// @note A no-op for unbuffered transports; always safe to call before releasing the writer.
    [[nodiscard]] virtual Result<void> flush() = 0;

protected:
    BinaryWriter() = default;
};

} // namespace ptiff::io
