#pragma once

#include <functional>
#include <memory>
#include <string>
#include <string_view>
#include <vector>

#include <ptiff/core/result.hpp>
#include <ptiff/export.hpp>
#include <ptiff/io/storage_backend.hpp>

namespace ptiff::io {

/// @brief Process-wide registry mapping a backend name to a factory function.
///
/// BackendFactory uses the combined Registry + Factory pattern: a backend name
/// (e.g. `"tiff"`, `"memory"`) maps to a function that produces a
/// @ref ptiff::io::StorageBackend "StorageBackend". It is the **one deliberate singleton** in
/// this codebase, chosen so PDS4 / ISIS / Zarr / ... backends can register from their own
/// translation unit's static initializer without this class changing. It is the standard entry
/// point that @ref ptiff::Reader "Reader" / @ref ptiff::Writer "Writer" and other callers use to
/// turn a name into a concrete backend.
///
/// @section backend_factory_singleton Why a singleton
///
/// Registration happens at static-initialization time from independent translation units with no
/// natural owner to inject it into; a single process-wide registry lets each backend self-register
/// and lets callers look it up anywhere.
///
/// @section backend_factory_thread Thread-safety
///
/// This class is thread-safe: the registry is internally synchronized, since static initializers
/// across translation units may race relative to other threads starting up.
///
/// @section backend_factory_example Example
///
/// @code{.cpp}
/// using ptiff::io::BackendFactory;
/// using ptiff::io::BackendCapabilities;
///
/// // Registration happens in a backend's own translation unit; callers only create:
/// auto backend = BackendFactory::instance().create("tiff");
/// assert(backend.has_value());
/// assert(backend.value()->name() == "tiff");
///
/// assert(!BackendFactory::instance().create("no-such-backend").has_value());
/// @endcode
///
/// @see @ref ptiff::io::StorageBackend "StorageBackend",
///      @ref ptiff::Reader "Reader", @ref ptiff::Writer "Writer".
class PTIFF_EXPORT BackendFactory {
public:
    /// @brief Signature of a backend factory function.
    /// @note Wraps a `std::function` so a backend can register any callable that owns whatever it
    ///       needs to build a StorageBackend.
    using Builder = std::function<std::unique_ptr<StorageBackend>()>;

    /// @brief Returns the sole process-wide BackendFactory.
    /// @return A reference to the singleton instance.
    static BackendFactory& instance();

    BackendFactory(const BackendFactory&) = delete;
    BackendFactory& operator=(const BackendFactory&) = delete;
    BackendFactory(BackendFactory&&) = delete;
    BackendFactory& operator=(BackendFactory&&) = delete;

    /// @brief Registers a builder under \p name.
    ///
    /// @param name    The backend name to register (e.g. `"tiff"`).
    /// @param builder A callable producing a @ref ptiff::io::StorageBackend "StorageBackend" when
    ///                invoked.
    /// @return `Result<void>` success on success, or
    ///         @ref ptiff::ErrorCode::InvalidArgument "InvalidArgument" if \p name is already
    ///         registered (registration is idempotency-checked and returns an error on a clash).
    Result<void> registerBackend(std::string name, Builder builder);
    /// @brief Creates a backend by name.
    ///
    /// @param name The backend name to look up (e.g. `"tiff"`).
    /// @return A `std::unique_ptr<StorageBackend>` built by the registered builder on success,
    ///         or @ref ptiff::ErrorCode::NotFound "NotFound" if \p name was never registered.
    ///
    /// @code{.cpp}
    /// auto b = BackendFactory::instance().create("memory");
    /// assert(b.has_value());
    /// @endcode
    [[nodiscard]] Result<std::unique_ptr<StorageBackend>> create(std::string_view name) const;
    /// @brief Returns the names of all currently registered backends.
    /// @return A vector of registered backend names (order unspecified).
    [[nodiscard]] std::vector<std::string> registeredBackends() const;

private:
    BackendFactory();
    ~BackendFactory();

    struct Impl;
    std::unique_ptr<Impl> impl_;
};

} // namespace ptiff::io
