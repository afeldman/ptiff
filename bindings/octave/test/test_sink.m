function test_sink()
%TEST_SINK  Sink (write) edge cases over the raw SWIG-generated binding.
%
%   Ports bindings/ruby/test/test_sink.rb: writes a tile too small / too big
%   for the sink's tile byte size, an out-of-range tile index, a sink to an
%   empty path, and a sink to an untiled descriptor.

c = ptiff_constants();
td = tile_descriptor_ptiff();

% ---- Write tile with a wrong-sized buffer must fail (rc != 0) ----
tmp = tempname(); mkdir(tmp);
try
  path = fullfile(tmp, 'bad.tif');
  sink = ptiff_sink_create(path, td);
  assert_ptiff(~isempty(sink), 'sink_create returned empty');
  bs = ptiff_sink_tile_byte_size(sink);
  assert_ptiff(ptiff_sink_write_tile(sink, 0, 0, uint8(zeros(1, bs - 1))) ~= 0, ...
    'buffer too small must fail');
  assert_ptiff(ptiff_sink_write_tile(sink, 0, 0, uint8(zeros(1, bs + 1))) ~= 0, ...
    'buffer too big must fail');
  ptiff_sink_close(sink);

  % ---- Out-of-range tile index must fail ----
  path = fullfile(tmp, 'oor.tif');
  sink2 = ptiff_sink_create(path, td);
  assert_ptiff(~isempty(sink2), 'sink_create(oor) returned empty');
  bs = ptiff_sink_tile_byte_size(sink2);
  buf = uint8(zeros(1, bs));
  assert_ptiff(ptiff_sink_write_tile(sink2, 99, 0, buf) ~= 0, 'col out of range');
  assert_ptiff(ptiff_sink_write_tile(sink2, 0, 99, buf) ~= 0, 'row out of range');
  ptiff_sink_close(sink2);

  % ---- Sink to an empty path must fail (sink_create returns NULL) ----
  sink3 = ptiff_sink_create('', td);
  assert_ptiff(isempty(sink3), 'sink_create("") expected NULL');
  if ~isempty(sink3), ptiff_sink_close(sink3); end

  % ---- Untiled descriptor: sink_create must fail and leave no file ----
  d = new_ptiff_image_descriptor();
  d.width = 16; d.height = 16;
  d.pixel_type = c.PTIFF_PIXEL_UINT8;
  d.channel_count = 1;
  p = fullfile(tmp, 'untiled.tif');
  sink4 = ptiff_sink_create(p, d);
  assert_ptiff(isempty(sink4), 'sink_create on untiled descriptor expected NULL');
  if ~isempty(sink4), ptiff_sink_close(sink4); end
  assert_ptiff(~exist(p, 'file'), 'untiled sink_create must not leave a file behind');
  delete_ptiff_image_descriptor(d);
catch err
  % best-effort cleanup then re-throw
  if exist('sink', 'var') && ~isempty(sink), try, ptiff_sink_close(sink), catch, end; end
  if exist('sink2', 'var') && ~isempty(sink2), try, ptiff_sink_close(sink2), catch, end; end
  rmdir(tmp, 's');
  rethrow(err);
end

delete_ptiff_image_descriptor(td);
rmdir(tmp, 's');

fprintf('test_sink: OK\n');
end
