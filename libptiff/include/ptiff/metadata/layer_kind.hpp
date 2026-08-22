#pragma once

namespace ptiff {

/// @brief What a @ref ptiff::ScientificLayer "ScientificLayer" represents.
///
/// Identifies the derived per-pixel quantity carried by a scientific layer. New kinds are added
/// here as new enum values -- never as a new C++ type -- so the Go/Python/Rust bindings never
/// need to learn a new type for a new layer kind.
///
/// @section layer_kind_values Values
///
/// | Enumerator | Meaning |
/// |------------|---------|
/// | `Dem`       | Digital elevation model (surface). |
/// | `Dtm`       | Digital terrain model (bare ground / geoid). |
/// | `Dsm`       | Digital surface model (top of canopy / buildings). |
/// | `Depth`     | Depth-from-stereo depth map. |
/// | `Normals`   | Surface normal per pixel. |
/// | `Albedo`    | Surface albedo/reflectivity. |
/// | `Confidence`| Per-pixel matching/quality confidence. |
/// | `Temperature` | Measured surface temperature. |
/// | `Slope`     | Local terrain slope. |
/// | `Aspect`    | Local terrain aspect (compass direction). |
/// | `Reflectance` | Spectral/visible reflectance. |
/// | `Mask`      | Binary/pre-classified mask. |
enum class LayerKind {
    Dem,
    Dtm,
    Dsm,
    Depth,
    Normals,
    Albedo,
    Confidence,
    Temperature,
    Slope,
    Aspect,
    Reflectance,
    Mask,
};

} // namespace ptiff
