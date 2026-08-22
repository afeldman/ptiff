#pragma once

#include <string_view>
#include <vector>

#include <ptiff/io/storage_model.hpp>

namespace ptiff::io::detail {

// The Scene Serializer/Deserializer shape is a root model with one child per image, each child
// carrying that image's fields. The TIFF backend's deserializeModel now returns exactly this
// shape (one child per IFD in the chain); the Writer facade flattens those children back into
// per-image flat records to hand to the backend's multi-image write path.

// Field names shared between the Scene child-node shape and the TIFF flat root-level shape.
inline constexpr std::string_view kSceneStorageFields[] = {
    "imageWidth",
    "imageHeight",
    "samplesPerPixel",
    "pixelType",
    "compression",
    "predictor",
    "container",
    "tileWidth",
    "tileHeight",
};

// Copies every known field present on `from` onto `to` (overwriting). Fields absent on `from`
// (e.g. optional "predictor" or "container") are simply left alone on `to`.
inline void copySceneStorageFields(const StorageModel& from, StorageModel& to) {
    for (std::string_view key : kSceneStorageFields) {
        if (auto value = from.field(key); value.has_value()) {
            to.setField(key, *value);
        }
    }
}

// Flattens a root model with one child per image into a vector of flat root-level models (one
// per image, in scene order), each carrying that image's fields -- the shape the multi-image
// TIFF backend's write path takes. Errors unless the root has at least one child.
[[nodiscard]] inline Result<std::vector<StorageModel>> flattenChildren(const StorageModel& root) {
    if (root.children().empty()) {
        return std::unexpected(
            Error{ErrorCode::NotImplemented,
                  "scene_storage_adapter: flattenChildren expects at least one child node"});
    }
    std::vector<StorageModel> models;
    models.reserve(root.children().size());
    for (const auto& child : root.children()) {
        StorageModel flat;
        copySceneStorageFields(child, flat);
        models.push_back(std::move(flat));
    }
    return models;
}

// Legacy single-image helper: flattens a root with exactly one child. Errors unless the root has
// exactly one child. For multi-image scenes use flattenChildren.
[[nodiscard]] inline Result<StorageModel> flattenSingleChild(const StorageModel& root) {
    auto flatModels = flattenChildren(root);
    if (!flatModels.has_value()) {
        return std::unexpected(flatModels.error());
    }
    if (flatModels->size() != 1) {
        return std::unexpected(
            Error{ErrorCode::NotImplemented,
                  "scene_storage_adapter: flattenSingleChild expects exactly one child node"});
    }
    return std::move(flatModels->front());
}

} // namespace ptiff::io::detail
