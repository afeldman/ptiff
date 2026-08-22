#include <exception>

#include <ImfChannelList.h>
#include <ImfFrameBuffer.h>
#include <ImfHeader.h>
#include <ImfOutputFile.h>

#include <ptiff/io/backend/memory/memory_layout.hpp>
#include <ptiff/io/backend/openexr/openexr_image_sink.hpp>
#include <ptiff/io/backend/openexr/openexr_stream.hpp>

namespace ptiff::io::backend::openexr {

OpenExrImageSink::OpenExrImageSink(io::BinaryWriter& writer, OpenExrImageInfo info)
    : writer_(writer),
      info_(info),
      layout_(io::tile::TileLayout{.tileSize = {.width = info.width, .height = info.height},
                                   .imageWidth = info.width,
                                   .imageHeight = info.height,
                                   .levelCount = 1}) {}

const io::tile::TileLayout& OpenExrImageSink::layout() const noexcept {
    return layout_;
}

Result<void> OpenExrImageSink::writeTile(const io::tile::Tile& tile) {
    auto region = layout_.regionFor(tile.index());
    if (!region.has_value()) {
        return std::unexpected(region.error());
    }

    auto channelNames = channelNamesFor(info_.samplesPerPixel);
    if (!channelNames.has_value()) {
        return std::unexpected(channelNames.error());
    }
    auto imfType = toImfPixelType(info_.pixelType);
    if (!imfType.has_value()) {
        return std::unexpected(imfType.error());
    }
    const std::uint32_t bps = memory::bytesPerSample(info_.pixelType);
    const std::size_t pixelStride = static_cast<std::size_t>(info_.samplesPerPixel) * bps;
    const std::size_t rowStride = static_cast<std::size_t>(info_.width) * pixelStride;
    const std::size_t expected = static_cast<std::size_t>(info_.height) * rowStride;
    if (tile.data().size() != expected) {
        return std::unexpected(Error{ErrorCode::InvalidArgument,
                                     "openexr: tile data size does not match image geometry"});
    }

    auto reset = writer_.seek(0);
    if (!reset.has_value()) {
        return std::unexpected(reset.error());
    }

    try {
        Imf::Header header(static_cast<int>(info_.width), static_cast<int>(info_.height));
        header.compression() = Imf::NO_COMPRESSION;
        for (const auto& name : *channelNames) {
            header.channels().insert(name.c_str(), Imf::Channel(*imfType));
        }

        BinaryWriterOStream stream(writer_);
        Imf::OutputFile out(stream, header);

        // OpenEXR's Slice API takes a non-const base pointer for both read and write use; the
        // OutputFile only ever reads through it here, never writes back into our buffer.
        auto* base = const_cast<char*>(reinterpret_cast<const char*>(tile.data().data()));

        Imf::FrameBuffer fb;
        for (std::size_t i = 0; i < channelNames->size(); ++i) {
            fb.insert((*channelNames)[i].c_str(),
                      Imf::Slice(*imfType, base + i * bps, pixelStride, rowStride));
        }
        out.setFrameBuffer(fb);
        out.writePixels(static_cast<int>(info_.height));
    } catch (const std::exception& e) {
        return std::unexpected(Error{ErrorCode::InvalidArgument,
                                     std::string{"openexr: pixel write failed: "} + e.what()});
    }
    return {};
}

} // namespace ptiff::io::backend::openexr
