# frozen_string_literal: true

module PTiff
  # Image is a PTIFF image: metadata plus tile access.
  #
  # Use the class methods `Image.open(path)` (read a TIFF/BigTIFF) and
  # `Image.create(path, width, height, pixel_type: ...)` (write a new one).
  # Reading exposes the structured camera calibration via `#camera` and the raw
  # tile bytes via `#read_tile(col, row)`.
  class Image
    attr_reader :path

    def initialize(path, handle: nil, sink: nil, camera: nil, desc: nil)
      @path = path
      @handle = handle
      @sink = sink
      @camera = camera
      @desc = desc
      @camera ||= Camera.from_path(path) if handle || File.file?(path)
    end

    # Opens the TIFF/BigTIFF file at path for reading. Raises IOError on failure.
    def self.open(path)
      path = path.to_s
      res = Ptiff::ptiff_source_open(path)
      handle = res.respond_to?(:first) ? res.first : res
      raise IOError, "ptiff_source_open failed for #{path.inspect}" if handle.nil?
      new(path, handle: handle)
    end

    # Creates a new image at path with the given size and pixel type.
    #
    #   pixel_type:  a PTIFF_PIXEL_* value (default UInt8)
    #   tile_width / tile_height: 0 (the default) disables tiling
    #   channel_count:  defaults to 1
    #   camera:  an optional PTiff::Camera to persist alongside the image
    #
    # Raises IOError if the sink cannot be created.
    def self.create(path, width, height, pixel_type: 0, tile_width: 0,
                    tile_height: 0, channel_count: 1, camera: nil)
      path = path.to_s
      desc = Ptiff::Ptiff_image_descriptor.new
      desc.width = Integer(width)
      desc.height = Integer(height)
      desc.pixel_type = Integer(pixel_type)
      desc.channel_count = Integer(channel_count)
      if tile_width.to_i > 0 && tile_height.to_i > 0
        desc.has_tile_info = 1
        desc.tile_info.tile_width = Integer(tile_width)
        desc.tile_info.tile_height = Integer(tile_height)
      end
      desc.has_compression = 0

      if camera
        struct = camera.is_a?(Camera) ? camera.to_struct : camera
        sink = Ptiff::ptiff_sink_create_camera(path, desc, struct)
        raise IOError, "ptiff_sink_create_camera failed for #{path.inspect}" if sink.nil?
      else
        sink = Ptiff::ptiff_sink_create(path, desc)
        raise IOError, "ptiff_sink_create failed for #{path.inspect}" if sink.nil?
      end
      new(path, sink: sink, desc: desc, camera: camera)
    end

    # Core descriptor as read back from the source (or built on create).
    def descriptor
      @desc ||= begin
        d = Ptiff::Ptiff_image_descriptor.new
        Ptiff::ptiff_source_descriptor(@handle, d)
        d
      end
    end

    def width          = descriptor.width
    def height         = descriptor.height
    def channel_count  = descriptor.channel_count
    def pixel_type     = descriptor.pixel_type
    def camera         = @camera

    def tile_columns
      @handle ? Ptiff::ptiff_source_tile_columns(@handle) : (@sink ? Ptiff::ptiff_sink_tile_columns(@sink) : 0)
    end

    def tile_rows
      @handle ? Ptiff::ptiff_source_tile_rows(@handle) : (@sink ? Ptiff::ptiff_sink_tile_rows(@sink) : 0)
    end

    def tile_byte_size
      @handle ? Ptiff::ptiff_source_tile_byte_size(@handle) : (@sink ? Ptiff::ptiff_sink_tile_byte_size(@sink) : 0)
    end

    # Reads the tile at grid position (column, row) and returns a Tile with the
    # raw byte payload. Requires the image to be open for reading.
    def read_tile(column, row)
      raise IOError, "image not open for reading: #{@path.inspect}" if @handle.nil?
      n = tile_byte_size
      buf = "\0".b * n
      _rc, _nread = Ptiff::ptiff_source_read_tile(@handle, Integer(column), Integer(row), buf)
      Tile.new(Integer(column), Integer(row), buf.dup.force_encoding(Encoding::BINARY))
    end

    # Writes data to the tile at grid position (column, row). Requires the image
    # to be open for writing (created via Image.create).
    def write_tile(column, row, data)
      raise IOError, "image not open for writing: #{@path.inspect}" if @sink.nil?
      buf = String(data).b
      rc = Ptiff::ptiff_sink_write_tile(@sink, Integer(column), Integer(row), buf)
      raise IOError, "ptiff_sink_write_tile(#{column},#{row}) failed (rc=#{rc})" unless rc.zero?
      rc
    end

    # Releases the underlying source/sink handles (a no-op when already closed).
    def close
      unless @handle.nil?
        begin
          Ptiff::ptiff_source_close(@handle)
        rescue StandardError
          nil
        end
        @handle = nil
      end
      unless @sink.nil?
        begin
          Ptiff::ptiff_sink_close(@sink)
        rescue StandardError
          nil
        end
        @sink = nil
      end
    end

    def inspect
      "#<PTiff::Image path=#{@path.inspect} #{width}x#{height}>"
    end
  end
end
