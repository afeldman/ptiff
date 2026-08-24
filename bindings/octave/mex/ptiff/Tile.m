classdef Tile
    %Tile  One image tile: grid position + raw byte payload (ptiff-octave).
    %
    %   Mirrors the SWIG binding's Tile value object: a small container with
    %   `column`, `row`, `data` (uint8 row vector) and `byte_size`.

    properties
        column
        row
        data
        byte_size
    end

    methods
        function obj = Tile(column, row, data, byte_size)
            obj.column = column;
            obj.row = row;
            obj.data = data;
            if nargin >= 4
                obj.byte_size = byte_size;
            else
                obj.byte_size = numel(data);
            end
        end
    end
end
