#pragma once

// Adapters between ptiff's BinaryReader/BinaryWriter cursor API and OpenEXR's own abstract
// Imf::IStream / Imf::OStream byte-transport interfaces (ImfIO.h). OpenEXR's InputFile /
// OutputFile always address the *whole* underlying stream in absolute offsets (tellg/seekg are
// "bytes from the beginning of the file"), so these adapters pass ptiff's reader/writer through
// unchanged -- no offset translation -- which is why OpenExrBackend always resets its
// reader/writer to offset 0 before constructing one (see openexr_document.hpp).
//
// OpenEXR's IStream::read/OStream::write contract is exception-based (throw on short read / I/O
// error); ptiff's Result<T> is not, so these adapters translate a failed BinaryReader/BinaryWriter
// call into a thrown std::runtime_error. Callers driving an InputFile/OutputFile through these
// adapters must wrap that use in try/catch and translate back into a Result<Error> (OpenEXR itself
// throws Iex-derived exceptions on malformed input, which need the same translation).

#include <cstdint>

#include <ImfIO.h>

#include <ptiff/io/binary_reader.hpp>
#include <ptiff/io/binary_writer.hpp>

namespace ptiff::io::backend::openexr {

/// @brief Imf::IStream over a ptiff BinaryReader.
class BinaryReaderIStream final : public Imf::IStream {
public:
    explicit BinaryReaderIStream(io::BinaryReader& reader);

    bool read(char c[/*n*/], int n) override;
    std::uint64_t tellg() override;
    void seekg(std::uint64_t pos) override;

private:
    io::BinaryReader& reader_;
};

/// @brief Imf::OStream over a ptiff BinaryWriter.
class BinaryWriterOStream final : public Imf::OStream {
public:
    explicit BinaryWriterOStream(io::BinaryWriter& writer);

    void write(const char c[/*n*/], int n) override;
    std::uint64_t tellp() override;
    void seekp(std::uint64_t pos) override;

private:
    io::BinaryWriter& writer_;
};

} // namespace ptiff::io::backend::openexr
