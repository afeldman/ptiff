#include <type_traits>

#include <ptiff/ptiff.hpp>

#include <catch2/catch_test_macros.hpp>

// Compile-time contract checks for the sprint-1 API surface. No runtime behavior to assert yet
// -- these guard the ownership/move-semantics decisions from docs/CODING_GUIDELINES.md.

namespace {

template <class T>
concept MoveOnlyValueType = !std::is_copy_constructible_v<T> && !std::is_copy_assignable_v<T> &&
                            std::is_move_constructible_v<T> && std::is_move_assignable_v<T>;

static_assert(MoveOnlyValueType<ptiff::Image>);
static_assert(MoveOnlyValueType<ptiff::Scene>);
static_assert(MoveOnlyValueType<ptiff::Metadata>);
static_assert(MoveOnlyValueType<ptiff::ScientificLayer>);
static_assert(MoveOnlyValueType<ptiff::Camera>);
static_assert(MoveOnlyValueType<ptiff::CoordinateReferenceSystem>);

static_assert(!std::is_copy_constructible_v<ptiff::Reader>);
static_assert(!std::is_move_constructible_v<ptiff::Reader>);
static_assert(!std::is_copy_constructible_v<ptiff::Writer>);
static_assert(!std::is_move_constructible_v<ptiff::Writer>);

} // namespace

TEST_CASE("api surface compiles", "[api-surface]") {
    // Presence of this file in the build is the check; static_asserts above do the work.
    SUCCEED();
}
