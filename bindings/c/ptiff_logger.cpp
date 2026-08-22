#include "ptiff_logger.h"

#include <ptiff/logging/logger.hpp>

extern "C" {

void ptiff_logger_set_level(int level) {
    ptiff::Logger::instance().setLevel(static_cast<ptiff::LogLevel>(level));
}

int ptiff_logger_level(void) {
    return static_cast<int>(ptiff::Logger::instance().level());
}

void ptiff_logger_log(int level, const char* message) {
    ptiff::Logger::instance().log(static_cast<ptiff::LogLevel>(level), message ? message : "");
}
}
