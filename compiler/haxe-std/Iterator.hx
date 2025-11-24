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
	An Iterator is a structure that permits iteration over elements of type `T`.
**/
typedef Iterator<T> = {
	/**
		Returns true if the iterator has more elements, false otherwise.
	**/
	function hasNext():Bool;

	/**
		Returns the current element of the iterator and advances to the next one.
		
		This method should not be called if `hasNext` returns false.
	**/
	function next():T;
}

/**
	An Iterable is a data structure which has an iterator() method.
**/
typedef Iterable<T> = {
	/**
		Returns an iterator on the elements of this iterable.
	**/
	function iterator():Iterator<T>;
}

/**
	A KeyValueIterator is an iterator that iterates over key-value pairs.
**/
typedef KeyValueIterator<K, V> = {
	/**
		Returns true if the iterator has more elements, false otherwise.
	**/
	function hasNext():Bool;

	/**
		Returns the current element of the iterator and advances to the next one.
		
		This method should not be called if `hasNext` returns false.
	**/
	function next():{key:K, value:V};
}

/**
	A KeyValueIterable is a data structure which has a keyValueIterator() method.
**/
typedef KeyValueIterable<K, V> = {
	/**
		Returns an iterator on the key-value pairs of this iterable.
	**/
	function keyValueIterator():KeyValueIterator<K, V>;
}