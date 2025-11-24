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
	The basic String class.
	
	A Haxe String is immutable, it is not possible to modify individual
	characters. No method of this class changes the state of `this` String.
**/
@:coreType
@:final
extern class String {
	/**
		The number of characters in `this` String.
	**/
	var length(default, null):Int;

	/**
		Creates a copy of `this` String.
	**/
	function new(string:String):Void;

	/**
		Returns the character at position `index` of `this` String.
		
		If `index` is negative or exceeds `this.length`, the empty String `""`
		is returned.
	**/
	function charAt(index:Int):String;

	/**
		Returns the character code at position `index` of `this` String.
		
		If `index` is negative or exceeds `this.length`, `null` is returned.
	**/
	function charCodeAt(index:Int):Null<Int>;

	/**
		Returns the position of the leftmost occurrence of `str` within `this`
		String.
		
		If `startIndex` is given, the search is performed within the substring
		of `this` String starting from `startIndex`.
		
		If `str` cannot be found, -1 is returned.
	**/
	function indexOf(str:String, ?startIndex:Int):Int;

	/**
		Returns the position of the rightmost occurrence of `str` within `this`
		String.
		
		If `startIndex` is given, the search is performed within the substring
		of `this` String from 0 to `startIndex + str.length`.
		
		If `str` cannot be found, -1 is returned.
	**/
	function lastIndexOf(str:String, ?startIndex:Int):Int;

	/**
		Splits `this` String at each occurrence of `delimiter`.
		
		If `delimiter` is the empty String `""`, `this` String is split into an
		Array of `this.length` elements, where the elements correspond to the
		characters of `this` String.
		
		If `delimiter` is not found within `this` String, the result is an Array
		with one element, which equals `this` String.
	**/
	function split(delimiter:String):Array<String>;

	/**
		Returns `len` characters of `this` String, starting at position `pos`.
		
		If `len` is omitted, all characters from position `pos` to the end of
		`this` String are included.
		
		If `pos` is negative, its value is calculated from the end of `this`
		String by `this.length + pos`.
		
		If `pos` exceeds `this.length`, the empty String `""` is returned.
	**/
	function substr(pos:Int, ?len:Int):String;

	/**
		Returns the part of `this` String from `startIndex` to but not including `endIndex`.
		
		If `endIndex` is omitted, all characters from `startIndex` to the end of
		`this` String are included.
		
		If `startIndex` or `endIndex` is negative, their value is calculated from the
		end of `this` String by `this.length + startIndex` or `this.length + endIndex`.
		
		If `startIndex` exceeds `endIndex`, the result is unspecified.
	**/
	function substring(startIndex:Int, ?endIndex:Int):String;

	/**
		Returns a String where all characters of `this` String are lower case.
	**/
	function toLowerCase():String;

	/**
		Returns a String where all characters of `this` String are upper case.
	**/
	function toUpperCase():String;

	/**
		Returns the String itself.
	**/
	function toString():String;

	/**
		Returns the String corresponding to the character code `code`.
		
		If `code` is negative or greater than 0x10FFFF, the empty String `""`
		is returned.
	**/
	static function fromCharCode(code:Int):String;
}