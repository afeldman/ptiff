classdef Metadata
    %Metadata  Read-only metadata of a single image file (Octave).
    %
    %   Built from the file itself without keeping a live image handle open:
    %   the core descriptor (ptiff_open_path) and the flattened PTIFF
    %   extension fields (ptiff_open_path_fields). Fields are exposed as a
    %   struct of key -> value strings.
    %
    %   Requires the SWIG module (call ptiff() once first).

    properties
        path
        width
        height
        channel_count
        pixel_type
        tile_width
        tile_height
        fields
    end

    methods
        function obj = Metadata(path)
            %METADATA  Metadata(path) -- read the file's core + extension fields.
            path = char(path);
            obj.path = path;

            desc = new_ptiff_image_descriptor();
            try
                rc = ptiff_open_path(path, desc);
                if rc ~= 0
                    error('ptiff:Metadata', 'ptiff_open_path(%s) failed (rc=%d)', path, rc);
                end
                obj.width = desc.width;
                obj.height = desc.height;
                obj.channel_count = desc.channel_count;
                obj.pixel_type = desc.pixel_type;
                if desc.has_tile_info ~= 0
                    obj.tile_width = desc.tile_info.tile_width;
                    obj.tile_height = desc.tile_info.tile_height;
                else
                    obj.tile_width = 0;
                    obj.tile_height = 0;
                end
            finally
                delete_ptiff_image_descriptor(desc);
            end

            [frc, m] = ptiff_open_path_fields(path);
            if frc ~= 0
                error('ptiff:Metadata', 'ptiff_open_path_fields(%s) failed (rc=%d)', path, frc);
            end
            obj.fields = struct();
            n = numel(m.key);
            for i = 1:n
                obj.fields.(m.key{i}) = m.value{i};
            end
        end

        function name = pixel_type_name(obj)
            %PIXEL_TYPE_NAME  Human name of the pixel type.
            names = {'UInt8', 'UInt16', 'UInt32', 'Float32', 'Float64'};
            if obj.pixel_type + 1 <= numel(names)
                name = names{obj.pixel_type + 1};
            else
                name = 'Unknown';
            end
        end

        function v = field(obj, key, default)
            %FIELD  Return a single extension field value (or default).
            if nargin < 3
                default = [];
            end
            if isfield(obj.fields, key)
                v = obj.fields.(key);
            else
                v = default;
            end
        end
    end
end
