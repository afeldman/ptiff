package ptiff

// Tile is one image tile: its grid position and the raw byte payload.
//
// It mirrors Python's `ptiff.Tile` value object over the SWIG low-level
// surface, so the idiomatic object layer stays uniform across languages.
type Tile struct {
	Column   uint
	Row      uint
	Data     []byte
	ByteSize int64
}

// NewTile builds a Tile from a grid position and its payload. When byteSize is
// zero it is derived from the length of data.
func NewTile(column, row uint, data []byte, byteSize int64) Tile {
	if byteSize == 0 {
		byteSize = int64(len(data))
	}
	return Tile{Column: column, Row: row, Data: data, ByteSize: byteSize}
}
