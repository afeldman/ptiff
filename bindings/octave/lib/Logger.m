classdef Logger
    %Logger  Thin, stateless view over the library's global logger (Octave).
    %
    %   Mirrors Python's ptiff.Logger and Ruby's PTiff::Logger: a small
    %   object whose methods call the SWIG low-level ptiff_logger_* functions.
    %   The underlying C logger is a single process-wide instance, so any
    %   number of Logger objects read and mutate it.
    %
    %   Level numeric values are the PTIFF_LOG_* C ABI enum values, exposed as
    %   read-only properties (TRACE..OFF). Convenience methods trace/debug/...
    %   emit at those levels.
    %
    %   Example
    %   -------
    %       l = ptiff.Logger();
    %       l.set_level(l.ERROR);
    %       l.error("boom");
    %
    %   Requires the SWIG module (call ptiff() once first).

    properties (Constant = true)
        % PTIFF_LOG_* level values (match the C ABI enum).
        TRACE    = 0
        DEBUG    = 1
        INFO     = 2
        WARN     = 3
        ERROR    = 4
        CRITICAL = 5
        OFF      = 6
    end

    methods
        function level = get_level(obj)
            %GET_LEVEL  Current minimum level that gets emitted.
            level = ptiff_logger_level();
        end

        function l = level(obj)
            %LEVEL  Alias for get_level.
            l = ptiff_logger_level();
        end

        function set_level(obj, value)
            %SET_LEVEL  Set the minimum level that gets emitted.
            ptiff_logger_set_level(int32(value));
        end

        function log(obj, value, msg)
            %LOG  Emit msg at level value (filtered below the threshold).
            ptiff_logger_log(int32(value), char(msg));
        end

        function trace(obj, msg),    obj.log(obj.TRACE, msg);  end
        function debug(obj, msg),    obj.log(obj.DEBUG, msg);  end
        function info(obj, msg),     obj.log(obj.INFO, msg);   end
        function warning(obj, msg),  obj.log(obj.WARN, msg);   end
        function error(obj, msg),    obj.log(obj.ERROR, msg);  end
        function critical(obj, msg), obj.log(obj.CRITICAL, msg); end
    end
end
