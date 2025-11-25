package rayzor;

/**
 * Runtime tracing/logging utilities for Rayzor
 *
 * These are low-level trace functions that directly call runtime logging.
 * Use the global trace() function for automatic type detection.
 */
@:native("rayzor::Trace")
extern class Trace {
    /**
     * Trace an integer value
     */
    @:native("haxe_trace_int")
    public static function traceInt(value:Int):Void;

    /**
     * Trace a float value
     */
    @:native("haxe_trace_float")
    public static function traceFloat(value:Float):Void;

    /**
     * Trace a boolean value
     */
    @:native("haxe_trace_bool")
    public static function traceBool(value:Bool):Void;

    /**
     * Trace a string value
     * Note: String must be a pointer + length pair
     */
    @:native("haxe_trace_string")
    public static function traceString(ptr:Int, len:Int):Void;

    /**
     * Trace any value (Dynamic type fallback)
     * For now, prints raw i64 value until Std.string() is available
     */
    @:native("haxe_trace_any")
    public static function traceAny(value:Dynamic):Void;
}
