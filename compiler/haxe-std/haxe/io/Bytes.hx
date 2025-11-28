/*
 * Copyright (C)2005-2019 Haxe Foundation
 *
 * Permission is hereby granted, free of charge, to any person obtaining a
 * copy of this software and associated documentation files (the "Software"),
 * to deal in the Software without restriction, including without limitation
 * the rights to use, copy, modify, merge, publish, distribute, sublicense,
 * and/or sell copies of the Software, and to permit persons to whom the
 * Software is furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in
 * all copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
 * FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
 * DEALINGS IN THE SOFTWARE.
 */

package haxe.io;

import rayzor.Bytes as RayzorBytes;

/**
    Rayzor implementation of haxe.io.Bytes.

    This class wraps the native rayzor.Bytes implementation which provides
    efficient byte buffer operations backed by native Rust vec<u8>.
**/
class Bytes {
    /**
        The length of the byte buffer.
    **/
    public var length(default, null):Int;

    var b:RayzorBytes;

    function new(len:Int, data:RayzorBytes) {
        this.length = len;
        this.b = data;
    }

    /**
        Returns the byte at index `pos`.
    **/
    public function get(pos:Int):Int {
        return b.get(pos);
    }

    /**
        Stores the given byte `v` at the given position `pos`.
    **/
    public function set(pos:Int, v:Int):Void {
        b.set(pos, v);
    }

    /**
        Copies `len` bytes from `src` into this instance.
        @param pos Zero-based location in `this` instance at which to start writing bytes.
        @param src Source `Bytes` instance from which to copy bytes.
        @param srcpos Zero-based location at `src` from which bytes will be copied.
        @param len Number of bytes to be copied.
    **/
    public function blit(pos:Int, src:Bytes, srcpos:Int, len:Int):Void {
        if (pos < 0 || srcpos < 0 || len < 0 || pos + len > length || srcpos + len > src.length)
            throw Error.OutsideBounds;
        src.b.blit(srcpos, b, pos, len);
    }

    /**
        Sets `len` consecutive bytes starting from index `pos` of `this` instance
        to `value`.
    **/
    public function fill(pos:Int, len:Int, value:Int):Void {
        b.fill(pos, len, value);
    }

    /**
        Returns a new `Bytes` instance that contains a copy of `len` bytes of
        `this` instance, starting at index `pos`.
    **/
    public function sub(pos:Int, len:Int):Bytes {
        if (pos < 0 || len < 0 || pos + len > length)
            throw Error.OutsideBounds;
        return new Bytes(len, b.sub(pos, len));
    }

    /**
        Returns `0` if the bytes of `this` instance and the bytes of `other` are
        identical.

        Returns a negative value if the `length` of `this` instance is less than
        the `length` of `other`, or a positive value if the `length` of `this`
        instance is greater than the `length` of `other`.

        In case of equal `length`s, returns a negative value if the first different
        value in `other` is greater than the corresponding value in `this`
        instance; otherwise returns a positive value.
    **/
    public function compare(other:Bytes):Int {
        return b.compare(other.b);
    }

    /**
        Returns the IEEE double-precision value at the given position `pos` (in
        little-endian encoding). Result is unspecified if `pos` is outside the
        bounds.
    **/
    public function getDouble(pos:Int):Float {
        return b.getDouble(pos);
    }

    /**
        Returns the IEEE single-precision value at the given position `pos` (in
        little-endian encoding). Result is unspecified if `pos` is outside the
        bounds.
    **/
    public function getFloat(pos:Int):Float {
        return b.getFloat(pos);
    }

    /**
        Stores the given IEEE double-precision value `v` at the given position
        `pos` in little-endian encoding. Result is unspecified if writing outside
        of bounds.
    **/
    public function setDouble(pos:Int, v:Float):Void {
        b.setDouble(pos, v);
    }

    /**
        Stores the given IEEE single-precision value `v` at the given position
        `pos` in little-endian encoding. Result is unspecified if writing outside
        of bounds.
    **/
    public function setFloat(pos:Int, v:Float):Void {
        b.setFloat(pos, v);
    }

    /**
        Returns the 16-bit unsigned integer at the given position `pos` (in
        little-endian encoding).
    **/
    public function getUInt16(pos:Int):Int {
        return get(pos) | (get(pos + 1) << 8);
    }

    /**
        Stores the given 16-bit unsigned integer `v` at the given position `pos`
        (in little-endian encoding).
    **/
    public function setUInt16(pos:Int, v:Int):Void {
        set(pos, v);
        set(pos + 1, v >> 8);
    }

    /**
        Returns the 32-bit integer at the given position `pos` (in little-endian
        encoding).
    **/
    public function getInt32(pos:Int):Int {
        return b.getInt32(pos);
    }

    /**
        Returns the 64-bit integer at the given position `pos` (in little-endian
        encoding).
    **/
    public function getInt64(pos:Int):Int {
        return b.getInt64(pos);
    }

    /**
        Stores the given 32-bit integer `v` at the given position `pos` (in
        little-endian encoding).
    **/
    public function setInt32(pos:Int, v:Int):Void {
        b.setInt32(pos, v);
    }

    /**
        Stores the given 64-bit integer `v` at the given position `pos` (in
        little-endian encoding).
    **/
    public function setInt64(pos:Int, v:Int):Void {
        b.setInt64(pos, v);
    }

    /**
        Returns the `len`-bytes long string stored at the given position `pos`,
        interpreted with the given `encoding` (UTF-8 by default).
    **/
    public function getString(pos:Int, len:Int, ?encoding:Encoding):String {
        if (pos < 0 || len < 0 || pos + len > length)
            throw Error.OutsideBounds;
        var subBytes = b.sub(pos, len);
        return subBytes.toString();
    }

    /**
        Returns a `String` representation of the bytes interpreted as UTF-8.
    **/
    public function toString():String {
        return b.toString();
    }

    /**
        Returns a hexadecimal `String` representation of the bytes of `this`
        instance.
    **/
    public function toHex():String {
        var s = new StringBuf();
        var hexChars = "0123456789abcdef";
        for (i in 0...length) {
            var c = get(i);
            s.addChar(hexChars.charCodeAt(c >> 4));
            s.addChar(hexChars.charCodeAt(c & 15));
        }
        return s.toString();
    }

    /**
        Returns the bytes of `this` instance as `BytesData`.
    **/
    public function getData():BytesData {
        return b;
    }

    /**
        Returns a new `Bytes` instance with the given `length`. The values of the
        bytes are not initialized and may not be zero.
    **/
    public static function alloc(length:Int):Bytes {
        return new Bytes(length, RayzorBytes.alloc(length));
    }

    /**
        Returns the `Bytes` representation of the given `String`, using the
        specified encoding (UTF-8 by default).
    **/
    public static function ofString(s:String, ?encoding:Encoding):Bytes {
        var bytes = RayzorBytes.ofString(s);
        return new Bytes(bytes.length, bytes);
    }

    /**
        Returns the `Bytes` representation of the given `BytesData`.
    **/
    public static function ofData(b:BytesData):Bytes {
        return new Bytes(b.length, b);
    }

    /**
        Converts the given hexadecimal `String` to `Bytes`. `s` must be a string of
        even length consisting only of hexadecimal digits. For example:
        `"0FDA14058916052309"`.
    **/
    public static function ofHex(s:String):Bytes {
        var len:Int = s.length;
        if ((len & 1) != 0)
            throw "Not a hex string (odd number of digits)";
        var ret:Bytes = Bytes.alloc(len >> 1);
        for (i in 0...ret.length) {
            var high = s.charCodeAt(i * 2);
            var low = s.charCodeAt(i * 2 + 1);
            high = (high & 0xF) + ((high & 0x40) >> 6) * 9;
            low = (low & 0xF) + ((low & 0x40) >> 6) * 9;
            ret.set(i, ((high << 4) | low) & 0xFF);
        }
        return ret;
    }

    /**
        Reads the `pos`-th byte of the given `b` bytes, in the most efficient way
        possible. Behavior when reading outside of the available data is
        unspecified.
    **/
    public static function fastGet(b:BytesData, pos:Int):Int {
        return b.get(pos);
    }
}
