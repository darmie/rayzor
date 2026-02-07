# Rayzor Standard Library Design

## Overview

The Rayzor standard library is written in **pure Haxe** using `extern` classes that map to runtime primitives implemented in the Rayzor compiler/runtime. This provides:

- ✅ 100% Haxe syntax compatibility
- ✅ Type-safe API
- ✅ Compiler can optimize and inline
- ✅ No parser extensions needed
- ✅ Clear separation: Haxe API → Runtime implementation

## Architecture

```
Haxe Code
    ↓
Rayzor Stdlib (extern classes)    ← Pure Haxe .hx files
    ↓
Rayzor Runtime Primitives         ← Compiler intrinsics
    ↓
MIR/Codegen
    ↓
Native Code
```

## Directory Structure

```
rayzor/
├── stdlib/                    # Rayzor standard library (Haxe)
│   ├── rayzor/
│   │   ├── concurrent/
│   │   │   ├── Thread.hx      # extern class
│   │   │   ├── Channel.hx     # extern class
│   │   │   └── Sync.hx        # Mutex, Arc, etc.
│   │   ├── memory/
│   │   │   ├── Rc.hx          # Reference counted pointer
│   │   │   ├── Arc.hx         # Atomic reference counted
│   │   │   └── Box.hx         # Heap allocation
│   │   ├── collections/
│   │   │   ├── Vec.hx         # Generic vector
│   │   │   ├── HashMap.hx     # Generic hashmap
│   │   │   └── Option.hx      # Optional value
│   │   └── async/
│   │       ├── Promise.hx     # Async promise
│   │       └── Future.hx      # Future value
│   └── README.md
│
├── runtime/                   # Rayzor runtime (Rust)
│   ├── src/
│   │   ├── concurrent/
│   │   │   ├── thread.rs      # Thread scheduler
│   │   │   └── channel.rs     # Channel implementation
│   │   ├── memory/
│   │   │   └── rc.rs          # Reference counting
│   │   └── lib.rs
│   └── Cargo.toml
│
└── compiler/                  # Rayzor compiler (Rust)
    └── src/
        ├── stdlib/            # Stdlib integration
        │   ├── thread.rs      # Thread intrinsics
        │   ├── channel.rs     # Channel intrinsics
        │   └── mod.rs
        └── ...
```

## Example: Concurrent Module

### stdlib/rayzor/concurrent/Thread.hx

```haxe
package rayzor.concurrent;

/**
 * Lightweight thread handle.
 *
 * Represents a spawned thread. Use Thread.spawn() to create.
 * Call join() to wait for completion and get the result.
 */
@:native("rayzor::thread::Thread")
extern class Thread<T> {
    /**
     * Spawn a new lightweight thread.
     *
     * The function must only capture Send types.
     * Returns a handle that can be used to join.
     *
     * @param fn Function to run in the thread
     * @return Thread handle
     */
    @:native("spawn")
    public static function spawn<T>(fn: Void -> T): Thread<T>;

    /**
     * Wait for thread completion and get result.
     *
     * Blocks until the thread finishes.
     *
     * @return The value returned by the thread function
     */
    @:native("join")
    public function join(): T;
}
```

### stdlib/rayzor/concurrent/Channel.hx

```haxe
package rayzor.concurrent;

/**
 * Message passing channel for inter-thread communication.
 *
 * Channels allow threads to safely send and receive messages.
 * The element type T must implement the Send trait.
 */
@:native("rayzor::channel::Channel")
extern class Channel<T> {
    /**
     * Create a new channel with the specified capacity.
     *
     * @param capacity Buffer size (0 for unbuffered)
     * @return New channel
     */
    @:native("new")
    public function new(capacity: Int);

    /**
     * Send a message to the channel.
     *
     * Blocks if the channel is full.
     *
     * @param value Message to send (must be Send)
     */
    @:native("send")
    public function send(value: T): Void;

    /**
     * Receive a message from the channel.
     *
     * Blocks until a message is available.
     *
     * @return The received message
     */
    @:native("receive")
    public function receive(): T;

    /**
     * Try to receive a message without blocking.
     *
     * Returns Some(value) if a message is available,
     * None if the channel is empty.
     *
     * @return Optional message
     */
    @:native("try_receive")
    public function tryReceive(): Option<T>;

    /**
     * Close the channel.
     *
     * No more messages can be sent after closing.
     */
    @:native("close")
    public function close(): Void;
}
```

### stdlib/rayzor/concurrent/Select.hx

```haxe
package rayzor.concurrent;

/**
 * Select over multiple channel operations.
 *
 * Note: This is implemented as a macro that expands to
 * compiler built-in select statement.
 */
class Select {
    /**
     * Wait for the first available channel operation.
     *
     * Usage:
     * ```haxe
     * Select.wait([
     *     Case(ch1.receive, (msg) -> trace("ch1: " + msg)),
     *     Case(ch2.receive, (msg) -> trace("ch2: " + msg)),
     * ]);
     * ```
     */
    @:macro
    public static function wait(cases: Array<SelectCase>): Void {
        // Macro expands to compiler built-in select statement
        // Implementation in compiler/src/stdlib/select_macro.rs
    }
}

/**
 * Represents a case in a select statement.
 */
extern class SelectCase<T> {
    @:native("case")
    public static function receive<T>(ch: Channel<T>, handler: T -> Void): SelectCase<T>;
}
```

## Example Usage

```haxe
import rayzor.concurrent.*;

@:derive([Send])
class Message {
    public var data: String;
    public function new(d: String) { data = d; }
}

class Main {
    static function main() {
        // Spawn threads
        var handle = Thread.spawn(() -> {
            trace("Worker thread");
            return 42;
        });

        // Create channel
        var ch = new Channel<Message>(10);

        // Sender thread
        Thread.spawn(() -> {
            ch.send(new Message("Hello"));
            ch.send(new Message("World"));
            ch.close();
        });

        // Receiver thread
        Thread.spawn(() -> {
            var msg = ch.receive();
            trace(msg.data);
        });

        // Non-blocking receive
        switch (ch.tryReceive()) {
            case Some(msg): trace(msg.data);
            case None: trace("No message");
        }

        // Join thread
        var result = handle.join();
        trace("Thread returned: " + result);

        // Select (using macro)
        var ch1 = new Channel<Int>(5);
        var ch2 = new Channel<String>(5);

        Select.wait([
            SelectCase.receive(ch1, (x) -> trace("Got int: " + x)),
            SelectCase.receive(ch2, (s) -> trace("Got string: " + s)),
        ]);
    }
}
```

## Compiler Integration

### 1. Extern Resolution

When the compiler sees `@:native("rayzor::thread::Thread")`, it:
1. Maps the Haxe class to a compiler intrinsic type
2. Validates trait requirements (e.g., Send for spawn)
3. Lowers to MIR using runtime primitives

### 2. Runtime Primitive Mapping

In `compiler/src/stdlib/thread.rs`:

```rust
/// Handle Thread.spawn() extern call
pub fn lower_thread_spawn(
    ctx: &mut LoweringContext,
    closure: &TypedExpression,
) -> Option<IrId> {
    // 1. Validate closure only captures Send types
    validate_send_captures(ctx, closure)?;

    // 2. Lower to MIR intrinsic
    let closure_id = ctx.lower_expression(closure)?;
    let thread_handle = ctx.builder.build_intrinsic(
        Intrinsic::ThreadSpawn,
        vec![closure_id]
    );

    Some(thread_handle)
}

/// Handle Thread.join() extern call
pub fn lower_thread_join(
    ctx: &mut LoweringContext,
    thread_handle: IrId,
) -> Option<IrId> {
    // Lower to MIR intrinsic that blocks until thread completes
    let result = ctx.builder.build_intrinsic(
        Intrinsic::ThreadJoin,
        vec![thread_handle]
    );

    Some(result)
}
```

### 3. MIR Instructions

New MIR instructions for concurrency:

```rust
pub enum IrInstruction {
    // ... existing instructions ...

    /// Spawn lightweight thread
    ThreadSpawn {
        dest: IrId,           // Thread handle
        closure: IrId,        // Closure to execute
    },

    /// Wait for thread completion
    ThreadJoin {
        dest: Option<IrId>,   // Result value
        handle: IrId,         // Thread handle
    },

    /// Create channel
    ChannelNew {
        dest: IrId,           // Channel handle
        capacity: i32,        // Buffer capacity
        elem_type: IrType,    // Element type
    },

    /// Send to channel
    ChannelSend {
        channel: IrId,        // Channel handle
        value: IrId,          // Value to send
    },

    /// Receive from channel (blocking)
    ChannelReceive {
        dest: IrId,           // Received value
        channel: IrId,        // Channel handle
    },

    /// Try receive from channel (non-blocking)
    ChannelTryReceive {
        dest: IrId,           // Option<T> result
        channel: IrId,        // Channel handle
    },
}
```

### 4. Codegen Integration

In Cranelift backend:

```rust
IrInstruction::ThreadSpawn { dest, closure } => {
    // Call runtime: rayzor_thread_spawn(closure_ptr)
    let spawn_fn = self.get_runtime_fn("rayzor_thread_spawn");
    let thread_handle = self.builder.ins().call(spawn_fn, &[closure_ptr]);
    self.register_value(*dest, thread_handle);
}

IrInstruction::ThreadJoin { dest, handle } => {
    // Call runtime: rayzor_thread_join(handle)
    let join_fn = self.get_runtime_fn("rayzor_thread_join");
    let result = self.builder.ins().call(join_fn, &[handle]);
    if let Some(dest_reg) = dest {
        self.register_value(*dest_reg, result);
    }
}
```

## Runtime Implementation

In `runtime/src/concurrent/thread.rs`:

```rust
use std::sync::Arc;
use std::thread;

/// Runtime representation of a thread handle
pub struct RayzorThreadHandle {
    handle: Option<thread::JoinHandle<Box<dyn Any + Send>>>,
}

/// Spawn a new lightweight thread
#[no_mangle]
pub extern "C" fn rayzor_thread_spawn(
    closure: *mut u8  // Pointer to closure data
) -> *mut RayzorThreadHandle {
    let closure_fn = unsafe { /* marshal closure */ };

    let handle = thread::spawn(move || {
        closure_fn()
    });

    Box::into_raw(Box::new(RayzorThreadHandle {
        handle: Some(handle)
    }))
}

/// Join a thread and get result
#[no_mangle]
pub extern "C" fn rayzor_thread_join(
    handle: *mut RayzorThreadHandle
) -> *mut u8 {
    let mut handle = unsafe { Box::from_raw(handle) };
    let join_handle = handle.handle.take().unwrap();
    let result = join_handle.join().unwrap();

    // Marshal result back to Haxe representation
    Box::into_raw(result) as *mut u8
}
```

## Benefits of This Approach

1. **Pure Haxe API** - No syntax extensions, fully compatible
2. **Type Safety** - Haxe type system validates usage
3. **Compiler Optimization** - Can inline and optimize extern calls
4. **Clean Separation** - API (Haxe) vs Implementation (Runtime)
5. **Familiar Pattern** - Same as Haxe's existing `sys` package
6. **Documentable** - Standard Haxe doc comments work
7. **IDE Support** - Auto-completion, type hints, etc.
8. **Testable** - Can write Haxe tests for the API

## Trait Validation

The compiler validates derived traits during extern call lowering:

```haxe
class NonSend {
    var x: Int;
}

// ERROR: NonSend does not implement Send
Thread.spawn(() -> {
    var ns = new NonSend();
    trace(ns);
});
```

Compiler error during `lower_thread_spawn`:
```
Error: Cannot spawn thread - captured variable 'ns' of type 'NonSend'
does not implement the Send trait.

Help: Add @:derive([Send]) to class NonSend, or don't capture this variable.
```

## Next Steps

1. Create stdlib directory structure
2. Implement core extern classes (Thread, Channel, Option, Vec, etc.)
3. Add compiler intrinsics for each extern primitive
4. Implement runtime functions in Rust
5. Add MIR instructions for concurrency
6. Integrate with Cranelift/LLVM codegen
7. Write tests for stdlib API

This gives us a clean, idiomatic Haxe API while maintaining full compiler control and optimization!
