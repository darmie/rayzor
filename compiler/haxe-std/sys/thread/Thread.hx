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
 * System thread implementation backed by rayzor.concurrent.Thread.
 *
 * This provides the standard Haxe sys.thread.Thread API using
 * the Rayzor runtime's native thread implementation.
 */
@:native("sys::thread::Thread")
extern class Thread {
    /**
     * Returns the current thread.
     *
     * Maps to rayzor_thread_current_id internally.
     */
    @:native("current")
    public static function current(): Thread;

    /**
     * Creates a new thread that will execute the `job` function, then exit.
     *
     * This function does not setup an event loop for a new thread.
     * Maps to rayzor_thread_spawn internally.
     *
     * @param job The function to execute in the new thread
     * @return A handle to the spawned thread
     */
    @:native("create")
    public static function create(job: Void->Void): Thread;

    /**
     * Reads a message from the thread queue. If `block` is true, the function
     * blocks until a message is available. If `block` is false, the function
     * returns `null` if no message is available.
     *
     * Note: Message passing requires additional channel infrastructure.
     * Currently returns null - use rayzor.concurrent.Channel for message passing.
     */
    @:native("readMessage")
    public static function readMessage(block: Bool): Dynamic;

    /**
     * Send a message to the thread queue.
     *
     * Note: Message passing requires additional channel infrastructure.
     * Use rayzor.concurrent.Channel for message passing between threads.
     */
    @:native("sendMessage")
    public function sendMessage(msg: Dynamic): Void;

    /**
     * Check if this thread has finished execution.
     *
     * Maps to rayzor_thread_is_finished internally.
     */
    @:native("isFinished")
    public function isFinished(): Bool;

    /**
     * Wait for this thread to complete.
     *
     * Maps to rayzor_thread_join internally.
     */
    @:native("join")
    public function join(): Void;

    // ========================================================================
    // Static utility methods
    // ========================================================================

    /**
     * Yield execution to allow other threads to run.
     *
     * Maps to rayzor_thread_yield_now internally.
     */
    @:native("yield")
    public static function yield(): Void;

    /**
     * Sleep the current thread for the specified duration in seconds.
     *
     * Maps to rayzor_thread_sleep internally (converted to milliseconds).
     */
    @:native("sleep")
    public static function sleep(seconds: Float): Void;

    /**
     * Get the current thread's ID.
     *
     * Maps to rayzor_thread_current_id internally.
     */
    @:native("currentId")
    public static function currentId(): Int;
}
