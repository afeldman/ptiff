function assert_ptiff(cond, msg)
%ASSERT_PTIFF  Minimal assertion helper for the ptiff Octave test suite.
%
%   assert_ptiff(COND, MSG) aborts with an error() carrying MSG unless COND
%   is true. Keeps the .m tests self-contained (no xUnit dependency), mirroring
%   the minitest/pytest asserts in the Ruby/Python ports.
if nargin < 2, msg = 'assertion failed'; end
if ~islogical(cond) && ~(isnumeric(cond) && numel(cond) == 1)
  error('assert_ptiff:nonlogical', 'assert_ptiff condition must be a scalar, got %s', class(cond));
end
if ~cond
  error('assert_ptiff:failed', '%s', msg);
end
end
