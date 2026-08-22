// Package ptiff -- idiomatic metadata surface for the camera/SPICE/CRS
// extension fields (Professor requirement: metadata must be readable in every
// binding). The SWIG-generated `Ptiff_open_path_fields` exposes the C ABI's
// malloc'd `ptiff_field**`/`int*` out-parameters verbatim, which is unusable
// from Go (no way to iterate the C array or free it through that signature).
//
// This hand-authored helper calls `ptiff_open_path_fields` directly over cgo
// and surfaces the result as a native `[]Field` slice (key/value string
// pairs), freeing the C array symmetrically with `ptiff_fields_free`.
//
// It is intentionally small, dependency-free and sits alongside the promoted
// SWIG output without replacing it (see bindings/swig/README.md "alles in
// SWIG" migration; SWIG-Go has no AppendOutput/argout equivalent for
// heap-array out-params, so the idiomatic slice lives here).
package ptiff

/*
#include <stdlib.h>
#include <string.h>

typedef struct ptiff_field {
	char* key;
	char* value;
} ptiff_field;

int ptiff_open_path_fields(const char* path, ptiff_field** out, int* out_count);
void ptiff_fields_free(ptiff_field* arr, int count);

typedef struct ptiff_camera {
	int      has_intrinsics;
	double   focal_length_x, focal_length_y, principal_x, principal_y;
	double   intrinsics[9];
	int      has_extrinsics;
	double   rotation_w, rotation_x, rotation_y, rotation_z;
	double   position_x, position_y, position_z;
	double   extrinsics[12];
	double   projection[12];
	char     timestamp[64];
} ptiff_camera;

int ptiff_open_path_camera(const char* path, ptiff_camera* out);
*/
import "C"

import (
	"fmt"
	"unsafe"
)

// Field is one flattened PTIFF extension entry, e.g. "ptiff.camera.model" ->
// "pinhole". It mirrors the C ABI's ptiff_field but with Go-idiomatic owned
// strings instead of malloc'd C strings.
type Field struct {
	Key   string
	Value string
}

// OpenPathFields opens the TIFF/BigTIFF file at path and returns the flattened
// PTIFF extension fields decoded from the private tags (SPICE, camera geometry,
// CRS, scientific layers, provenance). Keys carry the "ptiff.<domain>.<name>"
// shape and the slice is sorted lexicographically by key, exactly as the C ABI
// exposes them.
//
// The returned slice is always non-nil. On C-ABI failure a non-zero error code
// is returned as an error; the slice is then empty.
func OpenPathFields(path string) ([]Field, error) {
	cPath := C.CString(path)
	defer C.free(unsafe.Pointer(cPath))

	var out *C.ptiff_field
	var outCount C.int

	rc := C.ptiff_open_path_fields(cPath, &out, &outCount)
	if rc != 0 {
		return []Field{}, fmt.Errorf("ptiff_open_path_fields: error code %d", int(rc))
	}

	n := int(outCount)
	fields := make([]Field, 0, n)
	if n > 0 && out != nil {
		// Walk the C array; the two strings are malloc'd and owned by the
		// array, so copy them into Go strings before freeing the array.
		base := unsafe.Pointer(out)
		for i := 0; i < n; i++ {
			f := (*C.ptiff_field)(unsafe.Pointer(uintptr(base) + uintptr(i)*unsafe.Sizeof(C.ptiff_field{})))
			fields = append(fields, Field{
				Key:   C.GoString(f.key),
				Value: C.GoString(f.value),
			})
		}
	}

	C.ptiff_fields_free(out, C.int(n))
	return fields, nil
}

// Camera is a structured view of the `ptiff.camera.*` extension fields: the
// pinhole intrinsics, the extrinsics (rotation quaternion + world translation),
// the ISO-8601 observation timestamp, and the three derived matrices K / [R|t]
// / P (row-major doubles). The projection P = K * [R|t] is computed in the C++
// library.
type Camera struct {
	HasIntrinsics bool
	FocalLengthX  float64
	FocalLengthY  float64
	PrincipalX    float64
	PrincipalY    float64
	Intrinsics    [9]float64 // 3x3 K (row-major)
	HasExtrinsics bool
	RotationW     float64
	RotationX     float64
	RotationY     float64
	RotationZ     float64
	PositionX     float64
	PositionY     float64
	PositionZ     float64
	Extrinsics    [12]float64 // 3x4 [R|t] (row-major)
	Projection    [12]float64 // 3x4 P = K * [R|t] (row-major)
	Timestamp     string      // ISO-8601 UTC, empty when unset
}

// OpenPathCamera opens the TIFF/BigTIFF file at path and returns the structured
// camera calibration decoded from the `ptiff.camera.*` extension fields. On
// C-ABI failure a non-zero error code is returned as an error.
func OpenPathCamera(path string) (Camera, error) {
	cPath := C.CString(path)
	defer C.free(unsafe.Pointer(cPath))

	var cam C.ptiff_camera
	rc := C.ptiff_open_path_camera(cPath, &cam)
	if rc != 0 {
		return Camera{}, fmt.Errorf("ptiff_open_path_camera: error code %d", int(rc))
	}

	out := Camera{
		HasIntrinsics: cam.has_intrinsics != 0,
		FocalLengthX:  float64(cam.focal_length_x),
		FocalLengthY:  float64(cam.focal_length_y),
		PrincipalX:    float64(cam.principal_x),
		PrincipalY:    float64(cam.principal_y),
		HasExtrinsics: cam.has_extrinsics != 0,
		RotationW:     float64(cam.rotation_w),
		RotationX:     float64(cam.rotation_x),
		RotationY:     float64(cam.rotation_y),
		RotationZ:     float64(cam.rotation_z),
		PositionX:     float64(cam.position_x),
		PositionY:     float64(cam.position_y),
		PositionZ:     float64(cam.position_z),
		Timestamp:     C.GoString(&cam.timestamp[0]),
	}
	for i := range cam.intrinsics {
		out.Intrinsics[i] = float64(cam.intrinsics[i])
	}
	for i := range cam.extrinsics {
		out.Extrinsics[i] = float64(cam.extrinsics[i])
	}
	for i := range cam.projection {
		out.Projection[i] = float64(cam.projection[i])
	}
	return out, nil
}

// toMetadata builds a SWIG `Ptiff_camera` (the raw C-ABI struct) from this
// idiomatic Camera, ready to persist with CreateImage(..., camera=&cam). It is
// the Go counterpart of Python's Camera.to_metadata().
func (cam Camera) toMetadata() Ptiff_camera {
	c := NewPtiff_camera()
	c.SetHas_intrinsics(boolToInt(cam.HasIntrinsics))
	c.SetFocal_length_x(cam.FocalLengthX)
	c.SetFocal_length_y(cam.FocalLengthY)
	c.SetPrincipal_x(cam.PrincipalX)
	c.SetPrincipal_y(cam.PrincipalY)
	c.SetIntrinsics(&cam.Intrinsics[0])
	c.SetHas_extrinsics(boolToInt(cam.HasExtrinsics))
	c.SetRotation_w(cam.RotationW)
	c.SetRotation_x(cam.RotationX)
	c.SetRotation_y(cam.RotationY)
	c.SetRotation_z(cam.RotationZ)
	c.SetPosition_x(cam.PositionX)
	c.SetPosition_y(cam.PositionY)
	c.SetPosition_z(cam.PositionZ)
	c.SetExtrinsics(&cam.Extrinsics[0])
	c.SetProjection(&cam.Projection[0])
	c.SetTimestamp(cam.Timestamp)
	return c
}

func boolToInt(b bool) int {
	if b {
		return 1
	}
	return 0
}

// pixelTypeNames maps the PTIFF pixel-type enum values to human names, matching
// Python's Metadata.pixel_type_name and the C ABI's documentation.
var pixelTypeNames = map[Ptiff_pixel_type]string{
	PTIFF_PIXEL_UINT8:   "UInt8",
	PTIFF_PIXEL_UINT16:  "UInt16",
	PTIFF_PIXEL_UINT32:  "UInt32",
	PTIFF_PIXEL_FLOAT32: "Float32",
	PTIFF_PIXEL_FLOAT64: "Float64",
}

// Metadata is a read-only view of a single image file's core descriptor plus
// its flattened PTIFF extension fields. It mirrors Python's `ptiff.Metadata`
// and keeps no live image handle open.
type Metadata struct {
	path   string
	width  uint
	height uint
	ch     uint
	pixel  Ptiff_pixel_type
	fields map[string]string
}

// NewMetadata reads the metadata of the file at path without materialising a
// live image handle: the core descriptor (via ptiff_open_path) and the
// flattened extension fields (via ptiff_open_path_fields). On C-ABI failure a
// non-zero error code is returned as an error.
func NewMetadata(path string) (*Metadata, error) {
	desc := NewPtiff_image_descriptor()
	if rc := Ptiff_open_path(path, desc); rc != 0 {
		return nil, fmt.Errorf("ptiff_open_path: error code %d", rc)
	}

	fields, err := OpenPathFields(path)
	if err != nil {
		return nil, err
	}
	fmap := make(map[string]string, len(fields))
	for _, f := range fields {
		fmap[f.Key] = f.Value
	}

	return &Metadata{
		path:   path,
		width:  desc.GetWidth(),
		height: desc.GetHeight(),
		ch:     desc.GetChannel_count(),
		pixel:  Ptiff_pixel_type(desc.GetPixel_type()),
		fields: fmap,
	}, nil
}

// Path returns the file path this Metadata describes.
func (m *Metadata) Path() string { return m.path }

// Width returns the image width in pixels.
func (m *Metadata) Width() uint { return m.width }

// Height returns the image height in pixels.
func (m *Metadata) Height() uint { return m.height }

// ChannelCount returns the number of channels.
func (m *Metadata) ChannelCount() uint { return m.ch }

// PixelType returns the pixel type (a PTIFF_PIXEL_* value).
func (m *Metadata) PixelType() Ptiff_pixel_type { return m.pixel }

// PixelTypeName returns the human name of the pixel type (e.g. "UInt16"), or
// "Unknown" for an unrecognised value.
func (m *Metadata) PixelTypeName() string {
	if name, ok := pixelTypeNames[m.pixel]; ok {
		return name
	}
	return "Unknown"
}

// Fields returns a copy of the flattened PTIFF extension fields as a key->value
// map (e.g. "ptiff.camera.model" -> "pinhole"). Returning a copy keeps the
// Metadata safe against caller mutation.
func (m *Metadata) Fields() map[string]string {
	out := make(map[string]string, len(m.fields))
	for k, v := range m.fields {
		out[k] = v
	}
	return out
}

// Field returns a single extension field value, or "" (and false) when absent.
func (m *Metadata) Field(key string) (string, bool) {
	v, ok := m.fields[key]
	return v, ok
}
