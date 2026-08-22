#include <algorithm>
#include <memory>
#include <utility>

#include <ptiff/io/memory_binary_reader.hpp>

namespace ptiff::io {

struct MemoryBinaryReader::Impl {
    std::shared_ptr<const std::vector<std::byte>> data;
    std::uint64_t position = 0;
};

MemoryBinaryReader::MemoryBinaryReader(std::shared_ptr<const std::vector<std::byte>> data)
    : impl_(std::make_unique<Impl>()) {
    impl_->data = std::move(data);
}

MemoryBinaryReader::~MemoryBinaryReader() = default;

Result<std::size_t> MemoryBinaryReader::read(std::span<std::byte> destination) {
    if (destination.empty()) {
        return std::size_t{0};
    }
    const std::size_t available = impl_->data->size() - static_cast<std::size_t>(impl_->position);
    const std::size_t toCopy = std::min(destination.size(), available);
    if (toCopy > 0) {
        std::copy_n(impl_->data->begin() + static_cast<std::ptrdiff_t>(impl_->position),
                    toCopy,
                    destination.begin());
        impl_->position += toCopy;
    }
    return toCopy;
}

Result<void> MemoryBinaryReader::seek(std::uint64_t offset) {
    if (offset > impl_->data->size()) {
        return std::unexpected(Error{ErrorCode::InvalidArgument,
                                     "MemoryBinaryReader::seek: offset beyond the buffer extent"});
    }
    impl_->position = offset;
    return {};
}

Result<std::uint64_t> MemoryBinaryReader::position() const {
    return impl_->position;
}

Result<std::uint64_t> MemoryBinaryReader::size() const {
    return impl_->data->size();
}

} // namespace ptiff::io
