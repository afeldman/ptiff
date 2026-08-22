# frozen_string_literal: true

module PTiff
  # Tile is one image tile: its grid position and the raw byte payload.
  #
  # It mirrors Python's `ptiff.Tile` value object over the SWIG low-level
  # surface, so the idiomatic object layer stays uniform across languages.
  class Tile
    attr_reader :column, :row, :data

    # byte_size is the payload length as reported by the C ABI's
    # `ptiff_source_read_tile` (== data.bytesize for our tile reads), kept as
    # an explicit value so callers see exactly what libptiff returned.
    attr_reader :byte_size

    def initialize(column, row, data, byte_size: nil)
      @column = column
      @row = row
      @data = data
      @byte_size = byte_size || data.bytesize
    end

    def inspect
      "#<PTiff::Tile col=#{@column} row=#{@row} bytes=#{@byte_size}>"
    end
  end
end
