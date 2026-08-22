#include <utility>
#include <vector>

#include <ptiff/metadata/history.hpp>

namespace ptiff {

struct History::Impl {
    std::vector<HistoryEntry> entries;
};

History::History() : impl_(std::make_unique<Impl>()) {}
History::~History() = default;
History::History(History&&) noexcept = default;
History& History::operator=(History&&) noexcept = default;

void History::append(HistoryEntry entry) {
    impl_->entries.push_back(std::move(entry));
}

std::span<const HistoryEntry> History::entries() const noexcept {
    return impl_->entries;
}

} // namespace ptiff
