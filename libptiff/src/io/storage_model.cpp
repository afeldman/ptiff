#include <functional>
#include <map>
#include <string_view>
#include <utility>
#include <vector>

#include <ptiff/io/storage_model.hpp>

namespace ptiff::io {

struct StorageModel::Impl {
    std::map<std::string, std::string, std::less<>> fields;
    std::vector<StorageModel> children;
};

StorageModel::StorageModel() : impl_(std::make_unique<Impl>()) {}
StorageModel::~StorageModel() = default;
StorageModel::StorageModel(StorageModel&&) noexcept = default;
StorageModel& StorageModel::operator=(StorageModel&&) noexcept = default;

void StorageModel::setField(std::string_view key, std::string value) {
    impl_->fields.insert_or_assign(std::string{key}, std::move(value));
}

Result<std::string> StorageModel::field(std::string_view key) const {
    const auto it = impl_->fields.find(key);
    if (it == impl_->fields.end()) {
        return std::unexpected(Error{ErrorCode::NotFound, "StorageModel::field: no such key"});
    }
    return it->second;
}

void StorageModel::for_each_field(
    std::function<void(std::string_view, std::string_view)> fn) const {
    for (const auto& [key, value] : impl_->fields) {
        fn(std::string_view{key}, std::string_view{value});
    }
}

void StorageModel::addChild(StorageModel child) {
    impl_->children.push_back(std::move(child));
}

std::span<const StorageModel> StorageModel::children() const noexcept {
    return impl_->children;
}

} // namespace ptiff::io
