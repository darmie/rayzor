package rayzor;

/**
 * Raw mutable pointer to a value of type T.
 *
 * Ptr<T> is a zero-cost abstract over Int that provides
 * typed pointer semantics. Passable to C code via `.raw()`.
 *
 * **Note:** In Rayzor, `Int` is always 64-bit (i64) at the MIR/codegen
 * level. All pointer abstracts (Ptr, Ref, Box, Usize) share this
 * 64-bit underlying representation.
 *
 * With `@:cstruct` classes, the memory layout matches C exactly,
 * so pointers are directly interoperable.
 *
 * Example:
 * ```haxe
 * var ptr:Ptr<Vec3> = Ptr.fromRaw(address);
 * var v = ptr.deref();
 * ptr.write(newValue);
 * ```
 */
@:native("rayzor::Ptr")
extern abstract Ptr<T>(Int) {
    /** Create a Ptr from a raw address */
    @:native("from_raw")
    public static function fromRaw<T>(address:Int):Ptr<T>;

    /** Get the raw address as Int */
    @:native("raw")
    public function raw():Int;

    /** Dereference — read the value at this pointer */
    @:native("deref")
    public function deref():T;

    /** Write a value at this pointer */
    @:native("write")
    public function write(value:T):Void;

    /** Pointer arithmetic — offset by N elements of size T */
    @:native("offset")
    public function offset(n:Int):Ptr<T>;

    /** Check if this pointer is null */
    @:native("isNull")
    public function isNull():Bool;
}
