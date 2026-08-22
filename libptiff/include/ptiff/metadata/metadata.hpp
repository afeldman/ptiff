#pragma once

#include <memory>
#include <string>
#include <string_view>

#include <ptiff/core/result.hpp>
#include <ptiff/export.hpp>

namespace ptiff {

/// @brief A free-form scientific key/value bag for anything not covered by a dedicated type.
///
/// An unstructured metadata store: `get`/`set` string key/value pairs for any scientific
/// metadata that does not have a dedicated type (mission, history, layer kind, ...).
///
/// Sprint-1 note: this is an interface-only stub. Every accessor returns
/// @ref ptiff::ErrorCode::NotImplemented "ErrorCode::NotImplemented" until metadata storage
/// exists. Move-only; copy semantics will be decided once there is real state.
class PTIFF_EXPORT Metadata {
public:
    Metadata();
    ~Metadata();

    Metadata(const Metadata&) = delete;
    Metadata& operator=(const Metadata&) = delete;
    Metadata(Metadata&&) noexcept;
    Metadata& operator=(Metadata&&) noexcept;

    /// @brief Looks up a metadata value by key.
    ///
    /// Sprint-1 stub: returns @ref ptiff::ErrorCode::NotImplemented "NotImplemented".
    ///
    /// @param key The metadata key to look up.
    /// @return The value associated with \p key on success, or an error otherwise.
    [[nodiscard]] Result<std::string> get(std::string_view key) const;

private:
    struct Impl;
    std::unique_ptr<Impl> impl_;
};

} // namespace ptiff
