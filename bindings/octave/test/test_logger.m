function test_logger()
%TEST_LOGGER  Logger surface over the raw SWIG-generated Octave binding.
%
%   The C ABI's promise is the free functions ptiff_logger_set_level /
%   ptiff_logger_level, and that emitting a log line below/at the current
%   threshold never crashes. Mirrors the Python/Ruby logger tests.

c = ptiff_constants();
orig = ptiff_logger_level();

ptiff_logger_set_level(c.PTIFF_LOG_ERROR);
assert_ptiff(ptiff_logger_level() == c.PTIFF_LOG_ERROR, 'level = error');

% Emitting at a level below the threshold must be safe (and filtered).
ptiff_logger_log(c.PTIFF_LOG_TRACE, 'this trace line is filtered out');
ptiff_logger_log(c.PTIFF_LOG_INFO, 'this info line is filtered out');
ptiff_logger_log(c.PTIFF_LOG_ERROR, 'this error line is emitted');

ptiff_logger_set_level(c.PTIFF_LOG_DEBUG);
assert_ptiff(ptiff_logger_level() == c.PTIFF_LOG_DEBUG, 'level = debug');
ptiff_logger_log(c.PTIFF_LOG_DEBUG, 'backend debugging');
ptiff_logger_log(c.PTIFF_LOG_WARN, 'a warning');

ptiff_logger_set_level(orig);

fprintf('test_logger: OK\n');
end
