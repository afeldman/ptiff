#include <map>
#include <mutex>
#include <utility>

#include <ptiff/io/backend_factory.hpp>

namespace ptiff::io {

struct BackendFactory::Impl {
    mutable std::mutex mutex;
    std::map<std::string, Builder, std::less<>> builders;
};

BackendFactory::BackendFactory() : impl_(std::make_unique<Impl>()) {}
BackendFactory::~BackendFactory() = default;

BackendFactory& BackendFactory::instance() {
    static BackendFactory factory;
    return factory;
}

Result<void> BackendFactory::registerBackend(std::string name, Builder builder) {
    std::lock_guard lock{impl_->mutex};
    const auto [_, inserted] = impl_->builders.emplace(std::move(name), std::move(builder));
    if (!inserted) {
        return std::unexpected(Error{ErrorCode::InvalidArgument,
                                     "BackendFactory::registerBackend: name already registered"});
    }
    return {};
}

Result<std::unique_ptr<StorageBackend>> BackendFactory::create(std::string_view name) const {
    std::lock_guard lock{impl_->mutex};
    const auto it = impl_->builders.find(name);
    if (it == impl_->builders.end()) {
        return std::unexpected(
            Error{ErrorCode::NotFound, "BackendFactory::create: no such backend"});
    }
    return it->second();
}

std::vector<std::string> BackendFactory::registeredBackends() const {
    std::lock_guard lock{impl_->mutex};
    std::vector<std::string> names;
    names.reserve(impl_->builders.size());
    for (const auto& [name, builder] : impl_->builders) {
        names.push_back(name);
    }
    return names;
}

} // namespace ptiff::io
