package rayzor;

/**
 * Unsigned pointer-sized integer.
 *
 * Usize is a zero-cost abstract over Int that represents an unsigned
 * machine word. It is the natural type for memory addresses, sizes,
 * offsets, and array indices.
 *
 * **Note:** In Rayzor, `Int` is always 64-bit (i64) at the MIR level,
 * regardless of the standard Haxe 32-bit `Int` semantics.
 * All rayzor abstract types (Ptr, Ref, Box, Usize) use `Int` as their
 * underlying representation. The codegen backends (Cranelift, LLVM)
 * support both 32-bit and 64-bit targets — pointer sizes adapt
 * automatically via the target's pointer type.
 *
 * Implicitly convertible from/to Int. Use `.toPtr()` and `.toRef()`
 * for typed pointer conversions.
 *
 * Example:
 * ```haxe
 * var addr:Usize = 0x1000;           // implicit @:from Int
 * var ptr:Ptr<Vec3> = addr.toPtr();
 * var size:Int = addr;               // implicit @:to Int
 * ```
 */
@:native("rayzor::Usize")
extern abstract Usize(Int) {
    /** Create from an Int value */
    @:native("from_int")
    @:from
    public static function fromInt(value:Int):Usize;

    /** Convert to Int */
    @:native("to_int")
    @:to
    public function toInt():Int;

    /** Create from a raw pointer address */
    @:native("from_ptr")
    public static function fromPtr<T>(ptr:Ptr<T>):Usize;

    /** Create from a raw ref address */
    @:native("from_ref")
    public static function fromRef<T>(ref:Ref<T>):Usize;

    /** Convert to a typed mutable pointer */
    @:native("to_ptr")
    public function toPtr<T>():Ptr<T>;

    /** Convert to a typed read-only reference */
    @:native("to_ref")
    public function toRef<T>():Ref<T>;

    /** Add an offset (pointer arithmetic) */
    @:native("add")
    @:op(A + B)
    public function add(other:Usize):Usize;

    /** Subtract an offset */
    @:native("sub")
    @:op(A - B)
    public function sub(other:Usize):Usize;

    /** Bitwise AND */
    @:native("band")
    @:op(A & B)
    public function band(other:Usize):Usize;

    /** Bitwise OR */
    @:native("bor")
    @:op(A | B)
    public function bor(other:Usize):Usize;

    /** Left shift */
    @:native("shl")
    @:op(A << B)
    public function shl(bits:Int):Usize;

    /** Right shift (unsigned) */
    @:native("shr")
    @:op(A >>> B)
    public function shr(bits:Int):Usize;

    /** Align up to the given alignment (must be power of 2) */
    @:native("align_up")
    public function alignUp(alignment:Usize):Usize;

    /** Check if value is zero */
    @:native("is_zero")
    public function isZero():Bool;
}
