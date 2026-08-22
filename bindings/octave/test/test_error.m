function test_error()
%TEST_ERROR  Error-code / pixel-type / compression-kind numeric ordering pins.
%
%   There is no Error/PixelType/CompressionKind enum wrapper at the raw SWIG
%   level (that is hand-written-binding sugar): what the C ABI actually
%   promises is the numeric ordering of ptiff_error_code / ptiff_pixel_type /
%   ptiff_compression_kind, which must match the C++ enums -- this pins that
%   ordering so a future re-numbering is caught here. Mirrors the ordering
%   tests in bindings/{python,ruby}/test.

c = ptiff_constants();

% ---- ptiff_error_code ----
assert_ptiff(c.PTIFF_ERROR_NOT_IMPLEMENTED == 0, 'NOT_IMPLEMENTED');
assert_ptiff(c.PTIFF_ERROR_INVALID_ARGUMENT == 1, 'INVALID_ARGUMENT');
assert_ptiff(c.PTIFF_ERROR_OUT_OF_RANGE == 2, 'OUT_OF_RANGE');
assert_ptiff(c.PTIFF_ERROR_NOT_FOUND == 3, 'NOT_FOUND');
assert_ptiff(c.PTIFF_ERROR_UNKNOWN == 4, 'UNKNOWN');

% ---- ptiff_pixel_type ----
assert_ptiff(c.PTIFF_PIXEL_UINT8 == 0, 'UINT8');
assert_ptiff(c.PTIFF_PIXEL_UINT16 == 1, 'UINT16');
assert_ptiff(c.PTIFF_PIXEL_UINT32 == 2, 'UINT32');
assert_ptiff(c.PTIFF_PIXEL_FLOAT32 == 3, 'FLOAT32');
assert_ptiff(c.PTIFF_PIXEL_FLOAT64 == 4, 'FLOAT64');

% ---- ptiff_compression_kind ----
assert_ptiff(c.PTIFF_COMPRESSION_NONE == 0, 'NONE');
assert_ptiff(c.PTIFF_COMPRESSION_LZW == 1, 'LZW');
assert_ptiff(c.PTIFF_COMPRESSION_DEFLATE == 2, 'DEFLATE');
assert_ptiff(c.PTIFF_COMPRESSION_JPEG == 3, 'JPEG');

fprintf('test_error: OK\n');
end
