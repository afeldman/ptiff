#pragma once

#include <cstddef>
#include <cstdint>
#include <memory>
#include <span>
#include <vector>

#include <ptiff/core/result.hpp>
#include <ptiff/export.hpp>
#include <ptiff/io/binary_reader.hpp>

namespace ptiff::io {

/// @brief Concrete @ref ptiff::io::BinaryReader "BinaryReader" over an in-memory byte buffer.
///
/// MemoryBinaryReader reads from a fixed, read-only byte buffer (typically the buffer a
/// @ref ptiff::io::MemoryBinaryWriter "MemoryBinaryWriter" produced, shared via
/// `std::shared_ptr<const std::vector<std::byte>>`) using the standard
/// @ref ptiff::io::BinaryReader "BinaryReader" cursor API (read/seek/position/size). It keeps the
/// buffer alive for as long as the reader lives, so no external lifetime management is needed.
///
/// @section memory_binary_reader_thread Thread-safety
///
/// Not thread-safe, under the same contract as its
/// @ref ptiff::io::BinaryReader "BinaryReader" base class: it holds a mutable read cursor.
///
/// @section memory_binary_reader_example Example
///
/// @code{.cpp}
/// using ptiff::io::MemoryBinaryReader;
///
/// auto data = std::make_shared<const std::vector<std::byte>>(...);
/// MemoryBinaryReader r(data);
/// assert(r.size().value() == data->size());
/// @endcode
///
/// @see @ref ptiff::io::BinaryReader "BinaryReader",
///      @ref ptiff::io::MemoryBinaryWriter "MemoryBinaryWriter".
class PTIFF_EXPORT MemoryBinaryReader final : public BinaryReader {
public:
    /// @brief Constructs a reader over \p data, sharing ownership of the buffer.
    ///
    /// @param data The read-only buffer to read from; ownership is shared, so the buffer stays
    ///             alive while this reader exists.
    MemoryBinaryReader(std::shared_ptr<const std::vector<std::byte>> data);
    /// @brief Destroys the reader, releasing its shared ownership of the buffer.
    ~MemoryBinaryReader() override;

    /// @brief Reads up to `destination.size()` bytes into \p destination.
    /// @return The number of bytes actually read on success (fewer at end-of-input), or
    ///         @ref ptiff::ErrorCode::InvalidArgument "InvalidArgument" on a hard error.
    [[nodiscard]] Result<std::size_t> read(std::span<std::byte> destination) override;
    /// @brief Moves the read cursor to an absolute byte \p offset.
    /// @return `Result<void>` success on success, or
    ///         @ref ptiff::ErrorCode::InvalidArgument "InvalidArgument" if \p offset is beyond
    ///         the buffer extent.
    [[nodiscard]] Result<void> seek(std::uint64_t offset) override;
    /// @brief Reports the current absolute byte offset of the read cursor.
    /// @return The current 0-based cursor position on success.
    [[nodiscard]] Result<std::uint64_t> position() const override;
    /// @brief Reports the buffer's total size in bytes.
    /// @return The buffer size captured at construction.
    [[nodiscard]] Result<std::uint64_t> size() const override;

private:
    struct Impl;
    std::unique_ptr<Impl> impl_;
};

} // namespace ptiff::io
