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

// Core type definitions that are automatically imported in every module
// No package declaration means these are top-level types

/**
	The standard Void type. Only `null` values can be of the type `Void`.
**/
@:coreType abstract Void {}

/**
	The standard Boolean type, which can either be `true` or `false`.
**/
@:coreType @:notNull abstract Bool {}

/**
	The standard Int type. Its precision depends on the platform.
**/
@:coreType @:notNull abstract Int {}

/**
	The standard Float type, this is a double-precision IEEE 64bit float.
**/
@:coreType @:notNull abstract Float {}

/**
	`Null<T>` can be used to make a nullable type from a non-nullable type.
	If `T` is already nullable, `Null<T>` has no effect.
**/
@:coreType abstract Null<T> {}

/**
	Dynamic is a special type which can be used to represent any value.
**/
@:coreType abstract Dynamic<T> {}