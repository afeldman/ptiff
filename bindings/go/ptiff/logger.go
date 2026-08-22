package ptiff

// Log-level aliases exposed by the idiomatic Logger, mirroring Python's
// `ptiff.Logger` class constants. They re-export the SWIG `PTIFF_LOG_*`
// values (which are runtime variables, not Go constants) under friendlier
// names.
var (
	LogTrace    = Ptiff_log_level(PTIFF_LOG_TRACE)
	LogDebug    = Ptiff_log_level(PTIFF_LOG_DEBUG)
	LogInfo     = Ptiff_log_level(PTIFF_LOG_INFO)
	LogWarn     = Ptiff_log_level(PTIFF_LOG_WARN)
	LogError    = Ptiff_log_level(PTIFF_LOG_ERROR)
	LogCritical = Ptiff_log_level(PTIFF_LOG_CRITICAL)
	LogOff      = Ptiff_log_level(PTIFF_LOG_OFF)
)

// Logger is a thin, stateless view over the library's global logger.
//
// The underlying C logger is a single process-wide instance, so any number of
// Logger values read and mutate that one instance. It mirrors Python's
// `ptiff.Logger` wrapping the SWIG low-level `ptiff_logger_*` free functions.
type Logger struct{}

// Level returns the current minimum level that gets emitted.
func (Logger) Level() Ptiff_log_level {
	return Ptiff_log_level(Ptiff_logger_level())
}

// SetLevel sets the minimum level that gets emitted (see the Log* constants).
func (Logger) SetLevel(level Ptiff_log_level) {
	Ptiff_logger_set_level(int(level))
}

// Log emits message at level; it is filtered out below the current threshold.
func (Logger) Log(level Ptiff_log_level, message string) {
	Ptiff_logger_log(int(level), message)
}

// Trace emits message at the trace level.
func (l Logger) Trace(message string) { l.Log(LogTrace, message) }

// Debug emits message at the debug level.
func (l Logger) Debug(message string) { l.Log(LogDebug, message) }

// Info emits message at the info level.
func (l Logger) Info(message string) { l.Log(LogInfo, message) }

// Warning emits message at the warn level.
func (l Logger) Warning(message string) { l.Log(LogWarn, message) }

// Error emits message at the error level.
func (l Logger) Error(message string) { l.Log(LogError, message) }

// Critical emits message at the critical level.
func (l Logger) Critical(message string) { l.Log(LogCritical, message) }
