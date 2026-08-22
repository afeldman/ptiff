classdef Camera
    %Camera  Structured camera calibration for one frame (Octave).
    %
    %   Wraps the C-ABI ptiff_open_path_camera result (K, [R|t], P and the
    %   ISO-8601 timestamp) as a small value object. Matrices are returned as
    %   flat numeric row vectors (row-major); the projection P = K * [R|t] is
    %   computed in the C++ library.
    %
    %   A Camera can be built two ways:
    %     * `Camera.from_path(path)` -- read the calibration embedded in a
    %       PTIFF.
    %     * `Camera.new(...)` -- describe a camera from known parameters, then
    %       persist it when creating an image via Image.create(..., camera).
    %
    %   Requires the SWIG module (call ptiff() once first).

    properties
        % Raw map of fields as returned by ptiff_open_path_camera.
        data
    end

    methods (Static = true)
        function cam = from_path(path)
            %FROM_PATH  Open path and return its structured camera calibration.
            [rc, d] = ptiff_open_path_camera(char(path));
            if rc ~= 0
                error('ptiff:OpenPathCamera', ...
                    'ptiff_open_path_camera(%s) failed (rc=%d)', path, rc);
            end
            cam = Camera(d);
        end
    end

    methods
        function obj = Camera(data, varargin)
            %CAMERA  Camera() | Camera(struct) | Camera('key', value, ...)
            %   * Camera()            -- defaults (pinhole, intrinsics+extrinsics).
            %   * Camera(data)        -- data is a struct from
            %                           ptiff_open_path_camera (via from_path).
            %   * Camera('key', val)  -- describe a camera from known parameters
            %                           (focal_length_x, principal_x, ...).
            p = inputParser;
            addParameter(p, 'focal_length_x', 0.0);
            addParameter(p, 'focal_length_y', 0.0);
            addParameter(p, 'principal_x', 0.0);
            addParameter(p, 'principal_y', 0.0);
            addParameter(p, 'rotation_w', 1.0);
            addParameter(p, 'rotation_x', 0.0);
            addParameter(p, 'rotation_y', 0.0);
            addParameter(p, 'rotation_z', 0.0);
            addParameter(p, 'position_x', 0.0);
            addParameter(p, 'position_y', 0.0);
            addParameter(p, 'position_z', 0.0);
            addParameter(p, 'model', 'pinhole');
            addParameter(p, 'timestamp', '');
            args = {};
            if nargin >= 1 && ~isempty(data)
                if isstruct(data)
                    obj.data = data;
                    return;
                else
                    % data is the first key of a key/value builder list.
                    args = [{data}, varargin];
                    nargs = numel(args);
                    if mod(nargs, 2) ~= 0
                        % odd trailing scalar: treat as a single scalar? no --
                        % expect key/value pairs only.
                        error('ptiff:Camera', 'Camera(...) expects key/value pairs or a struct');
                    end
                end
            end
            parse(p, args{:});
            s = struct();
            s.has_intrinsics = 1;
            s.has_extrinsics = 1;
            s.focal_length_x = p.Results.focal_length_x;
            s.focal_length_y = p.Results.focal_length_y;
            s.principal_x = p.Results.principal_x;
            s.principal_y = p.Results.principal_y;
            s.rotation_w = p.Results.rotation_w;
            s.rotation_x = p.Results.rotation_x;
            s.rotation_y = p.Results.rotation_y;
            s.rotation_z = p.Results.rotation_z;
            s.position_x = p.Results.position_x;
            s.position_y = p.Results.position_y;
            s.position_z = p.Results.position_z;
            s.model = p.Results.model;
            s.timestamp = p.Results.timestamp;
            % Fill matrices so to_struct has defined values.
            s.intrinsics = zeros(1, 9);
            s.extrinsics = zeros(1, 12);
            s.projection = zeros(1, 12);
            obj.data = s;
        end

        function v = has_intrinsics(obj), v = obj.data.has_intrinsics ~= 0; end
        function v = has_extrinsics(obj), v = obj.data.has_extrinsics ~= 0; end
        function v = model(obj)
            v = 'pinhole';
            if isfield(obj.data, 'model') && ~isempty(obj.data.model)
                v = obj.data.model;
            end
        end
        function v = focal_length_x(obj), v = obj.data.focal_length_x; end
        function v = focal_length_y(obj), v = obj.data.focal_length_y; end
        function v = principal_x(obj),    v = obj.data.principal_x;    end
        function v = principal_y(obj),    v = obj.data.principal_y;    end
        function v = rotation_w(obj),     v = obj.data.rotation_w;     end
        function v = rotation_x(obj),     v = obj.data.rotation_x;     end
        function v = rotation_y(obj),     v = obj.data.rotation_y;     end
        function v = rotation_z(obj),     v = obj.data.rotation_z;     end
        function v = position_x(obj),     v = obj.data.position_x;     end
        function v = position_y(obj),     v = obj.data.position_y;     end
        function v = position_z(obj),     v = obj.data.position_z;     end
        function v = timestamp(obj),      v = obj.data.timestamp;      end

        function m = intrinsics_matrix(obj), m = obj.data.intrinsics; end
        function m = extrinsics_matrix(obj), m = obj.data.extrinsics; end
        function m = projection_matrix(obj), m = obj.data.projection; end

        function c = to_struct(obj)
            %TO_STRUCT  Build a SWIG ptiff_camera struct ready to persist with
            %Image.create(..., camera).
            c = new_ptiff_camera();
            c.has_intrinsics = int32(obj.has_intrinsics());
            c.focal_length_x = obj.focal_length_x();
            c.focal_length_y = obj.focal_length_y();
            c.principal_x = obj.principal_x();
            c.principal_y = obj.principal_y();
            c.has_extrinsics = int32(obj.has_extrinsics());
            c.rotation_w = obj.rotation_w();
            c.rotation_x = obj.rotation_x();
            c.rotation_y = obj.rotation_y();
            c.rotation_z = obj.rotation_z();
            c.position_x = obj.position_x();
            c.position_y = obj.position_y();
            c.position_z = obj.position_z();
            c.timestamp = obj.timestamp();
        end
    end
end
