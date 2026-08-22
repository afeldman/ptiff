#include <cstddef>
#include <cstdint>
#include <string>
#include <vector>

#include <ptiff/core/error.hpp>
#include <ptiff/core/result.hpp>
#include <ptiff/io/backend/tiff/ptiff_metadata.hpp>
#include <ptiff/io/backend/tiff/tiff_tag.hpp>
#include <ptiff/io/storage_model.hpp>

#include <catch2/catch_test_macros.hpp>

using ptiff::io::StorageModel;
using ptiff::io::backend::tiff::decodeMetadataPayload;
using ptiff::io::backend::tiff::encodeMetadataPayload;
using ptiff::io::backend::tiff::MetadataRecord;
using ptiff::io::backend::tiff::recordsFromStorageModel;

namespace {

std::vector<std::uint64_t> toByteValues(const std::vector<std::byte>& payload) {
    std::vector<std::uint64_t> out;
    out.reserve(payload.size());
    for (auto b : payload) {
        out.push_back(static_cast<std::uint64_t>(static_cast<unsigned char>(b)));
    }
    return out;
}

} // namespace

TEST_CASE("metadata payload encodes/decodes a SPICE record set losslessly", "[ptiff-metadata]") {
    const std::vector<MetadataRecord> records{
        {"frame", "IAU_MOON"},
        {"instrument", "LROC_WAC"},
        {"position_x", "384400.0"},
        {"time_system", "TDB"},
    };
    auto payload = encodeMetadataPayload(records);
    REQUIRE(payload.has_value());

    // Every encoded payload begins with the PTIFF magic.
    REQUIRE(payload->size() >= 5);
    REQUIRE((*payload)[0] == std::byte{'P'});
    REQUIRE((*payload)[1] == std::byte{'T'});

    auto decoded = decodeMetadataPayload(toByteValues(*payload));
    REQUIRE(decoded.has_value());
    // Canonical (key-sorted) order: frame, instrument, position_x, time_system.
    REQUIRE(decoded->size() == records.size());
    REQUIRE((*decoded)[0].key == "frame");
    REQUIRE((*decoded)[0].value == "IAU_MOON");
    REQUIRE((*decoded)[1].key == "instrument");
    REQUIRE((*decoded)[1].value == "LROC_WAC");
    REQUIRE((*decoded)[2].key == "position_x");
    REQUIRE((*decoded)[2].value == "384400.0");
    REQUIRE((*decoded)[3].key == "time_system");
    REQUIRE((*decoded)[3].value == "TDB");
}

TEST_CASE("metadata payload encoding is canonical regardless of input order", "[ptiff-metadata]") {
    const std::vector<MetadataRecord> a{{"b", "2"}, {"a", "1"}};
    const std::vector<MetadataRecord> b{{"a", "1"}, {"b", "2"}};
    auto pa = encodeMetadataPayload(a);
    auto pb = encodeMetadataPayload(b);
    REQUIRE(pa.has_value());
    REQUIRE(pb.has_value());
    REQUIRE(*pa == *pb);
}

TEST_CASE("metadata payload rejects a non-PTIFF byte blob", "[ptiff-metadata]") {
    // A random private-tag byte blob from another tool must not be mis-parsed.
    const std::vector<std::uint64_t> junk{0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03};
    auto decoded = decodeMetadataPayload(junk);
    REQUIRE_FALSE(decoded.has_value());
    REQUIRE(decoded.error().code() == ptiff::ErrorCode::InvalidArgument);
}

TEST_CASE("recordsFromStorageModel reads the ptiff.<domain>.* convention", "[ptiff-metadata]") {
    StorageModel model;
    model.setField("imageWidth", "64");
    model.setField("ptiff.camera.model", "pinhole");
    model.setField("ptiff.camera.focal_length_x", "100.0");

    auto records = recordsFromStorageModel(model, "camera");
    REQUIRE(records.size() == 2);
    // StorageModel orders fields by key; the camera.* records come back key-ascending.
    REQUIRE(records[0].key == "focal_length_x");
    REQUIRE(records[0].value == "100.0");
    REQUIRE(records[1].key == "model");
    REQUIRE(records[1].value == "pinhole");
}
