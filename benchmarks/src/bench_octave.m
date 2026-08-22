function bench_octave(out_path, use_real, use_nac)
%BENCH_OCTAVE  Benchmark read/write throughput of the SWIG Octave binding.
%
%   Mirrors benchmarks/src/bench_python.py with the same metric set and
%   median-of-repeats timing (tic/toc == perf_counter), over the raw
%   SWIG-generated Octave API (see bindings/octave/test/test_roundtrip.m).
%
%   Metrics:
%     write_all_tiles_ms  - create 128x128 UInt8 tiled TIFF, write all tiles
%     read_uint8_128_ms   - open benchmarks/fixtures/uint8_128.tif, read all tiles
%     read_uint8_512_ms   - open benchmarks/fixtures/uint8_512.tif, read all tiles
%     read_f32_512_ms     - open benchmarks/fixtures/f32_512.tif, read all tiles
%
%   Each metric repeats the tile body BENCH_ITERS times (default 50) inside one
%   timed sample; repeats come from BENCH_REPEATS (default 20). Results are
%   written as JSON to OUT_PATH.
%
%   Usage:
%     BENCH_REPEATS=20 BENCH_ITERS=50 octave --quiet --no-gui \
%       --eval "bench_octave('<abs path to benchmark-results>/octave.json');"
%     Pass USE_REAL=true to also read fixtures/real_lola_512.tif (a real NASA
%     LOLA elevation crop copied in by run_benchmarks.sh --real).
%     Pass USE_NAC=true to also read fixtures/nac_dtm.tif (a real NASA LRO-NAC
%     DTM, 2693x14236, 616 tiles -- copied in by run_benchmarks.sh --nac).

  if nargin < 2, use_real = false; end
  if nargin < 3, use_nac = false; end

  % ---- Load module once (one-shot loader; guard) ----
  if exist('ptiff', 'var') == 0
    ptiff();
  end

  repeats = str2double(getenv('BENCH_REPEATS'));
  if isnan(repeats) || repeats <= 0, repeats = 20; end
  iters = str2double(getenv('BENCH_ITERS'));
  if isnan(iters) || iters <= 0, iters = 50; end
  nac_iters = str2double(getenv('BENCH_NAC_ITERS'));
  if isnan(nac_iters) || nac_iters < 1, nac_iters = 2; end

  here = fileparts(mfilename('fullpath'));
  fixtures_dir = fullfile(here, '..', 'fixtures');

  t0 = tic; v = ptiff_runtime_version(); clear t0; % warm up backend
  global PTIFF_PIXEL_UINT8;

  % ---- write_all_tiles ----
  tmp = fullfile(tempdir(), 'bench_octave_write.tif');
  if exist(tmp, 'file'), delete(tmp); end
  d = new_ptiff_image_descriptor();
  d.width = 128; d.height = 128;
  d.pixel_type = PTIFF_PIXEL_UINT8;
  d.channel_count = 1;
  d.has_tile_info = 1;
  d.tile_info.tile_width = 64;
  d.tile_info.tile_height = 64;
  d.has_compression = 0;
  sink = ptiff_sink_create(tmp, d);
  bsc = ptiff_sink_tile_columns(sink);
  bsr = ptiff_sink_tile_rows(sink);
  bsb = ptiff_sink_tile_byte_size(sink);
  write_fn = @() (ptiff_sink_write_tile(sink, 0, 0, ...
      uint8(zeros(1, bsb))) == 0);
  write_fn(); % warm-up check (write once)
  % reset file for the timed run
  if exist(tmp, 'file'), delete(tmp); end
  sink2 = ptiff_sink_create(tmp, d);
  wcols = ptiff_sink_tile_columns(sink2);
  wrows = ptiff_sink_tile_rows(sink2);
  wbs  = ptiff_sink_tile_byte_size(sink2);
  pattern = uint8(7 * ones(1, wbs));
  write_run = @() bench_write_loop(sink2, wcols, wrows, pattern, iters);
  write_run(); % warm-up

  write_times = zeros(repeats, 1);
  for i = 1:repeats
    ste = tic;
    write_run();
    write_times(i) = toc(ste) * 1000.0;
  end
  ptiff_sink_close(sink2);
  delete_ptiff_image_descriptor(d);

  % ---- read_all_tiles over fixtures ----
  read_metrics = struct();
  for spec = {'uint8_128.tif', 'uint8_512.tif', 'f32_512.tif'}
    fname = spec{1};
    path = fullfile(fixtures_dir, fname);
    [src, ~] = ptiff_source_open(path);
    rcols = ptiff_source_tile_columns(src);
    rrows = ptiff_source_tile_rows(src);
    rbs   = ptiff_source_tile_byte_size(src);
    read_run = @() bench_read_loop(src, rcols, rrows, rbs, iters);
    read_run(); % warm-up
    rt = zeros(repeats, 1);
    for i = 1:repeats
      ste = tic;
      read_run();
      rt(i) = toc(ste) * 1000.0;
    end
    ptiff_source_close(src);
    switch fname
      case 'uint8_128.tif', read_metrics.read_uint8_128_ms = stat_of(rt);
      case 'uint8_512.tif', read_metrics.read_uint8_512_ms = stat_of(rt);
      case 'f32_512.tif',   read_metrics.read_f32_512_ms   = stat_of(rt);
    end
  end

  % --real: read a genuine NASA LOLA elevation crop (benchmarks/fixtures/
  % real_lola_512.tif), added by run_benchmarks.sh --real, in addition to the
  % synthetic gradient fixtures.
  if use_real
    real_tif = fullfile(fixtures_dir, 'real_lola_512.tif');
    if exist(real_tif, 'file')
      [rsrc, ~] = ptiff_source_open(real_tif);
      rrcols = ptiff_source_tile_columns(rsrc);
      rrrows = ptiff_source_tile_rows(rsrc);
      rrbs   = ptiff_source_tile_byte_size(rsrc);
      rread_run = @() bench_read_loop(rsrc, rrcols, rrrows, rrbs, iters);
      rread_run(); % warm-up
      rrt = zeros(repeats, 1);
      for i = 1:repeats
        ste = tic;
        rread_run();
        rrt(i) = toc(ste) * 1000.0;
      end
      ptiff_source_close(rsrc);
      read_metrics.read_real_lola_512_ms = stat_of(rrt);
    else
      fprintf(2, '[octave] --real requested but real_lola_512.tif missing; skipping\n');
    end
  end

  % --nac: read the full real NASA LRO-NAC DTM (616 tiles/pass, ~55 MB).
  % Each inner sample is a full pass over every tile; nac_iters bounds it.
  if use_nac
    nac_tif = fullfile(fixtures_dir, 'nac_dtm.tif');
    if exist(nac_tif, 'file')
      [nsrc, ~] = ptiff_source_open(nac_tif);
      ncols = ptiff_source_tile_columns(nsrc);
      nrows = ptiff_source_tile_rows(nsrc);
      nbs   = ptiff_source_tile_byte_size(nsrc);
      nac_run = @() bench_read_loop(nsrc, ncols, nrows, nbs, nac_iters);
      nac_run(); % warm-up
      nt = zeros(repeats, 1);
      for i = 1:repeats
        ste = tic;
        nac_run();
        nt(i) = toc(ste) * 1000.0;
      end
      ptiff_source_close(nsrc);
      read_metrics.read_nac_ms = stat_of(nt);
    else
      fprintf(2, '[octave] --nac requested but nac_dtm.tif missing; skipping\n');
    end
  end

  doc.metrics.write_all_tiles_ms = stat_of(write_times);
  doc.metrics.read_uint8_128_ms  = read_metrics.read_uint8_128_ms;
  doc.metrics.read_uint8_512_ms  = read_metrics.read_uint8_512_ms;
  doc.metrics.read_f32_512_ms    = read_metrics.read_f32_512_ms;
  if isfield(read_metrics, 'read_real_lola_512_ms')
    doc.metrics.read_real_lola_512_ms = read_metrics.read_real_lola_512_ms;
  end
  if isfield(read_metrics, 'read_nac_ms')
    doc.metrics.read_nac_ms = read_metrics.read_nac_ms;
    doc.nac_ifd.tiles_per_pass = ncols * nrows;
    doc.nac_ifd.nac_iters = nac_iters;
  end
  doc.language = 'octave';
  doc.binding_version = sprintf('%d.%d.%d', v.major, v.minor, v.patch);
  doc.repeats = repeats;
  doc.iterations_per_sample = iters;
  doc.write_image.width = 128; doc.write_image.height = 128;
  doc.write_image.pixel_type = 'uint8'; doc.write_image.tile = 64;

  % fixture sizes
  doc.fixtures.uint8_128 = fixture_info(fullfile(fixtures_dir, 'uint8_128.tif'));
  doc.fixtures.uint8_512 = fixture_info(fullfile(fixtures_dir, 'uint8_512.tif'));
  doc.fixtures.f32_512   = fixture_info(fullfile(fixtures_dir, 'f32_512.tif'));
  if exist(fullfile(fixtures_dir, 'real_lola_512.tif'), 'file')
    doc.fixtures.real_lola_512 = fixture_info(fullfile(fixtures_dir, 'real_lola_512.tif'));
  end
  if exist(fullfile(fixtures_dir, 'nac_dtm.tif'), 'file')
    doc.fixtures.nac_dtm = fixture_info(fullfile(fixtures_dir, 'nac_dtm.tif'));
  end

  out_path = fullfile(out_path);
  ensure_parent(out_path);
  fid = fopen(out_path, 'w');
  fprintf(fid, '%s\n', jsonencode(doc));
  fclose(fid);
  fprintf('{"language":"octave","metrics":%s}\n', jsonencode(doc.metrics));

  if exist(tmp, 'file'), delete(tmp); end
end

function st = stat_of(ms_vec)
  s = sort(ms_vec);
  st.repeats = numel(ms_vec);
  st.median_ms = round(median(s) * 10000) / 10000;
  st.min_ms    = round(min(s) * 10000) / 10000;
  st.max_ms    = round(max(s) * 10000) / 10000;
end

function bench_write_loop(sink, cols, rows, pattern, iters)
  for i = 1:iters
    for c = 0:cols-1
      for r = 0:rows-1
        rc = ptiff_sink_write_tile(sink, c, r, pattern);
        if rc ~= 0, error('write_tile rc=%d', rc); end
      end
    end
  end
end

function bench_read_loop(src, cols, rows, bs, iters)
  for i = 1:iters
    buf = uint8(zeros(1, bs));
    for c = 0:cols-1
      for r = 0:rows-1
        [rc, ~, ~] = ptiff_source_read_tile(src, c, r, buf);
        if rc ~= 0, error('read_tile rc=%d', rc); end
      end
    end
  end
end

function info = fixture_info(path)
  f = dir(path);
  info.file = char(path);
  info.size_bytes = 0;
  info.exists = ~isempty(f);
  if info.exists, info.size_bytes = f.bytes; end
end

function ensure_parent(path)
  [parent, ~, ~] = fileparts(path);
  if ~isempty(parent) && ~exist(parent, 'dir')
    mkdir(parent);
  end
end
