#include <cctype>
#include <string>
#include <string_view>
#include <unordered_map>

#include <ptiff/io/backend/isis/isis_label.hpp>

namespace ptiff::io::backend::isis {

namespace {

// ptiff pixelType -> ISIS3 PDS3 "Type" keyword value.
constexpr std::string_view pixelTypeToIsis(std::string_view pt) {
    if (pt == "UInt8")
        return "UnsignedByte";
    if (pt == "UInt16")
        return "UnsignedWord";
    if (pt == "UInt32")
        return "UnsignedInteger";
    if (pt == "Float32")
        return "Real";
    if (pt == "Float64")
        return "Double";
    return {}; // unsupported
}

// ISIS3 "Type" keyword -> ptiff pixelType, or empty if unrecognized.
constexpr std::string_view isisToPixelType(std::string_view isis) {
    if (isis == "UnsignedByte")
        return "UInt8";
    if (isis == "UnsignedWord")
        return "UInt16";
    if (isis == "UnsignedInteger")
        return "UInt32";
    if (isis == "Real")
        return "Float32";
    if (isis == "Double")
        return "Float64";
    return {};
}

// Trims surrounding whitespace.
std::string_view trim(std::string_view s) {
    while (!s.empty() && std::isspace(static_cast<unsigned char>(s.front())))
        s.remove_prefix(1);
    while (!s.empty() && std::isspace(static_cast<unsigned char>(s.back())))
        s.remove_suffix(1);
    return s;
}

void appendLine(std::string& out, std::string_view text) {
    out.append(text);
    out.push_back('\n');
}

// Writes the optional ptiff storage fields (tileWidth, tileHeight, predictor, container) as
// PDS3 keywords inside Core. compression is represented in the Pixels group instead.
void appendExtraFields(std::string& out, const StorageModel& model) {
    for (auto* key : {"tileWidth", "tileHeight", "predictor", "container"}) {
        if (auto v = model.field(key); v.has_value()) {
            appendLine(out, "          " + std::string{key} + " = \"" + *v + "\"");
        }
    }
}

// Parses every "key = value" assignment into a flat map, walking the label line by line. A key
// is the trimmed text before the first '=' on the line; the value is the trimmed text after it
// with surrounding double quotes removed. Structural lines (Object/Group/End, which carry no '=')
// are naturally skipped. Line-based parsing avoids off-by-one issues with newline scanning.
void collectAssignments(std::string_view label, std::unordered_map<std::string, std::string>& out) {
    std::size_t pos = 0;
    const std::size_t n = label.size();
    while (pos < n) {
        const std::size_t nl = label.find('\n', pos);
        std::string_view line =
            nl == std::string_view::npos ? label.substr(pos) : label.substr(pos, nl - pos);
        // Strip a trailing CR if present.
        if (!line.empty() && line.back() == '\r') {
            line.remove_suffix(1);
        }
        const std::size_t eq = line.find('=');
        if (eq != std::string_view::npos) {
            std::string_view key = trim(line.substr(0, eq));
            std::string_view value = trim(line.substr(eq + 1));
            if (value.size() >= 2 && value.front() == '"' && value.back() == '"') {
                value = value.substr(1, value.size() - 2);
            }
            if (!key.empty()) {
                out[std::string{key}] = std::string{value};
            }
        }
        pos = nl == std::string_view::npos ? n : nl + 1;
    }
}

} // namespace

Result<std::vector<std::byte>> writeLabel(const StorageModel& model) {
    auto width = model.field("imageWidth");
    auto height = model.field("imageHeight");
    auto spp = model.field("samplesPerPixel");
    auto ptype = model.field("pixelType");
    if (!width.has_value() || !height.has_value() || !spp.has_value() || !ptype.has_value()) {
        return std::unexpected(Error{ErrorCode::InvalidArgument,
                                     "isis: label requires imageWidth/imageHeight/samplesPerPixel/"
                                     "pixelType"});
    }
    const std::string_view isisType = pixelTypeToIsis(*ptype);
    if (isisType.empty()) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "isis: unsupported pixelType in label"});
    }
    auto compression = model.field("compression");

    std::string out;
    appendLine(out, "Object = IsisCube");
    appendLine(out, "  Object = Core");
    appendLine(out, "    Group = Dimensions");
    appendLine(out, "      Samples = " + *width);
    appendLine(out, "      Lines = " + *height);
    appendLine(out, "      Bands = " + *spp);
    appendLine(out, "    End_Group");
    appendLine(out, "    Group = Pixels");
    appendLine(out, "      Type = " + std::string{isisType});
    appendLine(out, "      ByteOrder = Lsb");
    appendLine(out,
               "      Compression = \"" +
                   (compression.has_value() ? *compression : std::string{"None"}) + "\"");
    appendLine(out, "    End_Group");
    appendExtraFields(out, model);
    appendLine(out, "  End_Object");
    appendLine(out, "End_Object");
    appendLine(out, "End");

    std::vector<std::byte> bytes(out.size());
    for (std::size_t i = 0; i < out.size(); ++i) {
        bytes[i] = static_cast<std::byte>(static_cast<unsigned char>(out[i]));
    }
    return bytes;
}

Result<StorageModel> parseLabel(std::span<const std::byte> labelText) {
    std::string_view label(reinterpret_cast<const char*>(labelText.data()), labelText.size());
    if (label.find("Object = IsisCube") == std::string_view::npos) {
        return std::unexpected(Error{ErrorCode::InvalidArgument,
                                     "isis: not an ISIS3 label (missing IsisCube object)"});
    }

    std::unordered_map<std::string, std::string> kv;
    collectAssignments(label, kv);

    auto get = [&](const char* key) -> const std::string* {
        auto it = kv.find(key);
        return it == kv.end() ? nullptr : &it->second;
    };

    const std::string* w = get("Samples");
    const std::string* h = get("Lines");
    const std::string* b = get("Bands");
    const std::string* t = get("Type");
    if (w == nullptr || h == nullptr || b == nullptr || t == nullptr) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "isis: label missing Dimensions/Pixels values"});
    }
    const auto isoType = isisToPixelType(*t);
    if (isoType.empty()) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "isis: unrecognized pixel Type in label"});
    }

    StorageModel model;
    model.setField("imageWidth", *w);
    model.setField("imageHeight", *h);
    model.setField("samplesPerPixel", *b);
    model.setField("pixelType", std::string{isoType});
    if (const std::string* c = get("Compression"); c != nullptr) {
        model.setField("compression", *c);
    } else {
        model.setField("compression", "None");
    }
    // Optional ptiff fields (round-trip with writeLabel).
    for (const char* key : {"tileWidth", "tileHeight", "predictor", "container"}) {
        if (const std::string* v = get(key); v != nullptr) {
            model.setField(key, *v);
        }
    }
    return model;
}

} // namespace ptiff::io::backend::isis
