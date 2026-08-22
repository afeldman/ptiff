# frozen_string_literal: true

# Backend registry surface over the raw SWIG-generated Ruby binding.
#
# Ports bindings/ruby/test/test_ptiff.rb's test_backends. The C ABI returns a
# single comma-space-joined, caller-owned C string (documented as released
# via ptiff_free_string). SWIG's default char* return typemap for Ruby
# already copies the C buffer into a Ruby String, so what test code gets
# back is Ruby-owned, not the original pointer: calling ptiff_free_string on
# it aborts the process (confirmed: same bug as the Python port). The
# original buffer is leaked by the generated binding, same as the Go and
# Python ports -- not fixed here, since fixing it needs a %newobject/custom
# typemap in ptiff.i, not a test change.
require 'minitest/autorun'
require 'ptiff'

class TestBackend < Minitest::Test
  def test_backend_names
    joined = Ptiff::ptiff_backend_names
    # May legitimately be empty when linking statically without
    # whole-archive (backend registration TUs get dead-stripped).
    return if joined.nil? || joined.empty?

    names = joined.split(', ')
    refute names.any?(&:empty?), "empty name in #{joined.inspect}"
  end
end
