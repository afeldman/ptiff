# frozen_string_literal: true

module PTiff
  # Metadata is read-only metadata of a single image file.
  #
  # It is built from the file itself without keeping a live image handle open:
  # the core descriptor (via `Ptiff::ptiff_open_path`) and the flattened PTIFF
  # extension fields (via `Ptiff::ptiff_open_path_fields`). It mirrors Python's
  # `ptiff.Metadata`.
  PIXEL_TYPE_NAMES = {
    0 => "UInt8", 1 => "UInt16", 2 => "UInt32",
    3 => "Float32", 4 => "Float64"
  }.freeze

  class Metadata
    attr_reader :path

    def initialize(path)
      path = path.to_s
      @path = path

      desc = Ptiff::Ptiff_image_descriptor.new
      rc = Ptiff::ptiff_open_path(path, desc)
      raise IOError, "ptiff_open_path failed for #{path.inspect} (rc=#{rc})" unless rc.zero?
      @desc = desc

      @fields = {}
      rc2, pairs = Ptiff::ptiff_open_path_fields(path)
      raise IOError, "ptiff_open_path_fields failed for #{path.inspect} (rc=#{rc2})" unless rc2.zero?
      Array(pairs).each { |k, v| @fields[k] = v }
    end

    def width = @desc.width
    def height = @desc.height
    def channel_count = @desc.channel_count
    def pixel_type = @desc.pixel_type

    def pixel_type_name
      PIXEL_TYPE_NAMES[@desc.pixel_type] || "Unknown"
    end

    def tile_width
      @desc.has_tile_info != 0 ? @desc.tile_info.tile_width : 0
    end

    def tile_height
      @desc.has_tile_info != 0 ? @desc.tile_info.tile_height : 0
    end

    # The flattened PTIFF extension fields as an ordered String-keyed Hash
    # (e.g. "ptiff.camera.model" => "pinhole").
    def fields = @fields.dup

    # Returns a single extension field value, or default when absent.
    def field(key, default = nil)
      @fields.fetch(key, default)
    end

    def inspect
      "#<PTiff::Metadata path=#{@path.inspect} #{width}x#{height} " \
        "px=#{pixel_type_name} ch=#{channel_count}>"
    end
  end
end
