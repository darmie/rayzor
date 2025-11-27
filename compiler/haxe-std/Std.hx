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

/**
	The Std class provides standard methods for manipulating basic types.
**/
extern class Std {
	/**
		Tells if a value `v` is of the type `t`. Returns `false` if `v` or `t` are null.
	**/
	static function isOfType(v:Dynamic, t:Dynamic):Bool;

	/**
		DEPRECATED. Use `Std.isOfType(v, t)` instead.
	**/
	@:deprecated('Std.is is deprecated. Use Std.isOfType instead.')
	static function is(v:Dynamic, t:Dynamic):Bool;

	/**
		Checks if object `value` is an instance of class or interface `c`.
		Returns the value cast to the type, or null if not an instance.
	**/
	static function downcast<T:{}, S:T>(value:T, c:Class<S>):S;

	@:deprecated('Std.instance() is deprecated. Use Std.downcast() instead.')
	static function instance<T:{}, S:T>(value:T, c:Class<S>):S;

	/**
		Converts any value to a String.
	**/
	@:native("haxe_std_string_ptr")
	static function string(s:Dynamic):String;

	/**
		Converts a `Float` to an `Int`, rounded towards 0.
	**/
	@:native("haxe_std_int")
	static function int(x:Float):Int;

	/**
		Converts a `String` to an `Int`.
		Returns null if the string cannot be parsed.
	**/
	@:native("haxe_std_parse_int")
	static function parseInt(x:String):Null<Int>;

	/**
		Converts a `String` to a `Float`.
		Returns NaN if the string cannot be parsed.
	**/
	@:native("haxe_std_parse_float")
	static function parseFloat(x:String):Float;

	/**
		Return a random integer between 0 included and `x` excluded.
		If `x <= 1`, the result is always 0.
	**/
	@:native("haxe_std_random")
	static function random(x:Int):Int;
}
