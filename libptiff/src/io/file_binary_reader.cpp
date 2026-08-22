#include <fstream>

#include <ptiff/io/file_binary_reader.hpp>

namespace ptiff::io {

struct FileBinaryReader::Impl {
    std::ifstream stream;
    std::uint64_t size = 0;
};

FileBinaryReader::FileBinaryReader() : impl_(std::make_unique<Impl>()) {}

FileBinaryReader::~FileBinaryReader() = default;

Result<std::unique_ptr<FileBinaryReader>> FileBinaryReader::open(const std::string& path) {
    std::unique_ptr<FileBinaryReader> reader{new FileBinaryReader()};
    reader->impl_->stream.open(path, std::ios::binary | std::ios::in);
    if (!reader->impl_->stream.is_open()) {
        return std::unexpected(
            Error{ErrorCode::NotFound, "FileBinaryReader::open: cannot open file: " + path});
    }

    reader->impl_->stream.seekg(0, std::ios::end);
    const auto end = reader->impl_->stream.tellg();
    if (end < 0) {
        return std::unexpected(
            Error{ErrorCode::NotFound, "FileBinaryReader::open: cannot determine size: " + path});
    }
    reader->impl_->size = static_cast<std::uint64_t>(end);
    reader->impl_->stream.seekg(0, std::ios::beg);

    return reader;
}

Result<std::size_t> FileBinaryReader::read(std::span<std::byte> destination) {
    impl_->stream.read(reinterpret_cast<char*>(destination.data()),
                       static_cast<std::streamsize>(destination.size()));
    if (impl_->stream.bad()) {
        return std::unexpected(Error{ErrorCode::InvalidArgument, "FileBinaryReader::read failed"});
    }
    const auto bytesRead = static_cast<std::size_t>(impl_->stream.gcount());
    impl_->stream.clear();
    return bytesRead;
}

Result<void> FileBinaryReader::seek(std::uint64_t offset) {
    impl_->stream.clear();
    impl_->stream.seekg(static_cast<std::streamoff>(offset), std::ios::beg);
    if (impl_->stream.fail()) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "FileBinaryReader::seek: invalid offset"});
    }
    return {};
}

Result<std::uint64_t> FileBinaryReader::position() const {
    auto pos = impl_->stream.tellg();
    if (pos < 0) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "FileBinaryReader::position: stream error"});
    }
    return static_cast<std::uint64_t>(pos);
}

Result<std::uint64_t> FileBinaryReader::size() const {
    return impl_->size;
}

} // namespace ptiff::io
