#include <algorithm>
#include <limits>
#include <utility>

#include <ptiff/io/memory_binary_writer.hpp>

namespace ptiff::io {

struct MemoryBinaryWriter::Impl {
    std::vector<std::byte> buffer;
    std::uint64_t position = 0;
};

MemoryBinaryWriter::MemoryBinaryWriter() : impl_(std::make_unique<Impl>()) {}

MemoryBinaryWriter::~MemoryBinaryWriter() = default;

Result<std::size_t> MemoryBinaryWriter::write(std::span<const std::byte> source) {
    if (source.empty()) {
        return source.size();
    }
    // Guard against a position/size overflow before the sum below.
    if (impl_->position > std::numeric_limits<std::uint64_t>::max() - source.size()) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "MemoryBinaryWriter::write: position overflow"});
    }
    const std::uint64_t end = impl_->position + source.size();
    if (end > impl_->buffer.size()) {
        // Appending past the current extent grows the buffer (zero-filling any gap between the
        // previous end and this write's start if they differ, which only happens when seeking
        // within the buffer and writing past it).
        impl_->buffer.resize(static_cast<std::size_t>(end));
    }
    std::copy_n(source.data(),
                source.size(),
                impl_->buffer.begin() + static_cast<std::ptrdiff_t>(impl_->position));
    impl_->position += source.size();
    return source.size();
}

Result<void> MemoryBinaryWriter::seek(std::uint64_t offset) {
    if (offset > impl_->buffer.size()) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument,
                  "MemoryBinaryWriter::seek: offset beyond the current buffer extent"});
    }
    impl_->position = offset;
    return {};
}

Result<std::uint64_t> MemoryBinaryWriter::position() const {
    return impl_->position;
}

Result<void> MemoryBinaryWriter::flush() {
    return {};
}

const std::vector<std::byte>& MemoryBinaryWriter::buffer() const noexcept {
    return impl_->buffer;
}

std::vector<std::byte> MemoryBinaryWriter::takeBuffer() {
    std::vector<std::byte> out;
    out.swap(impl_->buffer);
    impl_->position = 0;
    return out;
}

} // namespace ptiff::io
