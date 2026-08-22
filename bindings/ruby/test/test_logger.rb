# frozen_string_literal: true

# Logger surface over the raw SWIG-generated Ruby binding.
#
# Ports bindings/ruby/test/test_ptiff.rb's test_logger_roundtrip. There is no
# PTiff.logger convenience object here (that is hand-written-binding sugar) --
# the C ABI's actual promise is the free functions ptiff_logger_set_level/
# ptiff_logger_level, and that emitting a log line below/at the current
# threshold never crashes.
require 'minitest/autorun'
require 'ptiff'

class TestLogger < Minitest::Test
  def test_logger_roundtrip
    original = Ptiff::ptiff_logger_level
    begin
      Ptiff::ptiff_logger_set_level(Ptiff::PTIFF_LOG_ERROR)
      assert_equal Ptiff::PTIFF_LOG_ERROR, Ptiff::ptiff_logger_level

      # Emitting a log at a level below the current threshold must be safe.
      Ptiff::ptiff_logger_log(Ptiff::PTIFF_LOG_TRACE, 'this trace line is filtered out')
      Ptiff::ptiff_logger_log(Ptiff::PTIFF_LOG_INFO, 'this info line is filtered out')
      Ptiff::ptiff_logger_log(Ptiff::PTIFF_LOG_ERROR, 'this error line is emitted')

      Ptiff::ptiff_logger_set_level(Ptiff::PTIFF_LOG_DEBUG)
      assert_equal Ptiff::PTIFF_LOG_DEBUG, Ptiff::ptiff_logger_level
      Ptiff::ptiff_logger_log(Ptiff::PTIFF_LOG_DEBUG, 'backend debugging')
      Ptiff::ptiff_logger_log(Ptiff::PTIFF_LOG_WARN, 'a warning')
    ensure
      Ptiff::ptiff_logger_set_level(original)
    end
  end
end
