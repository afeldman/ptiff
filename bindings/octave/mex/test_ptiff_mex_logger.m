function test_ptiff_mex_logger()
%TEST_PTIFF_MEX_LOGGER  Logger level set/read + log call over the MEX adapter.
%
%   Mirrors the SWIG test_logger.m coverage (set level, read it back, emit a
%   message) but over the ptiff-octave adapter's logger commands. Restores the
%   original level so it is independent of any other test.

orig = ptiff_logger_level();
try
  ptiff_logger_set_level(4);          % PTIFF_LOG_ERROR
  assert(ptiff_logger_level() == 4, 'set level');
  ptiff_logger_log(4, 'test_ptiff_mex_logger message');
  ptiff_logger_set_level(0);          % PTIFF_LOG_TRACE
  assert(ptiff_logger_level() == 0, 'set trace');
  fprintf('test_ptiff_mex_logger: OK\n');
catch
  ptiff_logger_set_level(orig);
  rethrow(lasterror);
end
ptiff_logger_set_level(orig);
end
