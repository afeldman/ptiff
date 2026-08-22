// Package ptiff -- version surface over the raw SWIG-generated Go binding.
//
// Ports the version coverage of the hand-written binding's version_test.go
// (bindings/go/ptiff/version_test.go) onto the SWIG output: both the
// struct-return (Ptiff_runtime_version / Ptiff_compile_time_version) and the
// out-parameter (*_out) forms of the C ABI, since the latter has no
// hand-written-binding equivalent to port from but is part of the ABI surface
// ("version") the migration checklist requires covering.
package ptiff

import "testing"

func TestRuntimeVersionNonNegative(t *testing.T) {
	v := Ptiff_runtime_version()
	if v.GetMajor() < 0 || v.GetMinor() < 0 || v.GetPatch() < 0 {
		t.Fatalf("runtime version = %d.%d.%d, negative component", v.GetMajor(), v.GetMinor(), v.GetPatch())
	}
	t.Logf("runtime version: %d.%d.%d", v.GetMajor(), v.GetMinor(), v.GetPatch())
}

func TestCompileTimeVersionNonNegative(t *testing.T) {
	v := Ptiff_compile_time_version()
	if v.GetMajor() < 0 || v.GetMinor() < 0 || v.GetPatch() < 0 {
		t.Fatalf("compile-time version = %d.%d.%d, negative component", v.GetMajor(), v.GetMinor(), v.GetPatch())
	}
}

func TestRuntimeMatchesCompileTime(t *testing.T) {
	// runtimeVersion() currently forwards to compileTimeVersion() in libptiff, so
	// they must agree -- same invariant checked by the hand-written binding.
	r, c := Ptiff_runtime_version(), Ptiff_compile_time_version()
	if r.GetMajor() != c.GetMajor() || r.GetMinor() != c.GetMinor() || r.GetPatch() != c.GetPatch() {
		t.Errorf("runtime %d.%d.%d != compile-time %d.%d.%d",
			r.GetMajor(), r.GetMinor(), r.GetPatch(), c.GetMajor(), c.GetMinor(), c.GetPatch())
	}
}

func TestVersionOutParams(t *testing.T) {
	var rMaj, rMin, rPat int
	Ptiff_runtime_version_out(&rMaj, &rMin, &rPat)

	var cMaj, cMin, cPat int
	Ptiff_compile_time_version_out(&cMaj, &cMin, &cPat)

	if rMaj != cMaj || rMin != cMin || rPat != cPat {
		t.Errorf("runtime_out %d.%d.%d != compile_time_out %d.%d.%d", rMaj, rMin, rPat, cMaj, cMin, cPat)
	}

	// Out-param form must agree with the struct-return form.
	v := Ptiff_runtime_version()
	if rMaj != v.GetMajor() || rMin != v.GetMinor() || rPat != v.GetPatch() {
		t.Errorf("runtime_version_out %d.%d.%d != runtime_version() %d.%d.%d",
			rMaj, rMin, rPat, v.GetMajor(), v.GetMinor(), v.GetPatch())
	}
}
