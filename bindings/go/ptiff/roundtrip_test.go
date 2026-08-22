// Package ptiff exercises the SWIG-generated Go binding (bindings/swig/go).
//
// This test is part of the "everything SWIG gives us" multi-language build. It
// drives the SWIG-generated Go functions directly (NewPtiff_image_descriptor,
// Ptiff_sink_write_tile, Ptiff_source_read_tile, ...) -- the raw, mechanical
// surface SWIG produces for the libptiff C ABI. It is NOT the idiomatic
// hand-written Go binding (bindings/go/ptiff); it demonstrates what SWIG emits
// and that a full write+read round-trip works through it.
//
// Build before testing: `make go` in bindings/swig/ (generates ptiff.go,
// ptiff_wrap.c, go.mod, cgo_flags.go in this dir), then `go test ./...`.
package ptiff

import (
	"bytes"
	"os"
	"testing"
)

// TestRoundtrip writes a 32x32 UInt8 TIFF with 4x16x16 tiles through the
// ptiff_sink_* surface and reads it back through ptiff_source_*, mirroring the
// Python (test_roundtrip.py) and Ruby (test_roundtrip.rb) round-trips.
//
// NB: sink/src handles are raw value structs wrapping a uintptr; close each ONE
// time. Do not defer-close AND close (double-free/sigsegv in C).
func TestRoundtrip(t *testing.T) {
	if os.Getenv("PTIFF_SWIG_GO_SUBPROCESS") == "1" {
		t.Skip("probe")
	}
	out := "roundtrip_go.tif"
	os.Remove(out)

	// ---- Build the image descriptor from the SWIG-generated class ----
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
	desc.SetHas_compression(0)

	// ---- Write side (Ptiff_sink_*) ----
	sink := Ptiff_sink_create(out, desc)
	if sink == nil {
		t.Fatal("sink_create returned nil")
	}

	cols := Ptiff_sink_tile_columns(sink)
	rows := Ptiff_sink_tile_rows(sink)
	bs := Ptiff_sink_tile_byte_size(sink)
	t.Logf("sink tile grid %dx%d, byte_size=%d", cols, rows, bs)
	if cols != 2 || rows != 2 {
		t.Fatalf("grid = %dx%d, want 2x2", cols, rows)
	}
	if bs != 16*16 {
		t.Fatalf("byte_size = %d, want %d", bs, 16*16)
	}

	for c := uint(0); c < cols; c++ {
		for r := uint(0); r < rows; r++ {
			pat := bytes.Repeat([]byte{byte((c*rows + r) % 256)}, int(bs))
			if rc := Ptiff_sink_write_tile(sink, c, r, pat); rc != 0 {
				t.Fatalf("write_tile(%d,%d) rc=%d", c, r, rc)
			}
		}
	}
	// close the sink ONCE: it flushes the file so the source can read it. No
	// defer-close -- the value handle is freed by C and double-close segfaults.
	Ptiff_sink_close(sink)
	if _, err := os.Stat(out); err != nil {
		t.Fatalf("no output file: %v", err)
	}

	// ---- Read side (Ptiff_source_*) ----
	var errOut int
	src := Ptiff_source_open(out, &errOut)
	if src == nil {
		t.Fatalf("source_open failed err=%d", errOut)
	}

	srcDesc := NewPtiff_image_descriptor()
	if rc := Ptiff_source_descriptor(src, srcDesc); rc != 0 {
		t.Fatalf("source_descriptor rc=%d", rc)
	}
	if srcDesc.GetWidth() != 32 || srcDesc.GetHeight() != 32 {
		t.Fatalf("desc %dx%d, want 32x32", srcDesc.GetWidth(), srcDesc.GetHeight())
	}
	ti := srcDesc.GetTile_info()
	if ti.GetTile_width() != 16 || ti.GetTile_height() != 16 {
		t.Fatalf("tile %dx%d, want 16x16", ti.GetTile_width(), ti.GetTile_height())
	}

	rcols := Ptiff_source_tile_columns(src)
	rrows := Ptiff_source_tile_rows(src)
	rbs := Ptiff_source_tile_byte_size(src)
	t.Logf("source tile grid %dx%d, byte_size=%d", rcols, rrows, rbs)
	if rcols != 2 || rrows != 2 {
		t.Fatalf("src grid = %dx%d, want 2x2", rcols, rrows)
	}

	// read tile (1,0) -- writable []byte filled in place; bytes_read via *int64
	buf := make([]byte, rbs)
	var nread int64
	if rc := Ptiff_source_read_tile(src, 1, 0, buf, &nread); rc != 0 {
		t.Fatalf("read_tile rc=%d", rc)
	}
	expected := bytes.Repeat([]byte{byte((1*rrows + 0) % 256)}, int(rbs))
	if !bytes.Equal(buf, expected) {
		t.Fatalf("read-back tile (1,0) mismatch\n got=%v\nwant=%v", buf[:8], expected[:8])
	}

	// close the source once
	Ptiff_source_close(src)

	os.Remove(out)
	t.Log("ROUNDTRIP OK")
}
