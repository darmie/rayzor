package rayzor;

/**
 * Read-only reference to a value of type T.
 *
 * Ref<T> is a zero-cost abstract over Int that provides
 * typed read-only pointer semantics. Passable to C code via `.raw()`.
 *
 * **Note:** In Rayzor, `Int` is always 64-bit (i64) at the MIR/codegen
 * level. All pointer abstracts (Ptr, Ref, Box, Usize) share this
 * 64-bit underlying representation.
 *
 * Unlike Ptr<T>, Ref<T> does not allow mutation.
 *
 * Example:
 * ```haxe
 * var ref:Ref<Vec3> = arc.asRef();
 * var v = ref.deref();  // read-only access
 * ```
 */
@:native("rayzor::Ref")
extern abstract Ref<T>(Int) {
    /** Create a Ref from a raw address */
    @:native("from_raw")
    public static function fromRaw<T>(address:Int):Ref<T>;

    /** Get the raw address as Int */
    @:native("raw")
    public function raw():Int;

    /** Dereference — read the value (read-only) */
    @:native("deref")
    public function deref():T;
}
