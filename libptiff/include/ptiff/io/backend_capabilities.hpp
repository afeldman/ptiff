#pragma once

namespace ptiff::io {

/// @brief Describes what a @ref ptiff::io::StorageBackend "StorageBackend" can do.
///
/// BackendCapabilities is a plain flag struct so callers -- @ref ptiff::io::BackendFactory
/// "BackendFactory" users, future @ref ptiff::Reader "Reader" / @ref ptiff::Writer "Writer"
/// implementations -- can pick or reject a backend based on its advertised abilities without
/// hardcoding per-format knowledge. Each backend advertises its true support for the flags below;
/// callers should not assume a flag is set unless the backend reports it.
///
/// @section capabilities_example Example
///
/// @code{.cpp}
/// using ptiff::io::BackendCapabilities;
///
/// BackendCapabilities caps;
/// caps.supportsTiling        = true;
/// caps.supportsStreaming     = true;
/// caps.supportsRandomAccess  = true;
/// caps.supportsCloudStreaming = false;
///
/// if (caps.supportsTiling) {
///     // Image is safe to rewrite tile-by-tile through an ImageSink.
/// }
/// @endcode
///
/// @see @ref ptiff::io::StorageBackend "StorageBackend",
///      @ref ptiff::io::BackendFactory "BackendFactory".
struct BackendCapabilities {
    /// Set if the backend stores images as tiled data (and can serve/sink tile-by-tile).
    bool supportsTiling = false;
    /// Set if the backend can stream data incrementally (rather than only whole-file at once).
    bool supportsStreaming = false;
    /// Set if the backend supports arbitrary positional reads/writes (seek).
    bool supportsRandomAccess = false;
    /// Set if this backend's format is safe to read over a slow, small-read-heavy random-access
    /// transport (e.g. HTTP range-reads via ptiff::io::HttpRangeBinaryReader) -- i.e. its header/
    /// directory layout does not require large sequential scans to open. This does not mean the
    /// backend itself does any networking; it is a statement about the format, not the
    /// transport.
    bool supportsCloudStreaming = false;

    /// Defaulted member-wise equality; two capability sets are equal iff all flags match.
    friend constexpr bool operator==(const BackendCapabilities&,
                                     const BackendCapabilities&) = default;
};

} // namespace ptiff::io
