// Package ptiff -- backend registry surface over the raw SWIG-generated Go binding.
//
// Ports bindings/go/ptiff/backend_test.go's TestBackendNames. The C ABI
// returns a single comma-space-joined C string (caller-owned, freed via
// Ptiff_free_string) rather than a slice -- the hand-written binding does that
// splitting/freeing internally; here it is done explicitly.
package ptiff

import (
	"strings"
	"testing"
)

func TestBackendNames(t *testing.T) {
	joined := Ptiff_backend_names()
	// registeredBackends() may legitimately be empty when linking statically
	// without whole-archive (backend registration TUs get dead-stripped). The
	// call must never panic regardless.
	t.Logf("registered backends (joined): %q", joined)

	if joined == "" {
		return
	}
	for _, n := range strings.Split(joined, ", ") {
		if n == "" {
			t.Fatalf("backend_names() contained an empty name in %q", joined)
		}
	}
}
