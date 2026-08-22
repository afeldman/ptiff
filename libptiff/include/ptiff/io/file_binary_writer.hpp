#pragma once

#include <cstdint>
#include <memory>
#include <span>
#include <string>

#include <ptiff/core/result.hpp>
#include <ptiff/export.hpp>
#include <ptiff/io/binary_writer.hpp>

namespace ptiff::io {

/// @brief Concrete @ref ptiff::io::BinaryWriter "BinaryWriter" over a local file.
///
/// FileBinaryWriter creates a local file for writing in binary mode (truncating any existing file
/// at the given path) and moves bytes through it with the standard
/// @ref ptiff::io::BinaryWriter "BinaryWriter" cursor API (write/seek/position/flush). It is
/// backend-neutral -- any @ref ptiff::io::StorageBackend "StorageBackend" writing to local files
/// can use it, not just the TIFF backend.
///
/// @section file_binary_writer_thread Thread-safety
///
/// Not thread-safe, under the same contract as its
/// @ref ptiff::io::BinaryWriter "BinaryWriter" base class: it holds a mutable write cursor.
///
/// @section file_binary_writer_example Example
///
/// @code{.cpp}
/// using ptiff::io::FileBinaryWriter;
///
/// auto w = FileBinaryWriter::create("out.ptiff");
/// assert(w.has_value());
/// assert(w.value()->position().value() == 0);
/// @endcode
///
/// @see @ref ptiff::io::BinaryWriter "BinaryWriter",
///      @ref ptiff::io::FileBinaryReader "FileBinaryReader".
class PTIFF_EXPORT FileBinaryWriter final : public BinaryWriter {
public:
    /// @brief Creates (and truncates, if it already exists) \p path for writing.
    ///
    /// @param path The local filesystem path to create/replace.
    /// @return A `std::unique_ptr<FileBinaryWriter>` on success, or
    ///         @ref ptiff::ErrorCode::InvalidArgument "InvalidArgument" if the file cannot be
    ///         created (e.g. the containing directory doesn't exist).
    ///
    /// @code{.cpp}
    /// auto w = FileBinaryWriter::create("out.ptiff");
    /// assert(w.has_value());
    /// @endcode
    [[nodiscard]] static Result<std::unique_ptr<FileBinaryWriter>> create(const std::string& path);

    /// @brief Closes the underlying file stream.
    ~FileBinaryWriter() override;

    /// @brief Writes \p source's bytes starting at the current cursor.
    /// @return The number of bytes actually written (always `source.size()` on success), or
    ///         @ref ptiff::ErrorCode::InvalidArgument "InvalidArgument" on a write failure.
    [[nodiscard]] Result<std::size_t> write(std::span<const std::byte> source) override;
    /// @brief Moves the write cursor to an absolute byte \p offset.
    /// @return `Result<void>` success on success, or
    ///         @ref ptiff::ErrorCode::InvalidArgument "InvalidArgument" if \p offset is invalid
    ///         for the stream.
    [[nodiscard]] Result<void> seek(std::uint64_t offset) override;
    /// @brief Reports the current absolute byte offset of the write cursor.
    /// @return The current 0-based cursor position on success, or
    ///         @ref ptiff::ErrorCode::InvalidArgument "InvalidArgument" on a stream error.
    [[nodiscard]] Result<std::uint64_t> position() const override;
    /// @brief Flushes any buffered output to the underlying file.
    /// @return `Result<void>` success once pending bytes are committed, or
    ///         @ref ptiff::ErrorCode::InvalidArgument "InvalidArgument" on a flush failure.
    [[nodiscard]] Result<void> flush() override;

private:
    FileBinaryWriter();

    struct Impl;
    std::unique_ptr<Impl> impl_;
};

} // namespace ptiff::io
