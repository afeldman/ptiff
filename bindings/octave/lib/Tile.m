classdef Tile
    %Tile  One image tile: its grid position and the raw byte payload (Octave).
    %
    %   Mirrors Python's ptiff.Tile / Ruby's PTiff::Tile: a small value object.
    %
    %   Properties
    %   ----------
    %   column    : grid column
    %   row       : grid row
    %   data      : uint8 row vector with the raw tile bytes
    %   byte_size : payload length as reported by the C ABI

    properties
        column
        row
        data
        byte_size
    end

    methods
        function obj = Tile(column, row, data, byte_size)
            %TILE  Tile(column, row, data) or Tile(column, row, data, byte_size)
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
