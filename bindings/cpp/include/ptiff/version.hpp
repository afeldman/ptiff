#pragma once

/// @file version.hpp
/// @brief Version accessors over the C-ABI version surface.

#include <string>

#include <ptiff/core.hpp>
#include <ptiff/detail/c_abi.hpp>

namespace ptiff {

/// @brief Semantic version triple reported by the Rust core.
struct Version {
    int major;
    int minor;
    int patch;
    [[nodiscard]] std::string toString() const {
        return std::to_string(major) + "." + std::to_string(minor) + "." +
        std::to_string(patch);
    }
};

/// @brief Version the library was compiled/linked against (== runtime; the
///        Rust core is statically linked into `libptiff_c`).
inline Version compileTimeVersion() {
    detail::ptiff_version v = detail::ptiff_compile_time_version();
    return Version{v.major, v.minor, v.patch};
}

/// @brief Version of the loaded library at runtime.
inline Version runtimeVersion() {
    detail::ptiff_version v = detail::ptiff_runtime_version();
    return Version{v.major, v.minor, v.patch};
}

} // namespace ptiff
