/*
 * Rayzor Native Vector
 *
 * High-performance generic vector backed by native runtime.
 * Uses monomorphization for type-specialized implementations:
 * - Vec<Int> -> VecI32 (native i32 storage)
 * - Vec<Float> -> VecF64 (native f64 storage)
 * - Vec<Bool> -> VecBool (packed boolean storage)
 * - Vec<T> -> VecPtr (pointer storage for reference types)
 *
 * Performance benefits over Array<T>:
 * - Contiguous memory allocation (better cache locality)
 * - No boxing overhead for primitives
 * - Type-specific code generation
 * - Rust's efficient memory allocator
 */

package rayzor;

/**
    A native generic vector with high-performance operations.

    Vec<T> is monomorphized at compile time, generating specialized
    implementations for each concrete type argument.

    Example:
    ```haxe
    import rayzor.Vec;

    var ints = new Vec<Int>();
    ints.push(1);
    ints.push(2);
    ints.push(3);
    trace(ints.get(0));  // 1
    trace(ints.length);  // 3

    var floats = new Vec<Float>();
    floats.push(1.5);
    floats.push(2.5);
    ```
**/
@:generic
extern class Vec<T> {
    /**
        Creates a new empty vector.
    **/
    public function new(): Void;

    /**
        Returns the number of elements in the vector.
    **/
    public function length(): Int;

    /**
        Adds an element to the end of the vector.

        @param value The element to add
    **/
    public function push(value: T): Void;

    /**
        Removes and returns the last element.

        @return The removed element, or null/0 if empty
    **/
    public function pop(): T;

    /**
        Gets the element at the specified index.

        @param index The 0-based index
        @return The element at that index
    **/
    public function get(index: Int): T;

    /**
        Sets the element at the specified index.

        @param index The 0-based index
        @param value The value to set
    **/
    public function set(index: Int, value: T): Void;

    /**
        Returns the current capacity of the vector.

        @return The number of elements that can be stored without reallocation
    **/
    public function capacity(): Int;

    /**
        Checks if the vector is empty.

        @return True if the vector has no elements
    **/
    public function isEmpty(): Bool;

    /**
        Removes all elements from the vector.
        Capacity is retained.
    **/
    public function clear(): Void;

    /**
        Returns the first element.

        @return The first element, or null/0 if empty
    **/
    public function first(): T;

    /**
        Returns the last element.

        @return The last element, or null/0 if empty
    **/
    public function last(): T;

    /**
        Sorts the vector in ascending order (for primitive types).
        For Vec<Int> and Vec<Float>, uses natural ordering.
        For custom comparison, use sortBy().

        This operation modifies the vector in place.
    **/
    public function sort(): Void;

    /**
        Sorts the vector using a comparison function.

        @param compare Function that returns negative if a < b, 0 if a == b, positive if a > b
    **/
    public function sortBy(compare: T->T->Int): Void;
}
