#include <cstring>
#include <string>

#include <ptiff/io/backend/pds4/pds4_label.hpp>

#include <pugixml.hpp>

namespace ptiff::io::backend::pds4 {

namespace {

constexpr std::string_view kProductRoot = "Product_Observational";
constexpr std::string_view kArrayNode = "Array_2D_Image";

// Minimal string-building sink for pugi::xml_writer (collects serialized label bytes).
class XmlStringWriter final : public pugi::xml_writer {
public:
    void write(const void* data, std::size_t size) override {
        result.append(static_cast<const char*>(data), size);
    }
    std::string result;
};

// Writes every key=value storage field of @p model as a child element of @p parent. Element
// names are the storage field keys ("imageWidth", "pixelType", ...); the value is the element
// text. Keeping the element name equal to the storage key round-trips losslessly.
void appendFields(pugi::xml_node parent, const StorageModel& model) {
    model.for_each_field([&](std::string_view key, std::string_view value) {
        pugi::xml_node el = parent.append_child();
        el.set_name(key.data());
        el.text().set(value.data());
    });
}

// Reads the child elements of @p array into @p model, mapping each element name (which is the
// storage key) to its text value.
Result<void> collectFields(pugi::xml_node array, StorageModel& model) {
    for (pugi::xml_node el = array.first_child(); el; el = el.next_sibling()) {
        if (el.type() != pugi::node_element) {
            continue;
        }
        const char* name = el.name();
        const char* value = el.child_value();
        if (name == nullptr || *name == '\0') {
            continue;
        }
        if (value == nullptr) {
            return std::unexpected(
                Error{ErrorCode::InvalidArgument, "pds4: array field with empty value"});
        }
        model.setField(name, value);
    }
    return {};
}

} // namespace

Result<std::vector<std::byte>> writeLabel(const StorageModel& model) {
    pugi::xml_document doc;
    pugi::xml_node product = doc.append_child(kProductRoot.data());
    pugi::xml_node array = product.append_child(kArrayNode.data());
    appendFields(array, model);

    XmlStringWriter writer;
    doc.save(writer); // compact serialization (no indentation)

    std::vector<std::byte> bytes(writer.result.size());
    std::memcpy(bytes.data(), writer.result.c_str(), writer.result.size());
    return bytes;
}

Result<StorageModel> parseLabel(std::span<const std::byte> labelBytes) {
    if (labelBytes.empty()) {
        return std::unexpected(Error{ErrorCode::InvalidArgument, "pds4: empty label"});
    }
    std::string text(reinterpret_cast<const char*>(labelBytes.data()), labelBytes.size());

    pugi::xml_document doc;
    const auto parseResult =
        doc.load_buffer(text.data(), text.size(), pugi::parse_default | pugi::parse_comments);
    if (!parseResult) {
        return std::unexpected(Error{ErrorCode::InvalidArgument, "pds4: malformed XML label"});
    }

    const pugi::xml_node product = doc.child(kProductRoot.data());
    if (!product) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "pds4: label has no Product_Observational root"});
    }
    const pugi::xml_node array = product.child(kArrayNode.data());
    if (!array) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument, "pds4: label has no Array_2D_Image element"});
    }

    StorageModel model;
    auto collectResult = collectFields(array, model);
    if (!collectResult.has_value()) {
        return std::unexpected(collectResult.error());
    }
    return model;
}

} // namespace ptiff::io::backend::pds4
