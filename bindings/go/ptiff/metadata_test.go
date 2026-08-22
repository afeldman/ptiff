// Package ptiff -- OpenPathFields coverage against a real interop fixture
// (scripts/samples). Asserts the camera geometry fields surface as native Go
// []Field pairs through the same on-disk format every binding consumes.
package ptiff

import (
	"path/filepath"
	"testing"
)

func fixturePath(t *testing.T) string {
	t.Helper()
	// go test runs with CWD = package dir; fixture lives ../../.. up at the
	// repo scripts/samples.
	p := filepath.Join("..", "..", "..", "scripts", "samples", "ptiff_interop_fixture.tif")
	abs, err := filepath.Abs(p)
	if err != nil {
		t.Fatalf("fixture path: %v", err)
	}
	return abs
}

func TestOpenPathFieldsReadsCameraMetadata(t *testing.T) {
	fields, err := OpenPathFields(fixturePath(t))
	if err != nil {
		t.Fatalf("OpenPathFields: %v", err)
	}
	if len(fields) == 0 {
		t.Fatal("expected non-empty ptiff.* fields from fixture")
	}

	get := func(key string) (string, bool) {
		for _, f := range fields {
			if f.Key == key {
				return f.Value, true
			}
		}
		return "", false
	}

	cases := map[string]string{
		"ptiff.camera.model":          "pinhole",
		"ptiff.camera.focal_length_x": "700.0",
		"ptiff.camera.focal_length_y": "700.0",
		"ptiff.camera.principal_x":    "64.000000",
		"ptiff.camera.principal_y":    "64.000000",
		"ptiff.spice.frame":           "IAU_MOON",
	}
	for key, want := range cases {
		got, ok := get(key)
		if !ok {
			t.Errorf("missing field %q in %v", key, fields)
			continue
		}
		if got != want {
			t.Errorf("field %q = %q, want %q", key, got, want)
		}
	}
}

func TestOpenPathFieldsMissingFileErrors(t *testing.T) {
	_, err := OpenPathFields(filepath.Join(t.TempDir(), "does-not-exist.tif"))
	if err == nil {
		t.Fatal("expected error for missing file")
	}
}

func TestOpenPathCameraReadsStructuredCalibration(t *testing.T) {
	// The fixture carries intrinsics only (fx=700, fy=700, cx=64, cy=64), no
	// extrinsics fields. The structured camera must surface the 3x3 K matrix
	// and a zero-translation [R|t]; P = K * [R|t] collapses onto K.
	cam, err := OpenPathCamera(fixturePath(t))
	if err != nil {
		t.Fatalf("OpenPathCamera: %v", err)
	}
	if !cam.HasIntrinsics {
		t.Fatal("expected intrinsics in fixture")
	}
	if cam.FocalLengthX != 700.0 || cam.FocalLengthY != 700.0 {
		t.Fatalf("focal length = %v x %v, want 700 x 700", cam.FocalLengthX, cam.FocalLengthY)
	}
	if cam.PrincipalX != 64.0 || cam.PrincipalY != 64.0 {
		t.Fatalf("principal point = %v x %v, want 64 x 64", cam.PrincipalX, cam.PrincipalY)
	}
	// K = [ fx 0 cx ; 0 fy cy ; 0 0 1 ]
	if cam.Intrinsics[0] != 700.0 || cam.Intrinsics[4] != 700.0 ||
		cam.Intrinsics[2] != 64.0 || cam.Intrinsics[5] != 64.0 ||
		cam.Intrinsics[8] != 1.0 {
		t.Fatalf("K not as expected: %v", cam.Intrinsics)
	}
	// Fixture has no extrinsics -> has_extrinsics false, [R|t] identity, t=0.
	if cam.HasExtrinsics {
		t.Fatal("fixture should not advertise extrinsics")
	}
	if cam.Extrinsics[0] != 1.0 || cam.Extrinsics[5] != 1.0 || cam.Extrinsics[10] != 1.0 {
		t.Fatalf("[R|t] rotation not identity: %v", cam.Extrinsics)
	}
	if cam.Extrinsics[3] != 0.0 || cam.Extrinsics[11] != 0.0 {
		t.Fatalf("[R|t] translation should be zero: %v", cam.Extrinsics)
	}
	// P first row = fx, 0, cx ; last diag of K = 1.
	if cam.Projection[0] != 700.0 || cam.Projection[2] != 64.0 || cam.Projection[10] != 1.0 {
		t.Fatalf("projection P not as expected: %v", cam.Projection)
	}
}

func TestOpenPathCameraMissingFileErrors(t *testing.T) {
	_, err := OpenPathCamera(filepath.Join(t.TempDir(), "does-not-exist.tif"))
	if err == nil {
		t.Fatal("expected error for missing camera file")
	}
}
