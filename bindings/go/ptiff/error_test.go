// Package ptiff -- error code surface over the raw SWIG-generated Go binding.
//
// Ports bindings/go/ptiff/error_test.go's ErrorCode coverage. There is no
// Error type or String() method at the raw SWIG level (that is hand-written
// Go binding sugar): what the C ABI actually promises is the numeric
// ordering of ptiff_error_code, which must match ptiff::ErrorCode -- this
// pins that ordering so a future re-numbering is caught here.
package ptiff

import "testing"

func TestErrorCodeOrdering(t *testing.T) {
	cases := []struct {
		code Ptiff_error_code
		want int
	}{
		{PTIFF_ERROR_NOT_IMPLEMENTED, 0},
		{PTIFF_ERROR_INVALID_ARGUMENT, 1},
		{PTIFF_ERROR_OUT_OF_RANGE, 2},
		{PTIFF_ERROR_NOT_FOUND, 3},
		{PTIFF_ERROR_UNKNOWN, 4},
	}
	for _, tc := range cases {
		if int(tc.code) != tc.want {
			t.Errorf("error code = %d, want %d", int(tc.code), tc.want)
		}
	}
}
