package rayzor.concurrent;

/**
 * Multi-producer, multi-consumer channel for thread communication.
 *
 * Channels provide safe message passing between threads with compile-time validation.
 * The element type T must implement the Send trait.
 *
 * Example:
 * ```haxe
 * @:derive([Send])
 * class Message {
 *     public var value: Int;
 *     public function new(v: Int) { this.value = v; }
 * }
 *
 * var ch = Channel.init(10); // Buffered channel with capacity 10
 *
 * Thread.spawn(() -> {
 *     ch.send(new Message(42));
 * });
 *
 * var msg = ch.receive();
 * trace(msg.value); // 42
 * ```
 */
@:native("rayzor::concurrent::Channel")
extern class Channel<T> {
    /**
     * Create a new channel with the specified buffer capacity.
     *
     * - capacity = 0: Unbounded channel (uses linked list)
     * - capacity > 0: Bounded channel (blocks when full)
     *
     * The element type T must implement Send.
     *
     * @param capacity Buffer capacity (0 for unbounded)
     */
    public function new(capacity: Int);

    /**
     * Create a new channel with the specified buffer capacity.
     *
     * @deprecated Use `new Channel(capacity)` instead
     */
    @:native("init")
    public static function init<T>(capacity: Int): Channel<T>;

    /**
     * Send a value into the channel.
     *
     * For bounded channels, this blocks if the channel is full.
     * For unbounded channels, this never blocks.
     *
     * @param value The value to send (must be Send)
     */
    @:native("send")
    public function send(value: T): Void;

    /**
     * Attempt to send a value without blocking.
     *
     * @param value The value to send
     * @return true if sent successfully, false if channel is full
     */
    @:native("try_send")
    public function trySend(value: T): Bool;

    /**
     * Receive a value from the channel.
     *
     * This blocks until a value is available.
     *
     * @return The received value
     * @throws ChannelClosed if the channel is closed and empty
     */
    @:native("receive")
    public function receive(): T;

    /**
     * Attempt to receive a value without blocking.
     *
     * @return The received value, or null if channel is empty
     */
    @:native("try_receive")
    public function tryReceive(): Null<T>;

    /**
     * Close the channel.
     *
     * After closing:
     * - send() will panic
     * - receive() will return remaining values, then throw ChannelClosed
     */
    @:native("close")
    public function close(): Void;

    /**
     * Check if the channel is closed.
     *
     * @return true if the channel is closed
     */
    @:native("is_closed")
    public function isClosed(): Bool;

    /**
     * Get the number of values currently in the channel.
     *
     * @return Current number of buffered values
     */
    @:native("len")
    public function len(): Int;

    /**
     * Get the channel's capacity.
     *
     * @return Buffer capacity (0 for unbounded)
     */
    @:native("capacity")
    public function capacity(): Int;

    /**
     * Check if the channel is empty.
     *
     * @return true if no values are buffered
     */
    @:native("is_empty")
    public function isEmpty(): Bool;

    /**
     * Check if the channel is full.
     *
     * @return true if the channel is at capacity (always false for unbounded)
     */
    @:native("is_full")
    public function isFull(): Bool;
}
