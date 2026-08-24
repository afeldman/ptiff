// Package ptiff -- Image surface over the raw SWIG-generated Go binding.
//
// Ports bindings/go/ptiff/image_test.go. Two hand-written cases have no safe
// raw-SWIG counterpart and are intentionally not ported:
//   - NewImage(nil): the raw binding takes a value-typed descriptor (an
//     interface wrapping a C pointer); passing the zero value panics in the Go
//     runtime before it ever reaches the C ABI, so it exercises SWIG's Go
//     typemap plumbing, not the ABI's NULL-argument handling.
//   - Close twice: ptiff_image_destroy(NULL) is a documented no-op, but
//     calling it twice on the SAME live pointer is a C-level double-free.
//     The hand-written binding guards this by nilling its internal pointer
//     after Close(); the raw SWIG handle has no such guard, so double-destroy
//     is not safe to exercise here (see the "close once" warning in
//     roundtrip_test.go).
package ptiff

import "testing"

func TestImageRoundTrip(t *testing.T) {
	desc := NewPtiff_image_descriptor()
	desc.SetWidth(640)
	desc.SetHeight(480)
	desc.SetPixel_type(int(PTIFF_PIXEL_UINT16))
	desc.SetChannel_count(3)
	desc.SetHas_gsd(1)
	desc.SetGsd(5.0)
	desc.SetHas_tile_info(1)
	tiles := NewPtiff_tile_info()
	tiles.SetTile_width(128)
	tiles.SetTile_height(64)
	desc.SetTile_info(tiles)
	desc.SetHas_compression(1)
	desc.SetCompression(int(PTIFF_COMPRESSION_DEFLATE))

	img := Ptiff_image_create(desc)
	if img == nil {
		t.Fatal("image_create returned nil")
	}
	defer Ptiff_image_destroy(img)

	if got := Ptiff_image_width(img); got != 640 {
		t.Errorf("width = %d, want 640", got)
	}
	if got := Ptiff_image_height(img); got != 480 {
		t.Errorf("height = %d, want 480", got)
	}
	if got := Ptiff_image_pixel_type(img); got != int(PTIFF_PIXEL_UINT16) {
		t.Errorf("pixel_type = %d, want %d", got, int(PTIFF_PIXEL_UINT16))
	}
	if got := Ptiff_image_channel_count(img); got != 3 {
		t.Errorf("channel_count = %d, want 3", got)
	}

	var gsd float64
	if ok := Ptiff_image_gsd(img, &gsd); ok == 0 || gsd != 5.0 {
		t.Errorf("gsd = %v, %v; want 5.0, present", gsd, ok != 0)
	}

	ti := NewPtiff_tile_info()
	if ok := Ptiff_image_tile_info(img, ti); ok == 0 || ti.GetTile_width() != 128 || ti.GetTile_height() != 64 {
		t.Errorf("tile_info = %dx%d, %v; want 128x64, present", ti.GetTile_width(), ti.GetTile_height(), ok != 0)
	}

	var comp int
	if ok := Ptiff_image_compression(img, &comp); ok == 0 || comp != int(PTIFF_COMPRESSION_DEFLATE) {
		t.Errorf("compression = %v, %v; want deflate, present", comp, ok != 0)
	}
}

func TestImageOptionalFieldsAbsent(t *testing.T) {
	desc := NewPtiff_image_descriptor()
	desc.SetWidth(10)
	desc.SetHeight(10)

	img := Ptiff_image_create(desc)
	if img == nil {
		t.Fatal("image_create returned nil")
	}
	defer Ptiff_image_destroy(img)

	var gsd float64
	if ok := Ptiff_image_gsd(img, &gsd); ok != 0 {
		t.Error("gsd present; want absent")
	}
	ti := NewPtiff_tile_info()
	if ok := Ptiff_image_tile_info(img, ti); ok != 0 {
		t.Error("tile_info present; want absent")
	}
	var comp int
	if ok := Ptiff_image_compression(img, &comp); ok != 0 {
		t.Error("compression present; want absent")
	}
}

func TestPixelTypeOrdering(t *testing.T) {
	cases := []struct {
		pt   PixelType
		want int
	}{
		{PixelType(PTIFF_PIXEL_UINT8), 0},
		{PixelType(PTIFF_PIXEL_UINT16), 1},
		{PixelType(PTIFF_PIXEL_UINT32), 2},
		{PixelType(PTIFF_PIXEL_FLOAT32), 3},
		{PixelType(PTIFF_PIXEL_FLOAT64), 4},
	}
	for _, tc := range cases {
		if int(tc.pt) != tc.want {
			t.Errorf("pixel type = %d, want %d", int(tc.pt), tc.want)
		}
	}
}

func TestCompressionKindOrdering(t *testing.T) {
	cases := []struct {
		c    int
		want int
	}{
		{int(PTIFF_COMPRESSION_NONE), 0},
		{int(PTIFF_COMPRESSION_LZW), 1},
		{int(PTIFF_COMPRESSION_DEFLATE), 2},
		{int(PTIFF_COMPRESSION_JPEG), 3},
	}
	for _, tc := range cases {
		if int(tc.c) != tc.want {
			t.Errorf("compression kind = %d, want %d", int(tc.c), tc.want)
		}
	}
}
