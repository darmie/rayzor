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

package sys.thread;

/**
 * Counting semaphore implementation backed by rayzor's native semaphore.
 *
 * A semaphore maintains a counter that can be incremented (release) or
 * decremented (acquire). When the counter is zero, acquire() blocks until
 * another thread releases.
 */
@:native("sys::thread::Semaphore")
extern class Semaphore {
    /**
     * Creates a new semaphore with an initial value.
     *
     * Maps to rayzor_semaphore_init internally.
     *
     * @param value The initial value of the semaphore counter
     */
    @:native("new")
    public function new(value: Int): Void;

    /**
     * Locks the semaphore.
     *
     * If the value of the semaphore is zero, then the thread will block
     * until it is able to lock the semaphore.
     * If the value is non-zero, it is decreased by one.
     *
     * Maps to rayzor_semaphore_acquire internally.
     */
    @:native("acquire")
    public function acquire(): Void;

    /**
     * Try to lock the semaphore.
     *
     * If the value of the semaphore is zero, `false` is returned,
     * else the value is decreased and `true` is returned.
     *
     * If `timeout` is specified, this function will block until the thread
     * is able to acquire the semaphore, or the timeout expires.
     * `timeout` is in seconds.
     *
     * Maps to rayzor_semaphore_try_acquire internally.
     */
    @:native("tryAcquire")
    public function tryAcquire(?timeout: Float): Bool;

    /**
     * Release the semaphore.
     *
     * The value of the semaphore is increased by one.
     *
     * Maps to rayzor_semaphore_release internally.
     */
    @:native("release")
    public function release(): Void;
}
