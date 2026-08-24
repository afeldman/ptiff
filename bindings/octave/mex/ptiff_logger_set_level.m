function ptiff_logger_set_level(level)
%PTIFF_LOGGER_SET_LEVEL  Set the global log level (0=trace .. 6=off).
ptiff_octave('logger_level', level);
end
