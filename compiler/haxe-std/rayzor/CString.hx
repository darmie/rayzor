package rayzor;

/**
 * Null-terminated C string (char*) for FFI interop.
 *
 * CString wraps a raw `char*` pointer as an abstract over Int.
 * Use `from()` to convert a Haxe String to a CString (allocates a
 * null-terminated copy), and `toHaxeString()` to convert back.
 *
 * In `@:cstruct` classes, CString fields map to `char*` in the generated
 * C typedef, enabling direct string passing to TinyCC JIT code.
 *
 * Example:
 * ```haxe
 * var cs = CString.from("hello");
 * trace(cs.raw());       // raw char* address
 * trace(cs.toString());  // "hello"
 * cs.free();
 * ```
 */
@:native("rayzor::CString")
extern abstract CString(Int) {
    /** Create a CString from a Haxe String (allocates null-terminated copy) */
    @:native("from")
    public static function from(s:String):CString;

    /** Convert back to a Haxe String (reads null-terminated buffer) */
    @:native("to_haxe_string")
    public function toHaxeString():String;

    /** Get the raw char* address as Int */
    @:native("raw")
    public function raw():Int;

    /** Create a CString from a raw char* address */
    @:native("from_raw")
    public static function fromRaw(addr:Int):CString;

    /** Free the underlying buffer */
    @:native("free")
    public function free():Void;
}
