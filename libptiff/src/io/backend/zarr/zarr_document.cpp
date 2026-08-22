#include <cstring>
#include <string>
#include <string_view>

#include <ptiff/io/backend/memory/memory_layout.hpp> // bytesPerSample
#include <ptiff/io/backend/zarr/zarr_codec.hpp>      // payloadBound
#include <ptiff/io/backend/zarr/zarr_document.hpp>

#include <nlohmann/json.hpp>

namespace ptiff::io::backend::zarr {

namespace {

// Maps a ptiff pixelType storage-field value to a Zarr dtype string, or empty if unsupported.
std::string_view pixelTypeToDtype(std::string_view pt) {
    if (pt == "UInt8")
        return "|u1";
    if (pt == "UInt16")
        return "<u2";
    if (pt == "UInt32")
        return "<u4";
    if (pt == "Float32")
        return "<f4";
    if (pt == "Float64")
        return "<f8";
    return {};
}

// Maps a Zarr dtype string to a ptiff pixelType, or empty if unrecognized.
std::string_view dtypeToPixelType(std::string_view dtype) {
    if (dtype == "|u1")
        return "UInt8";
    if (dtype == "<u2")
        return "UInt16";
    if (dtype == "<u4")
        return "UInt32";
    if (dtype == "<f4")
        return "Float32";
    if (dtype == "<f8")
        return "Float64";
    return {};
}

// Writes the ptiff tile/geometry into a nlohmann JSON array header. shape is [H,W] for a single
// band or [B,H,W]; chunks is [tileH,tileW] (== the ptiff tile size).
nlohmann::json toJson(const StorageModel& model) {
    const std::string w = model.field("imageWidth").value();
    const std::string h = model.field("imageHeight").value();
    const std::string b = model.field("samplesPerPixel").value();
    const std::string tw = model.field("tileWidth").value();
    const std::string th = model.field("tileHeight").value();
    const std::string dtype(pixelTypeToDtype(model.field("pixelType").value()));

    nlohmann::json j;
    j["compressor"] = "none"; // filled below
    if (auto c = model.field("compression"); c.has_value()) {
        if (*c == "zstd")
            j["compressor"] = "zstd";
        else if (*c == "zlib")
            j["compressor"] = "zlib";
    }
    if (b == "1") {
        j["shape"] = {std::stoi(h), std::stoi(w)};
        j["chunks"] = {std::stoi(th), std::stoi(tw)};
    } else {
        j["shape"] = {std::stoi(b), std::stoi(h), std::stoi(w)};
        j["chunks"] = {std::stoi(th), std::stoi(tw)};
    }
    j["dtype"] = dtype;
    return j;
}

} // namespace

Result<Compressor> parseCompressor(std::string_view value) {
    // Accept the storage-field spelling ("None") and the JSON-header spelling ("none").
    if (value == "None" || value == "none")
        return Compressor::None;
    if (value == "zstd")
        return Compressor::Zstd;
    if (value == "zlib")
        return Compressor::Zlib;
    return std::unexpected(Error{ErrorCode::InvalidArgument,
                                 "zarr: unsupported compression (expected None/zstd/zlib)"});
}

Result<ZarrLayout> buildLayout(const StorageModel& model) {
    auto required = [&](const char* key) -> Result<std::string> {
        auto v = model.field(key);
        if (!v.has_value()) {
            return std::unexpected(Error{ErrorCode::InvalidArgument,
                                         "zarr: missing required field " + std::string{key}});
        }
        return v;
    };
    auto w = required("imageWidth");
    auto h = required("imageHeight");
    auto b = required("samplesPerPixel");
    auto pt = required("pixelType");
    auto tw = required("tileWidth");
    auto th = required("tileHeight");
    if (!w.has_value() || !h.has_value() || !b.has_value() || !pt.has_value() || !tw.has_value() ||
        !th.has_value()) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "zarr: missing geometry/frame fields"});
    }

    const std::string_view dtype = pixelTypeToDtype(*pt);
    if (dtype.empty()) {
        return std::unexpected(Error{ErrorCode::InvalidArgument, "zarr: unsupported pixelType"});
    }

    Compressor compressor = Compressor::None;
    if (auto c = model.field("compression"); c.has_value()) {
        auto parsed = parseCompressor(*c);
        if (!parsed.has_value()) {
            return std::unexpected(parsed.error());
        }
        compressor = *parsed;
    }

    // Note: StorageModel is move-only (non-copyable), so `image` is intentionally left empty
    // here; serializeModel/openImageSink consume only header/layout/geometry from the layout and
    // deserializeModel builds the model independently from the parsed header.
    ZarrLayout out;
    out.compressor = compressor;
    out.layout =
        io::tile::TileLayout{.tileSize = {.width = static_cast<std::uint32_t>(std::stoul(*tw)),
                                          .height = static_cast<std::uint32_t>(std::stoul(*th))},
                             .imageWidth = static_cast<std::uint32_t>(std::stoul(*w)),
                             .imageHeight = static_cast<std::uint32_t>(std::stoul(*h)),
                             .levelCount = 1};

    auto ptAll = ptiff::PixelType::UInt8;
    if (*pt == "UInt16")
        ptAll = ptiff::PixelType::UInt16;
    else if (*pt == "UInt32")
        ptAll = ptiff::PixelType::UInt32;
    else if (*pt == "Float32")
        ptAll = ptiff::PixelType::Float32;
    else if (*pt == "Float64")
        ptAll = ptiff::PixelType::Float64;
    const std::uint64_t bps = memory::bytesPerSample(ptAll);
    const std::uint64_t spp = std::stoull(*b);
    out.chunkBytes = static_cast<std::uint32_t>(std::stoull(*tw) * std::stoull(*th) * spp * bps);
    out.slotSize = static_cast<std::uint32_t>(4u + payloadBound(compressor, out.chunkBytes));

    nlohmann::json j = toJson(model);
    const std::string text = j.dump();
    out.header.assign(text.size(), std::byte{});
    std::memcpy(out.header.data(), text.data(), text.size());
    return out;
}

Result<ZarrLayout> parseHeader(std::span<const std::byte> jsonHeader) {
    std::string text(reinterpret_cast<const char*>(jsonHeader.data()), jsonHeader.size());
    nlohmann::json j;
    try {
        j = nlohmann::json::parse(text);
    } catch (const nlohmann::json::exception&) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "zarr: malformed JSON array header"});
    }

    auto arr = [&](const char* key) -> const nlohmann::json* {
        auto it = j.find(key);
        return it != j.end() ? &*it : nullptr;
    };
    const nlohmann::json* shape = arr("shape");
    const nlohmann::json* chunks = arr("chunks");
    const nlohmann::json* dtype = arr("dtype");
    if (shape == nullptr || chunks == nullptr || dtype == nullptr || !shape->is_array() ||
        !chunks->is_array() || !dtype->is_string()) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "zarr: header missing shape/chunks/dtype"});
    }

    const std::size_t ndim = shape->size();
    if (ndim != 2 && ndim != 3) {
        return std::unexpected(Error{ErrorCode::InvalidArgument, "zarr: unsupported shape rank"});
    }
    const std::uint32_t w = (*shape)[ndim - 1].get<std::uint32_t>();
    const std::uint32_t h = (*shape)[ndim - 2].get<std::uint32_t>();
    const std::uint32_t tw = (*chunks)[ndim - 1].get<std::uint32_t>();
    const std::uint32_t th = (*chunks)[ndim - 2].get<std::uint32_t>();
    const std::uint32_t bands = ndim == 3 ? (*shape)[0].get<std::uint32_t>() : 1;

    const std::string_view dstr = dtype->get_ref<const std::string&>();
    const auto pt = dtypeToPixelType(dstr);
    if (pt.empty()) {
        return std::unexpected(Error{ErrorCode::InvalidArgument, "zarr: unrecognized dtype"});
    }

    Compressor compressor = Compressor::None;
    if (const nlohmann::json* c = arr("compressor"); c != nullptr && c->is_string()) {
        const std::string cs = c->get_ref<const std::string&>();
        auto parsed = parseCompressor(cs);
        if (!parsed.has_value()) {
            return std::unexpected(parsed.error());
        }
        compressor = *parsed;
    }

    ZarrLayout out;
    out.compressor = compressor;
    out.layout = io::tile::TileLayout{.tileSize = {.width = tw, .height = th},
                                      .imageWidth = w,
                                      .imageHeight = h,
                                      .levelCount = 1};

    auto ptAll = ptiff::PixelType::UInt8;
    if (pt == "UInt16")
        ptAll = ptiff::PixelType::UInt16;
    else if (pt == "UInt32")
        ptAll = ptiff::PixelType::UInt32;
    else if (pt == "Float32")
        ptAll = ptiff::PixelType::Float32;
    else if (pt == "Float64")
        ptAll = ptiff::PixelType::Float64;
    const std::uint64_t bps = memory::bytesPerSample(ptAll);
    out.chunkBytes = static_cast<std::uint32_t>(tw * th * bands * bps);
    out.slotSize = static_cast<std::uint32_t>(4u + payloadBound(compressor, out.chunkBytes));

    // Rebuild the flat storage model (round-trips buildLayout's field set).
    out.image.setField("imageWidth", std::to_string(w));
    out.image.setField("imageHeight", std::to_string(h));
    out.image.setField("tileWidth", std::to_string(tw));
    out.image.setField("tileHeight", std::to_string(th));
    out.image.setField("samplesPerPixel", std::to_string(bands));
    out.image.setField("pixelType", std::string{pt});
    if (compressor == Compressor::Zstd)
        out.image.setField("compression", "zstd");
    else if (compressor == Compressor::Zlib)
        out.image.setField("compression", "zlib");
    else
        out.image.setField("compression", "None");
    return out;
}

Result<ZarrLayout> readDocument(io::BinaryReader& reader) {
    auto reset = reader.seek(0);
    if (!reset.has_value()) {
        return std::unexpected(reset.error());
    }
    std::vector<std::byte> headerSizeBytes(kHeaderSize);
    auto readSize = reader.read(std::span<std::byte>{headerSizeBytes});
    if (!readSize.has_value() || *readSize != kHeaderSize) {
        return std::unexpected(Error{ErrorCode::InvalidArgument, "zarr: truncated seek header"});
    }
    std::uint64_t jsonLen = 0;
    for (std::size_t i = 0; i < kHeaderSize; ++i) {
        jsonLen |= static_cast<std::uint64_t>(std::to_integer<unsigned char>(headerSizeBytes[i]))
                   << (8 * i);
    }
    if (jsonLen == 0 || jsonLen > (1ull << 24)) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "zarr: implausible header length"});
    }
    std::vector<std::byte> json(jsonLen);
    auto readJson = reader.read(std::span<std::byte>{json});
    if (!readJson.has_value() || *readJson != jsonLen) {
        return std::unexpected(Error{ErrorCode::InvalidArgument, "zarr: truncated header"});
    }
    auto parsed = parseHeader(json);
    if (!parsed.has_value()) {
        return std::unexpected(parsed.error());
    }
    parsed->header = std::move(json);
    return parsed;
}

} // namespace ptiff::io::backend::zarr
