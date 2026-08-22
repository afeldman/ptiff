// Package ptiff -- ptiff_sink error-path coverage over the raw SWIG-generated
// Go binding, complementing the happy-path round trip in roundtrip_test.go.
//
// Ports the safe subset of bindings/go/ptiff/sink_test.go. Not ported:
// TestSinkCloseTwice and TestSinkWriteAfterClose -- both rely on the
// hand-written binding's Close() nilling its internal pointer so a second
// call is a guarded no-op. The raw SWIG handle has no such guard:
// ptiff_sink_close on an already-closed pointer is a C-level double-free, and
// writing through an already-closed pointer is a use-after-free. Neither is
// safe to exercise against the real C ABI (see the "close once" warning in
// roundtrip_test.go).
package ptiff

import (
	"os"
	"path/filepath"
	"testing"
)

// newTestSinkDescriptor builds a 32x32 UInt8, single-channel, 16x16-tiled
// descriptor -- same shape as roundtrip_test.go's, so tile grid is 2x2 and
// tile byte size is 256.
func newTestSinkDescriptor() Ptiff_image_descriptor {
	desc := NewPtiff_image_descriptor()
	desc.SetWidth(32)
	desc.SetHeight(32)
	desc.SetPixel_type(int(PTIFF_PIXEL_UINT8))
	desc.SetChannel_count(1)
	desc.SetHas_tile_info(1)
	tiles := NewPtiff_tile_info()
	tiles.SetTile_width(16)
	tiles.SetTile_height(16)
	desc.SetTile_info(tiles)
	return desc
}

func TestSinkWriteTileWrongBufferSize(t *testing.T) {
	path := filepath.Join(t.TempDir(), "bad.tif")
	sink := Ptiff_sink_create(path, newTestSinkDescriptor())
	if sink.Swigcptr() == 0 {
		t.Fatal("sink_create returned nil")
	}
	defer Ptiff_sink_close(sink)

	bs := Ptiff_sink_tile_byte_size(sink)

	tooShort := make([]byte, bs-1)
	if rc := Ptiff_sink_write_tile(sink, 0, 0, tooShort); rc == 0 {
		t.Error("write_tile with a short buffer expected a nonzero return code")
	}

	tooLong := make([]byte, bs+1)
	if rc := Ptiff_sink_write_tile(sink, 0, 0, tooLong); rc == 0 {
		t.Error("write_tile with an oversized buffer expected a nonzero return code")
	}
}

func TestSinkWriteTileOutOfRange(t *testing.T) {
	path := filepath.Join(t.TempDir(), "oor.tif")
	sink := Ptiff_sink_create(path, newTestSinkDescriptor())
	if sink.Swigcptr() == 0 {
		t.Fatal("sink_create returned nil")
	}
	defer Ptiff_sink_close(sink)

	buf := make([]byte, Ptiff_sink_tile_byte_size(sink))
	if rc := Ptiff_sink_write_tile(sink, 99, 0, buf); rc == 0 {
		t.Error("write_tile with an out-of-range column expected a nonzero return code")
	}
	if rc := Ptiff_sink_write_tile(sink, 0, 99, buf); rc == 0 {
		t.Error("write_tile with an out-of-range row expected a nonzero return code")
	}
}

func TestCreateSinkEmptyPath(t *testing.T) {
	if sink := Ptiff_sink_create("", newTestSinkDescriptor()); sink.Swigcptr() != 0 {
		Ptiff_sink_close(sink)
		t.Fatal("sink_create(\"\") expected nil")
	}
}

func TestCreateSinkUntiledDescriptor(t *testing.T) {
	// A descriptor without tile info (has_tile_info == 0) must be rejected by
	// the C ABI, which requires a tiled layout for TIFF tile I/O.
	desc := NewPtiff_image_descriptor()
	desc.SetWidth(16)
	desc.SetHeight(16)
	desc.SetPixel_type(int(PTIFF_PIXEL_UINT8))
	desc.SetChannel_count(1)

	path := filepath.Join(t.TempDir(), "untiled.tif")
	if sink := Ptiff_sink_create(path, desc); sink.Swigcptr() != 0 {
		Ptiff_sink_close(sink)
		t.Fatal("sink_create on an untiled descriptor expected nil")
	}
	if _, err := os.Stat(path); err == nil {
		t.Error("untiled sink_create must not leave a file behind")
	}
}
