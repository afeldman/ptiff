classdef Image
    %Image  A PTIFF image: metadata plus tile access (Octave).
    %
    %   Use `Image.open(path)` (read a TIFF/BigTIFF) and `Image.create(path,
    %   width, height, ...)` (write a new one). Reading exposes the structured
    %   camera calibration via `#camera` and the raw tile bytes via
    %   `#read_tile(col, row)`.
    %
    %   Requires the SWIG module (call ptiff() once first).

    properties
        path
        source_   % raw ptiff_source handle (non-empty when open for reading)
        sink_     % raw ptiff_sink handle (non-empty when open for writing)
        camera_
        desc_
    end

    methods (Static = true)
        function img = open(path)
            %OPEN  Open the TIFF/BigTIFF file at path for reading.
            path = char(path);
            [handle, ~] = ptiff_source_open(path);
            if isempty(handle)
                error('ptiff:ImageOpen', 'ptiff_source_open(%s) failed', path);
            end
            img = Image();
            img.path = path;
            img.source_ = handle;
            try
                cam = Camera.from_path(path);
                img.camera_ = cam;
            catch
                img.camera_ = [];
            end
            img = img.load_descriptor_();
        end

        function img = create(path, width, height, varargin)
            %CREATE  Create a new image at path with the given size.
            %
            %   Name-value options:
            %     pixel_type     : a PTIFF_PIXEL_* value (default UInt8 = 0)
            %     tile_width     : tile width in px  (0 = untiled)
            %     tile_height    : tile height in px (0 = untiled)
            %     channel_count  : channels (default 1)
            %     camera         : an optional Camera to persist alongside
            patht = char(path);
            p = inputParser;
            addParameter(p, 'pixel_type', 0);
            addParameter(p, 'tile_width', 0);
            addParameter(p, 'tile_height', 0);
            addParameter(p, 'channel_count', 1);
            addParameter(p, 'camera', []);
            parse(p, varargin{:});

            desc = new_ptiff_image_descriptor();
            desc.width = int32(width);
            desc.height = int32(height);
            desc.pixel_type = int32(p.Results.pixel_type);
            desc.channel_count = int32(p.Results.channel_count);
            if p.Results.tile_width > 0 && p.Results.tile_height > 0
                desc.has_tile_info = int32(1);
                desc.tile_info.tile_width = int32(p.Results.tile_width);
                desc.tile_info.tile_height = int32(p.Results.tile_height);
            end
            desc.has_compression = int32(0);

            if ~isempty(p.Results.camera)
                c = p.Results.camera;
                camstruct = c.to_struct();
                sink = ptiff_sink_create_camera(patht, desc, camstruct);
                delete_ptiff_camera(camstruct);
            else
                sink = ptiff_sink_create(patht, desc);
            end
            if isempty(sink)
                delete_ptiff_image_descriptor(desc);
                error('ptiff:ImageCreate', 'ptiff_sink_create(%s) failed', patht);
            end

            img = Image();
            img.path = patht;
            img.sink_ = sink;
            img.desc_ = desc;
            if ~isempty(p.Results.camera)
                img.camera_ = p.Results.camera;
            end
        end
    end

    methods
        function width = width(obj)
            obj = obj.ensure_desc_();
            width = obj.desc_.width;
        end

        function height = height(obj)
            obj = obj.ensure_desc_();
            height = obj.desc_.height;
        end

        function ch = channel_count(obj)
            obj = obj.ensure_desc_();
            ch = obj.desc_.channel_count;
        end

        function pt = pixel_type(obj)
            obj = obj.ensure_desc_();
            pt = obj.desc_.pixel_type;
        end

        function c = camera(obj)
            c = obj.camera_;
        end

        function n = tile_columns(obj)
            if ~isempty(obj.source_)
                n = ptiff_source_tile_columns(obj.source_);
            elseif ~isempty(obj.sink_)
                n = ptiff_sink_tile_columns(obj.sink_);
            else
                n = 0;
            end
        end

        function n = tile_rows(obj)
            if ~isempty(obj.source_)
                n = ptiff_source_tile_rows(obj.source_);
            elseif ~isempty(obj.sink_)
                n = ptiff_sink_tile_rows(obj.sink_);
            else
                n = 0;
            end
        end

        function n = tile_byte_size(obj)
            if ~isempty(obj.source_)
                n = ptiff_source_tile_byte_size(obj.source_);
            elseif ~isempty(obj.sink_)
                n = ptiff_sink_tile_byte_size(obj.sink_);
            else
                n = 0;
            end
        end

        function t = read_tile(obj, column, row)
            %READ_TILE  Read the tile at grid position (column, row) -> Tile.
            if isempty(obj.source_)
                error('ptiff:ReadTile', 'image not open for reading: %s', obj.path);
            end
            bs = obj.tile_byte_size();
            [rc, data, ~] = ptiff_source_read_tile(obj.source_, int32(column), int32(row), uint8(zeros(1, bs)));
            if rc ~= 0
                error('ptiff:ReadTile', 'ptiff_source_read_tile(%d,%d) failed (rc=%d)', column, row, rc);
            end
            t = Tile(column, row, data, bs);
        end

        function rc = write_tile(obj, column, row, data)
            %WRITE_TILE  Write data to the tile at grid position (column, row).
            if isempty(obj.sink_)
                error('ptiff:WriteTile', 'image not open for writing: %s', obj.path);
            end
            rc = ptiff_sink_write_tile(obj.sink_, int32(column), int32(row), uint8(data));
            if rc ~= 0
                error('ptiff:WriteTile', 'ptiff_sink_write_tile(%d,%d) failed (rc=%d)', column, row, rc);
            end
        end

        function close(obj)
            %CLOSE  Release the underlying source/sink handles (idempotent).
            if ~isempty(obj.source_)
                try
                    ptiff_source_close(obj.source_);
                catch
                end
                obj.source_ = [];
            end
            if ~isempty(obj.sink_)
                try
                    ptiff_sink_close(obj.sink_);
                catch
                end
                obj.sink_ = [];
                if ~isempty(obj.desc_)
                    delete_ptiff_image_descriptor(obj.desc_);
                    obj.desc_ = [];
                end
            end
        end
    end

    methods (Access = private)
        function obj = ensure_desc_(obj)
            if isempty(obj.desc_) && ~isempty(obj.source_)
                d = new_ptiff_image_descriptor();
                ptiff_source_descriptor(obj.source_, d);
                obj.desc_ = d;
            end
        end

        function obj = load_descriptor_(obj)
            obj = obj.ensure_desc_();
        end
    end
end
