#pragma once

#include <cstddef>
#include <cstdint>
#include <memory>
#include <span>
#include <vector>

#include <ptiff/core/result.hpp>
#include <ptiff/export.hpp>
#include <ptiff/io/binary_writer.hpp>

namespace ptiff::io {

/// @brief Concrete @ref ptiff::io::BinaryWriter "BinaryWriter" over an in-memory byte buffer.
///
/// MemoryBinaryWriter accumulates written bytes into an internally owned
/// `std::vector<std::byte>` and exposes that buffer through @ref buffer / @ref takeBuffer so the
/// result can be handed back to a @ref ptiff::io::MemoryBinaryReader "MemoryBinaryReader" without
/// touching the filesystem. It implements the standard
/// @ref ptiff::io::BinaryWriter "BinaryWriter" cursor API (write/seek/position/flush).
///
/// @section memory_binary_writer_seek Seek semantics
///
/// `seek` may move the cursor to any offset **within the current buffer extent** (0..size()
/// inclusive); seeking past the end is rejected with @ref ptiff::ErrorCode::InvalidArgument
/// "InvalidArgument". `write` always lands at the current cursor: writing at the cursor when it
/// equals `size()` **appends** and grows the buffer, while writing at a position already within
/// the buffer overwrites those bytes in place (zero-filling any gap between the previous extent
/// and the write only in the seek-past-end case, which seek prevents). This matches the
/// in-memory transport's need to grow a write stream while guarding against a wild seek followed
/// by a giant write.
///
/// @section memory_binary_writer_thread Thread-safety
///
/// Not thread-safe, under the same contract as its
/// @ref ptiff::io::BinaryWriter "BinaryWriter" base class: it holds a mutable write cursor.
///
/// @section memory_binary_writer_example Example
///
/// @code{.cpp}
/// using ptiff::io::MemoryBinaryWriter;
///
/// MemoryBinaryWriter w;
/// const std::array<std::byte, 3> data{};
/// assert(w.write(data).has_value());
/// assert(w.buffer().size() == 3);
/// auto bytes = w.takeBuffer();   // move the buffer out
/// @endcode
///
/// @see @ref ptiff::io::BinaryWriter "BinaryWriter",
///      @ref ptiff::io::MemoryBinaryReader "MemoryBinaryReader".
class PTIFF_EXPORT MemoryBinaryWriter final : public BinaryWriter {
public:
    /// @brief Constructs an empty in-memory writer (empty buffer, cursor at 0).
    MemoryBinaryWriter();
    /// @brief Destroys the writer and its owned buffer.
    ~MemoryBinaryWriter() override;

    /// @brief Copies \p source's bytes into the buffer at the current cursor.
    /// @return The number of bytes written (always `source.size()` on success), or
    ///         @ref ptiff::ErrorCode::InvalidArgument "InvalidArgument" if a position overflow
    ///         would occur (see the seek-semantics note for when the buffer grows).
    [[nodiscard]] Result<std::size_t> write(std::span<const std::byte> source) override;
    /// @brief Moves the write cursor to an absolute byte \p offset.
    /// @return `Result<void>` success on success, or
    ///         @ref ptiff::ErrorCode::InvalidArgument "InvalidArgument" if \p offset is beyond
    ///         the current buffer extent.
    [[nodiscard]] Result<void> seek(std::uint64_t offset) override;
    /// @brief Reports the current absolute byte offset of the write cursor.
    /// @return The current 0-based cursor position on success.
    [[nodiscard]] Result<std::uint64_t> position() const override;
    /// @brief Flushes buffered output.
    ///
    /// @return `Result<void>` always success: in-memory output needs no explicit flush.
    [[nodiscard]] Result<void> flush() override;

    /// @brief Returns a const reference to the accumulated buffer.
    /// @return The writer's internal `std::vector<std::byte>` (valid for the writer's lifetime,
    ///         reallocated by @ref write).
    [[nodiscard]] const std::vector<std::byte>& buffer() const noexcept;

    /// @brief Moves the accumulated buffer out of the writer, leaving an empty buffer / cursor 0.
    /// @return The writer's internal buffer, moved.
    [[nodiscard]] std::vector<std::byte> takeBuffer();

private:
    struct Impl;
    std::unique_ptr<Impl> impl_;
};

} // namespace ptiff::io
