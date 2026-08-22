/*
 * ptiff_bridge.cpp
 *
 * Implementation of the C ABI in ptiff_bridge.h. Allocates via std::malloc/std::free for
 * anything handed across the boundary so the Go side can free it symmetrically with C.free.
 */
#include "ptiff_bridge.h"

#include <cstdlib>
#include <cstring>
#include <string>
#include <vector>

#include <ptiff/io/backend_factory.hpp>

namespace {

// Caller owns the returned malloc'd buffer.
char* dupToCString(const std::string& s) {
    char* out = static_cast<char*>(std::malloc(s.size() + 1));
    if (out == nullptr) {
        return nullptr;
    }
    std::memcpy(out, s.data(), s.size());
    out[s.size()] = '\0';
    return out;
}

} // namespace

extern "C" {

char* ptiff_backend_names(void) {
    const std::vector<std::string> names =
        ptiff::io::BackendFactory::instance().registeredBackends();
    std::string joined;
    for (std::size_t i = 0; i < names.size(); ++i) {
        if (i != 0) {
            joined += ", ";
        }
        joined += names[i];
    }
    return dupToCString(joined);
}

void ptiff_free_string(char* s) {
    std::free(s);
}

} // extern "C"
