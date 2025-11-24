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
	An Array is a storage for values. You can access it using indexes or
	with its API.
**/
@:coreType
extern class Array<T> {
	/**
		The length of `this` Array.
	**/
	var length(default, null):Int;

	/**
		Creates a new Array.
	**/
	function new():Void;

	/**
		Returns a new Array by appending the elements of `a` to the elements of
		`this` Array.
		
		This operation does not modify `this` Array.
	**/
	function concat(a:Array<T>):Array<T>;

	/**
		Returns a String representation of `this` Array, with elements separated
		by `sep`.
	**/
	function join(sep:String):String;

	/**
		Removes the last element of `this` Array and returns it.
		
		If `this` is empty, `null` is returned.
	**/
	function pop():Null<T>;

	/**
		Adds the element `x` at the end of `this` Array and returns the new
		length of `this` Array.
	**/
	function push(x:T):Int;

	/**
		Reverse the order of elements of `this` Array.
	**/
	function reverse():Void;

	/**
		Removes the first element of `this` Array and returns it.
		
		If `this` is empty, `null` is returned.
	**/
	function shift():Null<T>;

	/**
		Creates a copy of the given range of the Array, starting at `pos` up to
		but not including `end`.
		
		If `end` is omitted, all elements from `pos` to the end of the Array
		are included.
		
		If `pos` or `end` is negative, their offsets are calculated from the
		end of the Array.
	**/
	function slice(pos:Int, ?end:Int):Array<T>;

	/**
		Sorts `this` Array according to the comparison function `f`.
		
		`f(x,y)` should return 0 if `x == y`, a positive Int if `x > y`
		and a negative Int if `x < y`.
	**/
	function sort(f:T->T->Int):Void;

	/**
		Removes `len` elements from `this` Array, starting at and including
		`pos`, and returns them.
	**/
	function splice(pos:Int, len:Int):Array<T>;

	/**
		Returns a String representation of `this` Array.
	**/
	function toString():String;

	/**
		Adds the element `x` at the start of `this` Array.
		
		This operation modifies `this` Array in place.
	**/
	function unshift(x:T):Void;

	/**
		Inserts the element `x` at the position `pos`.
		
		This operation modifies `this` Array in place.
		
		The offset is calculated like so:
		- If `pos` exceeds `this.length`, the offset is `this.length`.
		- If `pos` is negative, the offset is calculated from the end of `this`
		  Array, i.e. `this.length + pos`.
		- Otherwise, the offset is `pos`.
	**/
	function insert(pos:Int, x:T):Void;

	/**
		Removes the first element of `this` Array which is equal to `x`.
		
		Returns `true` if such an element was removed, `false` otherwise.
	**/
	function remove(x:T):Bool;

	/**
		Returns a shallow copy of `this` Array.
	**/
	function copy():Array<T>;

	/**
		Returns an iterator of the Array values.
	**/
	function iterator():Iterator<T>;

	/**
		Returns an iterator of the Array indices and values.
	**/
	function keyValueIterator():KeyValueIterator<Int, T>;

	/**
		Filters `this` Array by calling `f` on each element.
		
		Returns a new Array containing only the elements for which `f` returned `true`.
	**/
	function filter(f:T->Bool):Array<T>;

	/**
		Applies the function `f` to all elements of `this` Array.
		
		Returns a new Array containing the results.
	**/
	function map<S>(f:T->S):Array<S>;
}