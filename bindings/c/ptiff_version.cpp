#include "ptiff_version.h"

#include <ptiff/core/version.hpp>

extern "C" {

ptiff_version ptiff_runtime_version(void) {
    const ptiff::Version v = ptiff::runtimeVersion();
    return ptiff_version{v.major, v.minor, v.patch};
}

ptiff_version ptiff_compile_time_version(void) {
    const ptiff::Version v = ptiff::compileTimeVersion();
    return ptiff_version{v.major, v.minor, v.patch};
}

void ptiff_runtime_version_out(int* major, int* minor, int* patch) {
    const ptiff::Version v = ptiff::runtimeVersion();
    if (major != nullptr)
        *major = v.major;
    if (minor != nullptr)
        *minor = v.minor;
    if (patch != nullptr)
        *patch = v.patch;
}

void ptiff_compile_time_version_out(int* major, int* minor, int* patch) {
    const ptiff::Version v = ptiff::compileTimeVersion();
    if (major != nullptr)
        *major = v.major;
    if (minor != nullptr)
        *minor = v.minor;
    if (patch != nullptr)
        *patch = v.patch;
}
}
