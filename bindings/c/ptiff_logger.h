#ifndef PTIFF_LOGGER_H
#define PTIFF_LOGGER_H

#include <stddef.h>
#include <stdint.h>

#include "ptiff_c_export.h"

#ifdef __cplusplus
extern "C" {
#endif

/* ---- LogLevel (must match ptiff::LogLevel ordering) ---- */
enum ptiff_log_level {
    PTIFF_LOG_TRACE = 0,
    PTIFF_LOG_DEBUG = 1,
    PTIFF_LOG_INFO = 2,
    PTIFF_LOG_WARN = 3,
    PTIFF_LOG_ERROR = 4,
    PTIFF_LOG_CRITICAL = 5,
    PTIFF_LOG_OFF = 6,
};

PTIFF_C_API void ptiff_logger_set_level(int level);
PTIFF_C_API int ptiff_logger_level(void);
PTIFF_C_API void ptiff_logger_log(int level, const char* message);

#ifdef __cplusplus
}
#endif

#endif // PTIFF_LOGGER_H
