// Package ptiff -- logger surface over the raw SWIG-generated Go binding.
//
// Ports bindings/go/ptiff/logger_test.go's TestLoggerRoundTrip. The
// LogLevel.String() case from the hand-written test has no counterpart here:
// string formatting is Go-binding sugar, not part of the C ABI.
package ptiff

import "testing"

func TestLoggerRoundTrip(t *testing.T) {
	original := Ptiff_logger_level()
	defer Ptiff_logger_set_level(original)

	Ptiff_logger_set_level(int(PTIFF_LOG_ERROR))
	if got := Ptiff_logger_level(); got != int(PTIFF_LOG_ERROR) {
		t.Fatalf("logger_level() after set_level(ERROR) = %d, want %d", got, int(PTIFF_LOG_ERROR))
	}

	// Emitting a log at a level below the current threshold must be safe (no crash).
	Ptiff_logger_log(int(PTIFF_LOG_TRACE), "this trace line is filtered out")
	Ptiff_logger_log(int(PTIFF_LOG_INFO), "this info line is filtered out")
	Ptiff_logger_log(int(PTIFF_LOG_ERROR), "this error line is emitted")

	Ptiff_logger_set_level(int(PTIFF_LOG_DEBUG))
	Ptiff_logger_log(int(PTIFF_LOG_DEBUG), "backend debugging")
	Ptiff_logger_log(int(PTIFF_LOG_WARN), "a warning")
}
