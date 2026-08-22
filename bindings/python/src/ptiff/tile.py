class Tile:
    """One image tile: its grid position and the raw byte payload."""

    def __init__(self, column, row, data, byte_size=0):
        self.column = column
        self.row = row
        self.data = data
        self.byte_size = byte_size if byte_size else len(data)

    def __repr__(self):
        return f"<ptiff.Tile col={self.column} row={self.row} bytes={self.byte_size}>"
