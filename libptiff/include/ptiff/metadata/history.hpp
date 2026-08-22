#pragma once

#include <chrono>
#include <memory>
#include <span>
#include <string>

#include <ptiff/export.hpp>

namespace ptiff {

/// @brief One provenance record: what happened, when, by whom, with what tool.
///
/// A single entry in a `History` log, describing one processing step applied to the data.
struct HistoryEntry {
    std::chrono::system_clock::time_point timestamp; ///< When the step happened.
    std::string description;                         ///< What was done (free-form description).
    std::string softwareName;                        ///< Software/tool used (e.g. "gdal").
    std::string softwareVersion;                     ///< Version of the software.
    std::string operatorName;                        ///< Who ran the step.
};

/// @brief An append-only provenance log.
///
/// Records the processing history of a dataset: a sequence of
/// @ref ptiff::HistoryEntry "HistoryEntry" items, appended in order and never removed or
/// reordered. Contrast with @ref ptiff::Mission "Mission", which is static capture context set
/// once and never appended to.
///
/// @section history_example Example
///
/// @code{.cpp}
/// using ptiff::History;
/// using ptiff::HistoryEntry;
///
/// History h;
/// HistoryEntry e;
/// e.timestamp = std::chrono::system_clock::now();
/// e.description = "radiometric calibration";
/// e.softwareName = "ptiff-utils";
/// h.append(std::move(e));
///
/// assert(h.entries().size() == 1);
/// @endcode
///
/// @see @ref ptiff::Mission "Mission" for the static, capture-time context.
class PTIFF_EXPORT History {
public:
    History();
    ~History();

    History(const History&) = delete;
    History& operator=(const History&) = delete;
    History(History&&) noexcept;
    History& operator=(History&&) noexcept;

    /// @brief Appends a provenance record to the end of the log.
    ///
    /// @param entry The @ref ptiff::HistoryEntry "HistoryEntry" to record.
    void append(HistoryEntry entry);

    /// @brief Returns the recorded entries as an immutable span, in appended order.
    ///
    /// @return A `std::span<const HistoryEntry>` over the log. The span is valid until the next
    ///         non-const operation on this History.
    [[nodiscard]] std::span<const HistoryEntry> entries() const noexcept;

private:
    struct Impl;
    std::unique_ptr<Impl> impl_;
};

} // namespace ptiff
