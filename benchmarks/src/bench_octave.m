function bench_octave(out_path, use_real, use_nac)
%BENCH_OCTAVE  Benchmark read/write throughput of the Octave MEX binding.
%
%   Mirrors benchmarks/src/bench_python.py with the same metric set and
%   median-of-repeats timing (tic/toc == perf_counter), over the hand-written
%   MEX adapter (bindings/octave/mex/), the sole idiomatic Octave binding.
%
%   Metrics:
%     write_all_tiles_ms  - create 128x128 UInt8 tiled TIFF, write all tiles
%     read_uint8_128_ms   - open benchmarks/fixtures/uint8_128.tif, read all tiles
%     read_uint8_512_ms   - open benchmarks/fixtures/uint8_512.tif, read all tiles
%     read_f32_512_ms     - open benchmarks/fixtures/f32_512.tif, read all tiles
%
%   Each metric repeats the tile body BENCH_ITERS times (default 50) inside one
%   timed sample; repeats come from BENCH_REPEATS (default 20). Results are
%   written as JSON (matching bench_python.py / summary.py) to out_path.
%
%   See run_benchmarks.sh: it places bindings/octave/mex + benchmarks/src on
%   the Octave path, so ptiff_* functions and bench_octave resolve directly.
%     Pass USE_REAL=true to also read fixtures/real_lola_512.tif (a real NASA
%   LOLA elevation crop copied in by run_benchmarks.sh --real).
%     Pass USE_NAC=true to also read fixtures/nac_dtm.tif (a real NASA LRO-NAC
%   DTM, 2693x14236, 616 tiles -- copied in by run_benchmarks.sh --nac).

  if nargin < 2, use_real = false; end
  if nargin < 3, use_nac = false; end

  % ---- Constants from the MEX adapter ----
  c = ptiff_constants_mex();
  PIX_UINT8 = c.PTIFF_PIXEL_UINT8;

  repeats = str2double(getenv('BENCH_REPEATS'));
  if isnan(repeats) || repeats <= 0, repeats = 20; end
  iters = str2double(getenv('BENCH_ITERS'));
  if isnan(iters) || iters <= 0, iters = 50; end
  nac_iters = str2double(getenv('BENCH_NAC_ITERS'));
  if isnan(nac_iters) || nac_iters < 1, nac_iters = 2; end

  here = fileparts(mfilename('fullpath'));
  fixtures_dir = fullfile(here, '..', 'fixtures');

  t0 = tic; v = ptiff_version(); clear t0; % warm up backend

  % ---- write_all_tiles ----
  tmp = fullfile(tempdir(), 'bench_octave_write.tif');
  if exist(tmp, 'file'), delete(tmp); end
  sink = ptiff_create(tmp, 128, 128, PIX_UINT8, 1, 64, 64);
  si = ptiff_sink_info(sink);
  bsc = si.tile_columns;
  bsr = si.tile_rows;
  bsb = si.tile_byte_size;
  ptiff_write_tile(sink, 0, 0, uint8(zeros(1, bsb))); % warm-up write once
  % reset file for the timed run
  if exist(tmp, 'file'), delete(tmp); end
  sink2 = ptiff_create(tmp, 128, 128, PIX_UINT8, 1, 64, 64);
  si2 = ptiff_sink_info(sink2);
  wcols = si2.tile_columns;
  wrows = si2.tile_rows;
  wbs  = si2.tile_byte_size;
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

  % ---- read_all_tiles over fixtures ----
  read_metrics = struct();
  for spec = {'uint8_128.tif', 'uint8_512.tif', 'f32_512.tif'}
    fname = spec{1};
    path = fullfile(fixtures_dir, fname);
    src = ptiff_open(path);
    si = ptiff_source_info(src);
    rcols = si.tile_columns;
    rrows = si.tile_rows;
    rbs   = si.tile_byte_size;
    read_run = @() bench_read_loop(src, rcols, rrows, iters);
    read_run(); % warm-up
    rt = zeros(repeats, 1);
    for i = 1:repeats
      ste = tic;
      read_run();
      rt(i) = toc(ste) * 1000.0;
    end
    ptiff_close(src);
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
      rsrc = ptiff_open(real_tif);
      rsi = ptiff_source_info(rsrc);
      rrcols = rsi.tile_columns;
      rrrows = rsi.tile_rows;
      rread_run = @() bench_read_loop(rsrc, rrcols, rrrows, iters);
      rread_run(); % warm-up
      rrt = zeros(repeats, 1);
      for i = 1:repeats
        ste = tic;
        rread_run();
        rrt(i) = toc(ste) * 1000.0;
      end
      ptiff_close(rsrc);
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
      nsrc = ptiff_open(nac_tif);
      nsi = ptiff_source_info(nsrc);
      ncols = nsi.tile_columns;
      nrows = nsi.tile_rows;
      nac_run = @() bench_read_loop(nsrc, ncols, nrows, nac_iters);
      nac_run(); % warm-up
      nt = zeros(repeats, 1);
      for i = 1:repeats
        ste = tic;
        nac_run();
        nt(i) = toc(ste) * 1000.0;
      end
      ptiff_close(nsrc);
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
  doc.binding_version = sprintf('%d.%d.%d', v.compile_major, v.compile_minor, v.compile_patch);
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
        ptiff_write_tile(sink, c, r, pattern);
      end
    end
  end
end

function bench_read_loop(src, cols, rows, iters)
  for i = 1:iters
    for c = 0:cols-1
      for r = 0:rows-1
        data = ptiff_read_tile(src, c, r);
        if isempty(data), error('read_tile empty'); end
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
