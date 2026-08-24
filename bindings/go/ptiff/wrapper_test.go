package ptiff

import (
	"os"
	"path/filepath"
	"testing"
)

// TestIdiomaticImageRoundtrip exercises the idiomatic Image/Tile layer
// (OpenImage / CreateImage / ReadTile / WriteTile / metrics), mirroring the
// Python `ptiff.Image` wrapper tests.
func TestIdiomaticImageRoundtrip(t *testing.T) {
	out := filepath.Join(t.TempDir(), "wrappertest.tif")
	defer os.Remove(out)

	img, err := CreateImage(out, 32, 32, PixelType(PTIFF_PIXEL_UINT8), CreateOptions{
		TileWidth:  16,
		TileHeight: 16,
	})
	if err != nil {
		t.Fatalf("CreateImage: %v", err)
	}
	if img.Width() != 32 || img.Height() != 32 {
		t.Fatalf("size = %dx%d, want 32x32", img.Width(), img.Height())
	}
	if img.ChannelCount() != 1 {
		t.Fatalf("channels = %d, want 1", img.ChannelCount())
	}
	if img.TileColumns() != 2 || img.TileRows() != 2 {
		t.Fatalf("grid = %dx%d, want 2x2", img.TileColumns(), img.TileRows())
	}
	bs := img.TileByteSize()
	if bs != 16*16 {
		t.Fatalf("tile byte_size = %d, want %d", bs, 16*16)
	}

	for c := uint(0); c < 2; c++ {
		for r := uint(0); r < 2; r++ {
			pattern := make([]byte, int(bs))
			for i := range pattern {
				pattern[i] = byte((c*2 + r) % 256)
			}
			if err := img.WriteTile(c, r, pattern); err != nil {
				t.Fatalf("WriteTile(%d,%d): %v", c, r, err)
			}
		}
	}
	img.Close()

	rd, err := OpenImage(out)
	if err != nil {
		t.Fatalf("OpenImage: %v", err)
	}
	if rd.Width() != 32 || rd.Height() != 32 {
		t.Fatalf("read-back size = %dx%d, want 32x32", rd.Width(), rd.Height())
	}
	tile, err := rd.ReadTile(1, 0)
	if err != nil {
		t.Fatalf("ReadTile(1,0): %v", err)
	}
	if tile.Column != 1 || tile.Row != 0 {
		t.Fatalf("tile pos = %dx%d, want 1x0", tile.Column, tile.Row)
	}
	want := byte((1*2 + 0) % 256)
	if len(tile.Data) != int(bs) || tile.Data[0] != want {
		t.Fatalf("tile(1,0) data mismatch: len=%d first=%d want=%d", len(tile.Data), tile.Data[0], want)
	}
	rd.Close()
}

// TestIdiomaticLogger exercises the Logger wrapper (SetLevel/Level + the
// convenience trace..critical methods), mirroring Python's ptiff.Logger tests.
func TestIdiomaticLogger(t *testing.T) {
	var log Logger
	original := log.Level()
	defer log.SetLevel(original)

	log.SetLevel(LogError)
	if got := log.Level(); int(got) != int(PTIFF_LOG_ERROR) {
		t.Fatalf("level = %d, want error", got)
	}
	log.Trace("trace filtered out")
	log.Debug("debug filtered out")
	log.Error("error emitted")
	log.SetLevel(LogDebug)
	log.Debug("backend debug")
	log.Warning("a warning")
}

// TestIdiomaticMetadata exercises the Metadata wrapper over the fixture,
// mirroring Python's ptiff.Metadata tests.
func TestIdiomaticMetadata(t *testing.T) {
	fp := filepath.Join("..", "..", "..", "scripts", "samples", "ptiff_interop_fixture.tif")
	abs, err := filepath.Abs(fp)
	if err != nil {
		t.Fatalf("fixture path: %v", err)
	}

	m, err := NewMetadata(abs)
	if err != nil {
		t.Fatalf("NewMetadata: %v", err)
	}
	if m.Width() != 128 || m.Height() != 128 {
		t.Fatalf("size = %dx%d, want 128x128", m.Width(), m.Height())
	}
	if int(m.PixelType()) != int(PTIFF_PIXEL_UINT8) || m.PixelTypeName() != "UInt8" {
		t.Fatalf("pixel = %v (%q), want UInt8", m.PixelType(), m.PixelTypeName())
	}
	if v, ok := m.Field("ptiff.camera.model"); !ok || v != "pinhole" {
		t.Fatalf("ptiff.camera.model = %q (ok=%v), want pinhole", v, ok)
	}
	if len(m.Fields()) == 0 {
		t.Fatal("expected non-empty fields")
	}
}

// TestIdiomaticCameraWriteRoundtrip writes a tiled image through the idiomatic
// CreateImage(..., camera=&cam), persisting the calibration as ptiff.camera.*
// metadata, then reads it back via OpenImage and verifies Image.Camera()
// matches. Exercises Camera.toMetadata() on the write path.
func TestIdiomaticCameraWriteRoundtrip(t *testing.T) {
	out := filepath.Join(t.TempDir(), "cam_wrap.tif")
	defer os.Remove(out)

	cam := Camera{
		HasIntrinsics: true,
		FocalLengthX:  700.0,
		FocalLengthY:  715.0,
		PrincipalX:    32.0,
		PrincipalY:    24.0,
		HasExtrinsics: true,
		RotationW:     1.0,
		PositionX:     1.0,
		PositionY:     2.0,
		PositionZ:     3.0,
		Timestamp:     "2026-08-21T12:34:56.000Z",
	}
	// Identity rotation + position t=(1,2,3): P = K*[R|t] -> translation column
	// P[0,3]=fx*tx+cx*tz, P[1,7]=fy*ty+cy*tz, P[2,11]=tz.
	cam.Intrinsics[0], cam.Intrinsics[4], cam.Intrinsics[8] = 700.0, 715.0, 1.0
	cam.Extrinsics[0], cam.Extrinsics[5], cam.Extrinsics[10] = 1.0, 1.0, 1.0
	cam.Extrinsics[3], cam.Extrinsics[7], cam.Extrinsics[11] = 1.0, 2.0, 3.0
	cam.Projection[0], cam.Projection[4], cam.Projection[10] = 700.0, 715.0, 1.0
	cam.Projection[3], cam.Projection[7], cam.Projection[11] = 700*1+32*3, 715*2+24*3, 3.0

	img, err := CreateImage(out, 32, 32, PixelType(PTIFF_PIXEL_UINT8), CreateOptions{
		TileWidth:  16,
		TileHeight: 16,
		Camera:     &cam,
	})
	if err != nil {
		t.Fatalf("CreateImage: %v", err)
	}
	bs := img.TileByteSize()
	for c := uint(0); c < 2; c++ {
		for r := uint(0); r < 2; r++ {
			if err := img.WriteTile(c, r, make([]byte, int(bs))); err != nil {
				t.Fatalf("WriteTile(%d,%d): %v", c, r, err)
			}
		}
	}
	img.Close()

	rd, err := OpenImage(out)
	if err != nil {
		t.Fatalf("OpenImage: %v", err)
	}
	got := rd.Camera()
	if !got.HasIntrinsics || !got.HasExtrinsics {
		t.Fatalf("has flags = %v/%v", got.HasIntrinsics, got.HasExtrinsics)
	}
	if got.FocalLengthX != 700.0 || got.FocalLengthY != 715.0 {
		t.Fatalf("fx/fy = %v/%v", got.FocalLengthX, got.FocalLengthY)
	}
	if got.PrincipalX != 32.0 || got.PrincipalY != 24.0 {
		t.Fatalf("cx/cy = %v/%v", got.PrincipalX, got.PrincipalY)
	}
	if got.Timestamp != "2026-08-21T12:34:56.000Z" {
		t.Fatalf("timestamp = %q", got.Timestamp)
	}
	if got.Projection[3] != 700*1+32*3 || got.Projection[7] != 715*2+24*3 || got.Projection[11] != 3 {
		t.Fatalf("projection P = %v", got.Projection)
	}
	rd.Close()
}
