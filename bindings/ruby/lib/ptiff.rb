# frozen_string_literal: true

require_relative "ptiff/native_extension"

# PTiff is the idiomatic, "class-structured" Ruby binding for libptiff.
#
# It is split into two layers, mirroring the Python package `src/ptiff/`:
#
# * `Ptiff` -- the raw, SWIG-generated low-level surface (the platform-native
#   compiled extension in `lib/ptiff.{bundle,so}`): all the `ptiff_*` C-ABI
#   functions and the `Ptiff_*`/`PTIFF_*` structs and constants. This is what
#   the compiler produces and what the wrapper classes underneath build on.
# * `PTiff` -- the idiomatic object layer: `Camera`, `Image`, `Tile`,
#   `Logger` and `Metadata` classes that wrap the `Ptiff::` low-level surface
#   so callers do not touch raw handles or structs directly.
#
# The gem is bundler-managed (see Gemfile / ptiff.gemspec): add it to a
# Gemfile with `gem "ptiff", path: "..."` and `bundle install`.

module PTiff
  NATIVE_EXTENSION_PATH = NativeExtension.find(__dir__)

  raise LoadError, "cannot load native extension from #{__dir__}" unless NATIVE_EXTENSION_PATH
end

require PTiff::NATIVE_EXTENSION_PATH
require_relative "ptiff/version"
require_relative "ptiff/tile"
require_relative "ptiff/camera"
require_relative "ptiff/logger"
require_relative "ptiff/metadata"
require_relative "ptiff/image"
