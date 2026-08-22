#include <fstream>

#include <ptiff/io/file_binary_writer.hpp>

namespace ptiff::io {

struct FileBinaryWriter::Impl {
    std::ofstream stream;
};

FileBinaryWriter::FileBinaryWriter() : impl_(std::make_unique<Impl>()) {}

FileBinaryWriter::~FileBinaryWriter() = default;

Result<std::unique_ptr<FileBinaryWriter>> FileBinaryWriter::create(const std::string& path) {
    std::unique_ptr<FileBinaryWriter> writer{new FileBinaryWriter()};
    writer->impl_->stream.open(path, std::ios::binary | std::ios::out | std::ios::trunc);
    if (!writer->impl_->stream.is_open()) {
        return std::unexpected(Error{ErrorCode::InvalidArgument,
                                     "FileBinaryWriter::create: cannot create file: " + path});
    }
    return writer;
}

Result<std::size_t> FileBinaryWriter::write(std::span<const std::byte> source) {
    impl_->stream.write(reinterpret_cast<const char*>(source.data()),
                        static_cast<std::streamsize>(source.size()));
    if (impl_->stream.fail()) {
        return std::unexpected(Error{ErrorCode::InvalidArgument, "FileBinaryWriter::write failed"});
    }
    return source.size();
}

Result<void> FileBinaryWriter::seek(std::uint64_t offset) {
    impl_->stream.seekp(static_cast<std::streamoff>(offset), std::ios::beg);
    if (impl_->stream.fail()) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "FileBinaryWriter::seek: invalid offset"});
    }
    return {};
}

Result<std::uint64_t> FileBinaryWriter::position() const {
    auto pos = impl_->stream.tellp();
    if (pos < 0) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "FileBinaryWriter::position: stream error"});
    }
    return static_cast<std::uint64_t>(pos);
}

Result<void> FileBinaryWriter::flush() {
    impl_->stream.flush();
    if (impl_->stream.fail()) {
        return std::unexpected(Error{ErrorCode::InvalidArgument, "FileBinaryWriter::flush failed"});
    }
    return {};
}

} // namespace ptiff::io
