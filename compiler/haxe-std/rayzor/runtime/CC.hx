package rayzor.runtime;

/**
 * TinyCC runtime compiler — compile and execute C code at runtime.
 *
 * Example:
 * ```haxe
 * import rayzor.runtime.CC;
 *
 * var cc = CC.create();
 * cc.compile("
 *     int add(int a, int b) { return a + b; }
 * ");
 * cc.relocate();
 * var addPtr = cc.getSymbol("add");
 * cc.delete();
 * ```
 */
@:native("rayzor::runtime::CC")
extern class CC {
    /**
     * Create a new TCC compilation context.
     * Sets output type to memory (JIT-style).
     */
    @:native("create")
    public static function create():CC;

    /**
     * Compile a string of C source code.
     * Can be called multiple times before relocate().
     *
     * @param code C source code string
     * @return true on success, false on compilation error
     */
    @:native("compile")
    public function compile(code:String):Bool;

    /**
     * Register a symbol (value or pointer) so C code can reference it.
     * C code accesses it via `extern`: `extern long my_sym;`
     *
     * All Haxe reference types (Arc, Vec, Box, class instances) are
     * pointer-sized integers and can be passed directly.
     *
     * @param name Symbol name visible to C code
     * @param value Raw value or pointer address (i64)
     */
    @:native("addSymbol")
    public function addSymbol(name:String, value:Int):Void;

    /**
     * Relocate all compiled code into executable memory.
     * Must be called after all compile() and addSymbol() calls,
     * and before getSymbol().
     *
     * @return true on success, false on relocation error
     */
    @:native("relocate")
    public function relocate():Bool;

    /**
     * Get a function pointer or symbol address by name.
     * Must be called after relocate().
     *
     * @param name Symbol name to look up
     * @return Address as integer (castable to function pointer)
     */
    @:native("getSymbol")
    public function getSymbol(name:String):Int;

    /**
     * Free the TCC compilation context and all associated resources.
     * Note: relocated code memory remains valid (intentional leak for JIT use).
     */
    @:native("delete")
    public function delete():Void;
}
