#include <exception>

#include <ImfChannelList.h>
#include <ImfHeader.h>
#include <ImfInputFile.h>
#include <ImfOutputFile.h>

#include <ptiff/io/backend/memory/memory_layout.hpp>
#include <ptiff/io/backend/openexr/openexr_document.hpp>
#include <ptiff/io/backend/openexr/openexr_stream.hpp>

namespace ptiff::io::backend::openexr {

namespace {

std::string pixelTypeToString(ptiff::PixelType type) {
    switch (type) {
    case ptiff::PixelType::Float32:
        return "Float32";
    case ptiff::PixelType::UInt32:
        return "UInt32";
    default:
        return "Float32"; // unreachable: callers only pass validated types
    }
}

} // namespace

Result<Imf::PixelType> toImfPixelType(ptiff::PixelType type) {
    switch (type) {
    case ptiff::PixelType::Float32:
        return Imf::FLOAT;
    case ptiff::PixelType::UInt32:
        return Imf::UINT;
    default:
        return std::unexpected(
            Error{ErrorCode::InvalidArgument,
                  "openexr: only Float32 and UInt32 pixel types are supported this phase"});
    }
}

Result<ptiff::PixelType> fromImfPixelType(Imf::PixelType type) {
    switch (type) {
    case Imf::FLOAT:
        return ptiff::PixelType::Float32;
    case Imf::UINT:
        return ptiff::PixelType::UInt32;
    default:
        return std::unexpected(
            Error{ErrorCode::InvalidArgument,
                  "openexr: only Float32 and UInt32 pixel types are supported this phase"});
    }
}

Result<std::vector<std::string>> channelNamesFor(std::uint32_t samplesPerPixel) {
    switch (samplesPerPixel) {
    case 1:
        return std::vector<std::string>{"Y"};
    case 3:
        return std::vector<std::string>{"R", "G", "B"};
    case 4:
        return std::vector<std::string>{"R", "G", "B", "A"};
    default:
        return std::unexpected(
            Error{ErrorCode::InvalidArgument,
                  "openexr: only samplesPerPixel 1 (Y), 3 (RGB) or 4 (RGBA) are supported"});
    }
}

Result<OpenExrImageInfo> imageInfoFromModel(const StorageModel& model) {
    auto width = memory::requiredU32Field(model, "imageWidth");
    if (!width.has_value()) {
        return std::unexpected(width.error());
    }
    auto height = memory::requiredU32Field(model, "imageHeight");
    if (!height.has_value()) {
        return std::unexpected(height.error());
    }
    auto spp = memory::requiredU32Field(model, "samplesPerPixel");
    if (!spp.has_value()) {
        return std::unexpected(spp.error());
    }
    auto channelNames = channelNamesFor(*spp);
    if (!channelNames.has_value()) {
        return std::unexpected(channelNames.error());
    }

    auto pixelTypeField = model.field("pixelType");
    if (!pixelTypeField.has_value()) {
        return std::unexpected(Error{ErrorCode::InvalidArgument, "openexr: missing pixelType"});
    }
    auto pixelType = memory::parsePixelType(*pixelTypeField);
    if (!pixelType.has_value()) {
        return std::unexpected(pixelType.error());
    }
    if (!toImfPixelType(*pixelType).has_value()) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument,
                  "openexr: only Float32 and UInt32 pixel types are supported this phase"});
    }

    // tileWidth/tileHeight are optional; if present they must describe the same single
    // whole-image tile this backend always uses (genuine multi-tile OpenEXR is deferred).
    auto tileWidth = model.field("tileWidth");
    if (tileWidth.has_value() && *tileWidth != std::to_string(*width)) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument,
                  "openexr: tileWidth must equal imageWidth (whole-image tile only)"});
    }
    auto tileHeight = model.field("tileHeight");
    if (tileHeight.has_value() && *tileHeight != std::to_string(*height)) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument,
                  "openexr: tileHeight must equal imageHeight (whole-image tile only)"});
    }
    auto compression = model.field("compression");
    if (compression.has_value() && *compression != "None") {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument,
                  "openexr: only uncompressed (compression==None) images are supported"});
    }

    return OpenExrImageInfo{
        .width = *width, .height = *height, .samplesPerPixel = *spp, .pixelType = *pixelType};
}

StorageModel modelFromImageInfo(const OpenExrImageInfo& info) {
    StorageModel model;
    model.setField("imageWidth", std::to_string(info.width));
    model.setField("imageHeight", std::to_string(info.height));
    model.setField("tileWidth", std::to_string(info.width));
    model.setField("tileHeight", std::to_string(info.height));
    model.setField("samplesPerPixel", std::to_string(info.samplesPerPixel));
    model.setField("pixelType", pixelTypeToString(info.pixelType));
    model.setField("compression", "None");
    return model;
}

Result<void> writeHeaderOnly(const StorageModel& model, io::BinaryWriter& writer) {
    auto info = imageInfoFromModel(model);
    if (!info.has_value()) {
        return std::unexpected(info.error());
    }
    auto channelNames = channelNamesFor(info->samplesPerPixel);
    if (!channelNames.has_value()) {
        return std::unexpected(channelNames.error());
    }
    auto imfType = toImfPixelType(info->pixelType);
    if (!imfType.has_value()) {
        return std::unexpected(imfType.error());
    }
    auto reset = writer.seek(0);
    if (!reset.has_value()) {
        return std::unexpected(reset.error());
    }

    try {
        Imf::Header header(static_cast<int>(info->width), static_cast<int>(info->height));
        header.compression() = Imf::NO_COMPRESSION;
        for (const auto& name : *channelNames) {
            header.channels().insert(name.c_str(), Imf::Channel(*imfType));
        }
        BinaryWriterOStream stream(writer);
        Imf::OutputFile out(stream, header);
        // Deliberately no writePixels() call: this writes a real, structurally valid .exr
        // header + (empty) chunk-offset table only, matching serializeModel's "metadata, no
        // pixel bytes" contract elsewhere in this codebase (see PDS4/ISIS). OpenEXR documents
        // that destroying an OutputFile before all scanlines are written yields an incomplete
        // file; it does not throw, and the header remains readable (verified empirically).
    } catch (const std::exception& e) {
        return std::unexpected(Error{ErrorCode::InvalidArgument,
                                     std::string{"openexr: header write failed: "} + e.what()});
    }
    return {};
}

Result<OpenExrImageInfo> readHeaderInfo(io::BinaryReader& reader) {
    // Self-describing from byte 0; always start from the document origin, robust against a
    // prior read (deserializeModel / openImageSource may both be called on the same reader).
    auto reset = reader.seek(0);
    if (!reset.has_value()) {
        return std::unexpected(reset.error());
    }

    try {
        BinaryReaderIStream stream(reader);
        Imf::InputFile in(stream);
        const auto& dw = in.header().dataWindow();
        const auto width = static_cast<std::uint32_t>(dw.max.x - dw.min.x + 1);
        const auto height = static_cast<std::uint32_t>(dw.max.y - dw.min.y + 1);

        const auto& channels = in.header().channels();
        std::uint32_t spp = 0;
        if (channels.findChannel("Y") != nullptr && channels.findChannel("R") == nullptr) {
            spp = 1;
        } else if (channels.findChannel("A") != nullptr) {
            spp = 4;
        } else if (channels.findChannel("R") != nullptr) {
            spp = 3;
        } else {
            return std::unexpected(
                Error{ErrorCode::InvalidArgument, "openexr: unsupported channel layout"});
        }

        auto channelNames = channelNamesFor(spp);
        if (!channelNames.has_value()) {
            return std::unexpected(channelNames.error());
        }
        const Imf::Channel* first = channels.findChannel(channelNames->front());
        if (first == nullptr) {
            return std::unexpected(
                Error{ErrorCode::InvalidArgument, "openexr: missing expected channel"});
        }
        for (const auto& name : *channelNames) {
            const Imf::Channel* c = channels.findChannel(name);
            if (c == nullptr || c->type != first->type) {
                return std::unexpected(
                    Error{ErrorCode::InvalidArgument,
                          "openexr: channels must all share the same pixel type"});
            }
        }
        auto pixelType = fromImfPixelType(first->type);
        if (!pixelType.has_value()) {
            return std::unexpected(pixelType.error());
        }

        return OpenExrImageInfo{
            .width = width, .height = height, .samplesPerPixel = spp, .pixelType = *pixelType};
    } catch (const std::exception& e) {
        return std::unexpected(Error{ErrorCode::InvalidArgument,
                                     std::string{"openexr: header read failed: "} + e.what()});
    }
}

} // namespace ptiff::io::backend::openexr
