package ptiff

import (
	"os"
	"path/filepath"
	"testing"
)

// TestCameraWriteRoundtrip writes a tiled image through the new
// ptiff_sink_create_camera write surface, persisting intrinsics + extrinsics
// as `ptiff.camera.*` metadata, then reads it back via the idiomatic
// OpenPathCamera helper and verifies both the scalar fields and the derived
// projection P = K * [R|t]. Mirrors the C/Python/Ruby/Octave camera round trips.
func TestCameraWriteRoundtrip(t *testing.T) {
	out := filepath.Join(t.TempDir(), "cam_rt.tif")
	defer os.Remove(out)

	desc := NewPtiff_image_descriptor()
	desc.SetWidth(32)
	desc.SetHeight(32)
	desc.SetPixel_type(0) // PTIFF_PIXEL_UINT8
	desc.SetChannel_count(1)
	desc.SetHas_tile_info(1)
	tiles := NewPtiff_tile_info()
	tiles.SetTile_width(16)
	tiles.SetTile_height(16)
	desc.SetTile_info(tiles)

	cam := NewPtiff_camera()
	cam.SetHas_intrinsics(1)
	cam.SetFocal_length_x(700.0)
	cam.SetFocal_length_y(715.0)
	cam.SetPrincipal_x(32.0)
	cam.SetPrincipal_y(24.0)
	cam.SetHas_extrinsics(1)
	cam.SetRotation_w(1.0)
	cam.SetPosition_x(1.0)
	cam.SetPosition_y(2.0)
	cam.SetPosition_z(3.0)
	cam.SetTimestamp("2026-08-21T12:34:56.000Z")

	sink := Ptiff_sink_create_camera(out, desc, cam)
	if sink == nil {
		t.Fatal("sink_create_camera returned nil")
	}
	bs := Ptiff_sink_tile_byte_size(sink)
	for c := uint(0); c < 2; c++ {
		for r := uint(0); r < 2; r++ {
			if rc := Ptiff_sink_write_tile(sink, c, r, make([]byte, int(bs))); rc != 0 {
				t.Fatalf("write_tile(%d,%d) rc=%d", c, r, rc)
			}
		}
	}
	Ptiff_sink_close(sink)

	got, err := OpenPathCamera(out)
	if err != nil {
		t.Fatalf("OpenPathCamera: %v", err)
	}
	if !got.HasIntrinsics || !got.HasExtrinsics {
		t.Fatalf("has flags = %v/%v", got.HasIntrinsics, got.HasExtrinsics)
	}
	if got.FocalLengthX != 700.0 || got.FocalLengthY != 715.0 {
		t.Errorf("fx/fy = %v/%v", got.FocalLengthX, got.FocalLengthY)
	}
	if got.PrincipalX != 32.0 || got.PrincipalY != 24.0 {
		t.Errorf("cx/cy = %v/%v", got.PrincipalX, got.PrincipalY)
	}
	if got.RotationW != 1.0 || got.PositionX != 1.0 ||
		got.PositionY != 2.0 || got.PositionZ != 3.0 {
		t.Errorf("rotation/position mismatch: %v %v %v %v",
			got.RotationW, got.PositionX, got.PositionY, got.PositionZ)
	}
	if got.Timestamp != "2026-08-21T12:34:56.000Z" {
		t.Errorf("timestamp = %q", got.Timestamp)
	}
	// Identity rotation R=I, position t=(1,2,3): translation column of
	// P = K*[R|t] is K*t -> P[0,3]=fx*tx+cx*tz, P[1,7]=fy*ty+cy*tz, P[2,11]=tz.
	if got.Projection[3] != 700*1+32*3 || got.Projection[7] != 715*2+24*3 || got.Projection[11] != 3 {
		t.Errorf("projection P = %v", got.Projection)
	}
}
