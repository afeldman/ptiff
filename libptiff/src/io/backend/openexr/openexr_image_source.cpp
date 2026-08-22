#include <exception>
#include <span>
#include <utility>

#include <ImfFrameBuffer.h>
#include <ImfHeader.h>
#include <ImfInputFile.h>

#include <ptiff/io/backend/memory/memory_layout.hpp>
#include <ptiff/io/backend/openexr/openexr_image_source.hpp>
#include <ptiff/io/backend/openexr/openexr_stream.hpp>

namespace ptiff::io::backend::openexr {

OpenExrImageSource::OpenExrImageSource(io::BinaryReader& reader, OpenExrImageInfo info)
    : reader_(reader),
      info_(info),
      layout_(io::tile::TileLayout{.tileSize = {.width = info.width, .height = info.height},
                                   .imageWidth = info.width,
                                   .imageHeight = info.height,
                                   .levelCount = 1}) {}

const io::tile::TileLayout& OpenExrImageSource::layout() const noexcept {
    return layout_;
}

Result<void> OpenExrImageSource::ensureLoaded() {
    if (loaded_) {
        return {};
    }

    auto channelNames = channelNamesFor(info_.samplesPerPixel);
    if (!channelNames.has_value()) {
        return std::unexpected(channelNames.error());
    }
    const std::uint32_t bps = memory::bytesPerSample(info_.pixelType);
    auto imfType = toImfPixelType(info_.pixelType);
    if (!imfType.has_value()) {
        return std::unexpected(imfType.error());
    }

    const std::size_t pixelStride = static_cast<std::size_t>(info_.samplesPerPixel) * bps;
    const std::size_t rowStride = static_cast<std::size_t>(info_.width) * pixelStride;
    buffer_.assign(static_cast<std::size_t>(info_.height) * rowStride, std::byte{0});

    // The reader may have been left mid-document by an earlier readHeaderInfo() call (e.g.
    // OpenExrBackend::openImageSource always reads the header first); an Imf::InputFile always
    // needs to see its own document from byte 0.
    auto reset = reader_.seek(0);
    if (!reset.has_value()) {
        return std::unexpected(reset.error());
    }

    try {
        BinaryReaderIStream stream(reader_);
        Imf::InputFile in(stream);
        const auto& dw = in.header().dataWindow();

        Imf::FrameBuffer fb;
        for (std::size_t i = 0; i < channelNames->size(); ++i) {
            char* base = reinterpret_cast<char*>(buffer_.data()) + i * bps;
            fb.insert((*channelNames)[i].c_str(),
                      Imf::Slice(*imfType, base, pixelStride, rowStride));
        }
        in.setFrameBuffer(fb);
        in.readPixels(dw.min.y, dw.max.y);
    } catch (const std::exception& e) {
        return std::unexpected(Error{ErrorCode::InvalidArgument,
                                     std::string{"openexr: pixel read failed: "} + e.what()});
    }

    loaded_ = true;
    return {};
}

Result<io::tile::Tile> OpenExrImageSource::readTile(const io::tile::TileIndex& index) {
    auto region = layout_.regionFor(index);
    if (!region.has_value()) {
        return std::unexpected(region.error());
    }
    auto loaded = ensureLoaded();
    if (!loaded.has_value()) {
        return std::unexpected(loaded.error());
    }
    return io::tile::Tile{ptiff::TileId{0}, index, *region, std::span<const std::byte>{buffer_}};
}

} // namespace ptiff::io::backend::openexr
