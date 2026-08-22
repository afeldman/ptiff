#pragma once

#include <functional>
#include <memory>
#include <span>
#include <string>
#include <string_view>

#include <ptiff/core/result.hpp>
#include <ptiff/export.hpp>

namespace ptiff::io {

/// @brief Format-neutral intermediate representation of a document to store.
///
/// StorageModel is the in-memory, binary-free payload a
/// @ref ptiff::io::Serializer "Serializer" produces from a @ref ptiff::Scene "Scene" and a
/// @ref ptiff::io::StorageBackend "StorageBackend" later turns into (or back out of) bytes. It
/// holds only structured fields (a key/value table of strings) and nested child nodes -- one
/// child per image / camera / layer / ... that a Serializer chooses to emit. No binary data
/// lives here.
///
/// @section storage_model_value Value semantics (PIMPL)
///
/// StorageModel is a concrete value type implemented with the pointer-to-implementation idiom:
/// it is move-constructible and move-assignable but non-copyable, and cheap to move around a
/// scene tree. The children form an owning tree (each child is itself a StorageModel).
///
/// @section storage_model_thread Thread-safety
///
/// Thread-compatible: safe to read concurrently, not safe to mutate concurrently with any other
/// access -- the same contract as every other PIMPL value type in this codebase.
///
/// @section storage_model_example Example
///
/// @code{.cpp}
/// using ptiff::io::StorageModel;
///
/// StorageModel image;
/// image.setField("width", "64");
/// image.setField("height", "32");
///
/// StorageModel camera;
/// camera.setField("name", "left");
///
/// StorageModel scene;
/// scene.addChild(std::move(image));
/// scene.addChild(std::move(camera));
///
/// assert(scene.children().size() == 2);
/// assert(!scene.field("missing").has_value());          // never-set key
/// @endcode
///
/// @see @ref ptiff::io::Serializer "Serializer",
///      @ref ptiff::io::StorageBackend "StorageBackend".
class PTIFF_EXPORT StorageModel {
public:
    /// @brief Default-constructs an empty storage model (no fields, no children).
    StorageModel();
    /// @brief Destroys the model and its owning child tree.
    ~StorageModel();

    StorageModel(const StorageModel&) = delete;
    StorageModel& operator=(const StorageModel&) = delete;
    /// @brief Move-constructs a model, taking over the other's fields and children.
    StorageModel(StorageModel&&) noexcept;
    /// @brief Move-assigns a model, taking over the other's fields and children.
    StorageModel& operator=(StorageModel&&) noexcept;

    /// @brief Sets (or overwrites) a scalar string field.
    ///
    /// @param key   The field name.
    /// @param value The field value; copied into the model.
    ///
    /// @code{.cpp}
    /// StorageModel m;
    /// m.setField("model", "HiRISE");
    /// m.setField("model", "CTX");        // overwrites
    /// assert(m.field("model") == std::string{"CTX"});
    /// @endcode
    void setField(std::string_view key, std::string value);
    /// @brief Reads a scalar string field.
    ///
    /// @param key The field name to look up.
    /// @return The field's value on success, or
    ///         @ref ptiff::ErrorCode::NotFound "NotFound" if \p key was never set.
    [[nodiscard]] Result<std::string> field(std::string_view key) const;

    /// @brief Invokes \p fn for every (key, value) field pair, in ascending key order.
    ///
    /// Enumerates the node's scalar string fields exactly once each, ordered
    /// lexicographically by key (the map's stable ordering). This is the counterpart to
    /// @ref setField and the general way to persist a @ref StorageModel "StorageModel" tree
    /// without naming the fields a priori (e.g. a serialization codec).
    ///
    /// @param fn Callable invoked as `fn(key, value)` for each pair, where both arguments are
    ///           `std::string_view` valid for the duration of the call.
    ///
    /// @code{.cpp}
    /// StorageModel m;
    /// m.setField("b", "2");
    /// m.setField("a", "1");
    /// std::vector<std::string> order;
    /// m.for_each_field([&](std::string_view k, std::string_view v) {
    ///     order.push_back(std::string{k});   // "a", then "b"
    /// });
    /// @endcode
    void for_each_field(std::function<void(std::string_view, std::string_view)> fn) const;

    /// @brief Appends a nested child node.
    ///
    /// @param child The child model; moved into this model's owning child list. One child
    ///              typically corresponds to one image / camera / layer the serializer emits.
    ///
    /// @code{.cpp}
    /// StorageModel root;
    /// StorageModel image;
    /// image.setField("width", "64");
    /// root.addChild(std::move(image));
    /// assert(root.children().size() == 1);
    /// @endcode
    void addChild(StorageModel child);
    /// @brief Returns a view over this model's child nodes.
    /// @return A `std::span<const StorageModel>` over the child list; valid for this model's
    ///         lifetime and invalidated by `addChild` or a move into/out of this model.
    [[nodiscard]] std::span<const StorageModel> children() const noexcept;

private:
    struct Impl;
    std::unique_ptr<Impl> impl_;
};

} // namespace ptiff::io
