/*
 * Rayzor Native Bytes
 *
 * High-performance byte buffer backed by native vec_u8 runtime.
 * Used as the underlying storage for haxe.io.Bytes.
 *
 * Memory layout: { ptr: *u8, len: u64, cap: u64 }
 * - Contiguous memory allocation
 * - Automatic growth (2x capacity)
 * - Zero-copy access where possible
 */

package rayzor;

/**
    A native byte buffer with efficient memory management.

    Bytes provides low-level byte storage operations backed by
    the Rayzor runtime's vec_u8 implementation.

    Example:
    ```haxe
    import rayzor.Bytes;

    var bytes = Bytes.alloc(10);  // 10 byte buffer
    bytes.set(0, 72);  // 'H'
    bytes.set(1, 105); // 'i'
    trace(bytes.get(0));  // 72
    trace(bytes.length);  // 10
    ```
**/
extern class Bytes {
    /**
        The length of the byte buffer.
    **/
    public var length(default, null): Int;

    /**
        Allocates a new byte buffer of the given size.
        All bytes are initialized to 0.

        @param size The number of bytes to allocate
        @return A new Bytes instance
    **/
    public static function alloc(size: Int): Bytes;

    /**
        Creates Bytes from a String (UTF-8 encoding).

        @param s The string to convert
        @return Bytes containing the UTF-8 encoded string
    **/
    public static function ofString(s: String): Bytes;

    /**
        Gets a single byte at the given position.

        @param pos The 0-based byte position
        @return The byte value (0-255)
    **/
    public function get(pos: Int): Int;

    /**
        Sets a single byte at the given position.

        @param pos The 0-based byte position
        @param value The byte value (0-255)
    **/
    public function set(pos: Int, value: Int): Void;

    /**
        Returns a sub-range of bytes as a new Bytes object.

        @param pos Starting position
        @param len Number of bytes
        @return A new Bytes containing the sub-range
    **/
    public function sub(pos: Int, len: Int): Bytes;

    /**
        Copies bytes from source to this buffer.

        @param srcPos Position in source to start copying from
        @param dest Destination Bytes buffer
        @param destPos Position in destination to start copying to
        @param len Number of bytes to copy
    **/
    public function blit(srcPos: Int, dest: Bytes, destPos: Int, len: Int): Void;

    /**
        Fills a range with a byte value.

        @param pos Starting position
        @param len Number of bytes to fill
        @param value The byte value to fill with
    **/
    public function fill(pos: Int, len: Int, value: Int): Void;

    /**
        Compares this buffer with another.

        @param other The Bytes to compare with
        @return 0 if equal, negative if less, positive if greater
    **/
    public function compare(other: Bytes): Int;

    /**
        Converts the bytes to a String (UTF-8 decoding).

        @return The decoded string
    **/
    public function toString(): String;

    /**
        Gets a 16-bit signed integer at the given position.
        Uses little-endian byte order.

        @param pos The byte position
        @return The 16-bit integer
    **/
    public function getInt16(pos: Int): Int;

    /**
        Gets a 32-bit signed integer at the given position.
        Uses little-endian byte order.

        @param pos The byte position
        @return The 32-bit integer
    **/
    public function getInt32(pos: Int): Int;

    /**
        Gets a 64-bit signed integer at the given position.
        Uses little-endian byte order.

        @param pos The byte position
        @return The 64-bit integer
    **/
    public function getInt64(pos: Int): Int;

    /**
        Gets a 32-bit float at the given position.
        Uses little-endian byte order.

        @param pos The byte position
        @return The float value
    **/
    public function getFloat(pos: Int): Float;

    /**
        Gets a 64-bit double at the given position.
        Uses little-endian byte order.

        @param pos The byte position
        @return The double value
    **/
    public function getDouble(pos: Int): Float;

    /**
        Sets a 16-bit signed integer at the given position.
        Uses little-endian byte order.

        @param pos The byte position
        @param value The 16-bit integer
    **/
    public function setInt16(pos: Int, value: Int): Void;

    /**
        Sets a 32-bit signed integer at the given position.
        Uses little-endian byte order.

        @param pos The byte position
        @param value The 32-bit integer
    **/
    public function setInt32(pos: Int, value: Int): Void;

    /**
        Sets a 64-bit signed integer at the given position.
        Uses little-endian byte order.

        @param pos The byte position
        @param value The 64-bit integer
    **/
    public function setInt64(pos: Int, value: Int): Void;

    /**
        Sets a 32-bit float at the given position.
        Uses little-endian byte order.

        @param pos The byte position
        @param value The float value
    **/
    public function setFloat(pos: Int, value: Float): Void;

    /**
        Sets a 64-bit double at the given position.
        Uses little-endian byte order.

        @param pos The byte position
        @param value The double value
    **/
    public function setDouble(pos: Int, value: Float): Void;
}
