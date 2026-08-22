#include <span>
#include <stdexcept>

#include <ptiff/io/backend/openexr/openexr_stream.hpp>

namespace ptiff::io::backend::openexr {

BinaryReaderIStream::BinaryReaderIStream(io::BinaryReader& reader)
    : Imf::IStream("ptiff-embedded.exr"), reader_(reader) {}

bool BinaryReaderIStream::read(char c[], int n) {
    auto span = std::span<std::byte>{reinterpret_cast<std::byte*>(c), static_cast<std::size_t>(n)};
    auto result = reader_.read(span);
    if (!result.has_value() || *result != span.size()) {
        throw std::runtime_error("ptiff: OpenEXR stream read past end of input");
    }
    auto pos = reader_.position();
    auto size = reader_.size();
    if (!pos.has_value() || !size.has_value()) {
        throw std::runtime_error("ptiff: OpenEXR stream position/size unavailable");
    }
    return *pos != *size;
}

std::uint64_t BinaryReaderIStream::tellg() {
    auto pos = reader_.position();
    if (!pos.has_value()) {
        throw std::runtime_error("ptiff: OpenEXR stream tellg failed");
    }
    return *pos;
}

void BinaryReaderIStream::seekg(std::uint64_t pos) {
    auto result = reader_.seek(pos);
    if (!result.has_value()) {
        throw std::runtime_error("ptiff: OpenEXR stream seekg failed");
    }
}

BinaryWriterOStream::BinaryWriterOStream(io::BinaryWriter& writer)
    : Imf::OStream("ptiff-embedded.exr"), writer_(writer) {}

void BinaryWriterOStream::write(const char c[], int n) {
    auto span = std::span<const std::byte>{reinterpret_cast<const std::byte*>(c),
                                           static_cast<std::size_t>(n)};
    auto result = writer_.write(span);
    if (!result.has_value() || *result != span.size()) {
        throw std::runtime_error("ptiff: OpenEXR stream write failed");
    }
}

std::uint64_t BinaryWriterOStream::tellp() {
    auto pos = writer_.position();
    if (!pos.has_value()) {
        throw std::runtime_error("ptiff: OpenEXR stream tellp failed");
    }
    return *pos;
}

void BinaryWriterOStream::seekp(std::uint64_t pos) {
    auto result = writer_.seek(pos);
    if (!result.has_value()) {
        throw std::runtime_error("ptiff: OpenEXR stream seekp failed");
    }
}

} // namespace ptiff::io::backend::openexr
