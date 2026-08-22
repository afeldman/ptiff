# frozen_string_literal: true

module PTiff
  # Logger is a thin, stateless view over the library's global logger.
  #
  # The underlying C logger is a single process-wide instance, so any number of
  # Logger objects read and mutate that one instance. It wraps the SWIG
  # low-level `Ptiff::ptiff_logger_*` functions and re-exports the `PTIFF_LOG_*`
  # level values as friendly class constants.
  class Logger
    TRACE    = Ptiff::PTIFF_LOG_TRACE
    DEBUG    = Ptiff::PTIFF_LOG_DEBUG
    INFO     = Ptiff::PTIFF_LOG_INFO
    WARN     = Ptiff::PTIFF_LOG_WARN
    ERROR    = Ptiff::PTIFF_LOG_ERROR
    CRITICAL = Ptiff::PTIFF_LOG_CRITICAL
    OFF      = Ptiff::PTIFF_LOG_OFF

    # Returns the current minimum level that gets emitted (a Logger::* value).
    def level
      Ptiff::ptiff_logger_level
    end

    # Sets the minimum level that gets emitted (a Logger::* value).
    def level=(value)
      Ptiff::ptiff_logger_set_level(value)
    end

    alias set_level level=

    # Emits message at level; it is filtered out below the current threshold.
    def log(level, message)
      Ptiff::ptiff_logger_log(level, message)
    end

    def trace(message) = log(TRACE, message)
    def debug(message) = log(DEBUG, message)
    def info(message)  = log(INFO, message)
    def warning(message) = log(WARN, message)
    def error(message) = log(ERROR, message)
    def critical(message) = log(CRITICAL, message)
  end
end
