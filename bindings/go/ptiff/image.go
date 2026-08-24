package ptiff

import "fmt"

// Image is a PTIFF image: metadata plus tile access.
//
// It mirrors Python's `ptiff.Image` over the SWIG low-level source/sink
// surface. Use OpenImage to read a TIFF/BigTIFF and CreateImage to write a new
// one. Reading exposes the structured camera calibration via Camera and the raw
// tile bytes via ReadTile.
type Image struct {
	path   string
	src    Struct_SS_ptiff_source // non-nil when open for reading
	sink   Struct_SS_ptiff_sink   // non-nil when open for writing
	camera Camera
	desc   Ptiff_image_descriptor
}

// OpenImage opens the TIFF/BigTIFF file at path for reading and returns an
// Image wrapping the SWIG source handle. On C-ABI failure a non-zero error code
// is returned as an error.
func OpenImage(path string) (*Image, error) {
	var errOut int
	src := Ptiff_source_open(path, &errOut)
	if src.Swigcptr() == 0 {
		return nil, fmt.Errorf("ptiff_source_open: error code %d", errOut)
	}

	cam, _ := OpenPathCamera(path) // best-effort; a missing camera is not fatal

	desc := NewPtiff_image_descriptor()
	if rc := Ptiff_source_descriptor(src, desc); rc != 0 {
		Ptiff_source_close(src)
		return nil, fmt.Errorf("ptiff_source_descriptor: error code %d", rc)
	}

	return &Image{path: path, src: src, camera: cam, desc: desc}, nil
}

// CreateOptions carries optional settings for CreateImage (tiling and camera).
type CreateOptions struct {
	TileWidth    uint
	TileHeight   uint
	ChannelCount uint
	Camera       *Camera // when set, the calibration is persisted alongside
}

// CreateImage creates a new image at path with the given size and pixel type.
// A zero-value CreateOptions produces an untiled, single-channel image. On
// C-ABI failure a non-zero error code is returned as an error.
func CreateImage(path string, width, height uint, pixelType PixelType, opts CreateOptions) (*Image, error) {
	channels := opts.ChannelCount
	if channels == 0 {
		channels = 1
	}
	desc := NewPtiff_image_descriptor()
	desc.SetWidth(width)
	desc.SetHeight(height)
	desc.SetPixel_type(int(pixelType))
	desc.SetChannel_count(channels)
	if opts.TileWidth > 0 && opts.TileHeight > 0 {
		tiles := NewPtiff_tile_info()
		tiles.SetTile_width(opts.TileWidth)
		tiles.SetTile_height(opts.TileHeight)
		desc.SetHas_tile_info(1)
		desc.SetTile_info(tiles)
	}
	desc.SetHas_compression(0)
	desc.SetCompression(0)

	var sink Struct_SS_ptiff_sink
	if opts.Camera != nil {
		cam := opts.Camera.toMetadata()
		sink = Ptiff_sink_create_camera(path, desc, cam)
	} else {
		sink = Ptiff_sink_create(path, desc)
	}
	if sink.Swigcptr() == 0 {
		return nil, fmt.Errorf("ptiff_sink_create failed for %q", path)
	}

	var cam Camera
	if opts.Camera != nil {
		cam = *opts.Camera
	}
	return &Image{path: path, sink: sink, camera: cam, desc: desc}, nil
}

// Path returns the file path this Image points at.
func (img *Image) Path() string { return img.path }

// Width returns the image width in pixels.
func (img *Image) Width() uint { return img.desc.GetWidth() }

// Height returns the image height in pixels.
func (img *Image) Height() uint { return img.desc.GetHeight() }

// ChannelCount returns the number of channels.
func (img *Image) ChannelCount() uint { return img.desc.GetChannel_count() }

// PixelType returns the pixel type (a PTIFF_PIXEL_* value).
func (img *Image) PixelType() PixelType { return PixelType(img.desc.GetPixel_type()) }

// Camera returns the structured camera calibration. For an opened image it is
// read from the file's `ptiff.camera.*` extension fields; for a created image
// it is whatever CreateOptions.Camera carried (possibly zero-valued).
func (img *Image) Camera() Camera { return img.camera }

// TileColumns returns the number of tile columns (0 when untiled).
func (img *Image) TileColumns() uint {
	if img.src != nil {
		return Ptiff_source_tile_columns(img.src)
	}
	if img.sink != nil {
		return Ptiff_sink_tile_columns(img.sink)
	}
	return 0
}

// TileRows returns the number of tile rows (0 when untiled).
func (img *Image) TileRows() uint {
	if img.src != nil {
		return Ptiff_source_tile_rows(img.src)
	}
	if img.sink != nil {
		return Ptiff_sink_tile_rows(img.sink)
	}
	return 0
}

// TileByteSize returns the byte-payload size of a single tile (0 when untiled).
func (img *Image) TileByteSize() int64 {
	if img.src != nil {
		return Ptiff_source_tile_byte_size(img.src)
	}
	if img.sink != nil {
		return Ptiff_sink_tile_byte_size(img.sink)
	}
	return 0
}

// ReadTile reads the tile at grid position (column, row) and returns its raw
// bytes. It requires the image to be open for reading (OpenImage).
func (img *Image) ReadTile(column, row uint) (Tile, error) {
	if img.src == nil {
		return Tile{}, fmt.Errorf("image not open for reading: %q", img.path)
	}
	n := img.TileByteSize()
	buf := make([]byte, int(n))
	var bytesRead int64
	if rc := Ptiff_source_read_tile(img.src, column, row, buf, &bytesRead); rc != 0 {
		return Tile{}, fmt.Errorf("ptiff_source_read_tile(%d,%d): error code %d", column, row, rc)
	}
	return NewTile(column, row, buf, bytesRead), nil
}

// WriteTile writes data to the tile at grid position (column, row). It requires
// the image to be open for writing (CreateImage).
func (img *Image) WriteTile(column, row uint, data []byte) error {
	if img.sink == nil {
		return fmt.Errorf("image not open for writing: %q", img.path)
	}
	if rc := Ptiff_sink_write_tile(img.sink, column, row, data); rc != 0 {
		return fmt.Errorf("ptiff_sink_write_tile(%d,%d): error code %d", column, row, rc)
	}
	return nil
}

// Close releases the underlying source/sink handles. It is safe to call
// multiple times; closed handles are released and zeroed.
func (img *Image) Close() {
	if img.src != nil {
		Ptiff_source_close(img.src)
		img.src = nil
	}
	if img.sink != nil {
		Ptiff_sink_close(img.sink)
		img.sink = nil
	}
}
