#include <ptiff/core/version.hpp>

namespace ptiff {

Version runtimeVersion() {
    return compileTimeVersion();
}

} // namespace ptiff
