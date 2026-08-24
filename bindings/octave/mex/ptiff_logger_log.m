function ptiff_logger_log(level, message)
%PTIFF_LOGGER_LOG  Emit a message at the given level (0=trace .. 6=off).
ptiff_octave('logger_log', level, message);
end
