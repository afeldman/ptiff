# frozen_string_literal: true

require "rbconfig"

module PTiff
  module NativeExtension
    module_function

    def candidate_paths(dir, config = RbConfig::CONFIG)
      [config["DLEXT"], config["DLEXT2"], "bundle", "so"]
        .compact
        .uniq
        .map { |ext| File.expand_path("ptiff.#{ext}", dir) }
    end

    def find(dir, config = RbConfig::CONFIG)
      candidate_paths(dir, config).find { |path| File.file?(path) }
    end
  end
end
