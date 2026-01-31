package rayzor;

/**
 * 128-bit SIMD vector of 4 × Float (f32).
 *
 * SIMD4f is a zero-cost @:coreType abstract that maps directly to
 * native SIMD registers (SSE on x86, NEON on ARM). All arithmetic
 * operators compile to single vector instructions with no function
 * call overhead.
 *
 * Example:
 * ```haxe
 * var a:SIMD4f = (1.0, 2.0, 3.0, 4.0);
 * var b = SIMD4f.splat(2.0);
 * var c = a * b;       // [2, 4, 6, 8]
 * trace(c[0]);         // 2.0
 * trace(c.sum());      // 20.0
 * ```
 */
@:coreType
@:notNull
@:native("rayzor::SIMD4f")
abstract SIMD4f {
    /** Broadcast a single value to all 4 lanes */
    @:native("splat")
    public static function splat(v:Float):SIMD4f;

    /** Construct from 4 individual values */
    @:native("make")
    public static function make(x:Float, y:Float, z:Float, w:Float):SIMD4f;

    /** Load 4 contiguous floats from a pointer */
    @:native("load")
    public static function load(ptr:Ptr<Float>):SIMD4f;

    /** Store 4 floats to a pointer */
    @:native("store")
    public function store(ptr:Ptr<Float>):Void;

    /** Implicit conversion from array literal: var v:SIMD4f = [1.0, 2.0, 3.0, 4.0]; */
    @:from
    static function fromArray(arr:Array<Float>):SIMD4f;

    /** Element-wise addition */
    @:native("add")
    @:op(A + B)
    public function add(other:SIMD4f):SIMD4f;

    /** Element-wise subtraction */
    @:native("sub")
    @:op(A - B)
    public function sub(other:SIMD4f):SIMD4f;

    /** Element-wise multiplication */
    @:native("mul")
    @:op(A * B)
    public function mul(other:SIMD4f):SIMD4f;

    /** Element-wise division */
    @:native("div")
    @:op(A / B)
    public function div(other:SIMD4f):SIMD4f;

    /** Read lane: v[i] */
    @:arrayAccess
    @:native("extract")
    public function get(lane:Int):Float;

    /** Write lane: v[i] = x */
    @:arrayAccess
    @:native("insert")
    public function set(lane:Int, value:Float):SIMD4f;

    /** Horizontal sum of all 4 lanes */
    @:native("sum")
    public function sum():Float;

    /** Dot product: sum(a[i] * b[i]) */
    @:native("dot")
    public function dot(other:SIMD4f):Float;
}
