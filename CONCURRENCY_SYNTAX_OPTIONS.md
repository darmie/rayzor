# Concurrency Syntax Options for Rayzor

## Problem Statement

Standard Haxe metadata is supported on:
- Class/interface declarations: `@:meta class Foo {}`
- Field declarations: `@:meta var x: Int;`
- Function declarations: `@:meta function foo() {}`
- Variable declarations: `@:meta var x = 5;` (Haxe 4+)
- Some expressions: `@:meta expr`

**NOT supported in standard Haxe:**
```haxe
@:send(ch, value);           // ❌ Metadata as statement
var result = @:receive(ch);  // ❌ Metadata as expression value
```

## Option 1: Macro-Based API (Library Approach)

Use Haxe macros to provide the API, keeping it as a library feature.

```haxe
import rayzor.concurrent.*;

// Spawn
spawn(() -> doWork());

var handle = spawn(() -> 42);
var result = handle.join();

// Channels
var ch = new Channel<Message>(10);
ch.send(msg);
var msg = ch.receive();
```

**Pros:**
- ✅ Pure Haxe compatible
- ✅ No parser changes needed
- ✅ Familiar OOP syntax

**Cons:**
- ❌ Not compiler built-in (harder to optimize)
- ❌ Requires library import
- ❌ Runtime overhead (unless heavily optimized)

## Option 2: Special Identifiers (Compiler Built-in)

Treat certain identifiers as compiler built-ins (like `trace` in Haxe).

```haxe
// Spawn - compiler recognizes 'spawn' identifier
spawn(() -> doWork());

var handle = spawn(() -> 42);
var result = join(handle);

// Channels - compiler recognizes channel functions
var ch = channel(10);
send(ch, msg);
var msg = receive(ch);
var opt = try_receive(ch);

// Select - special syntax
select {
    case msg1 = receive(ch1):
        process1(msg1);
    case msg2 = receive(ch2):
        process2(msg2);
}
```

**Pros:**
- ✅ Clean syntax
- ✅ Compiler built-in (optimizable)
- ✅ Similar to Go's approach
- ✅ Haxe-compatible (just special identifiers)

**Cons:**
- ⚠️ `spawn`, `channel`, `send`, `receive` become reserved (in practice)
- ⚠️ Could conflict with user code
- ⚠️ Less explicit about "compiler magic"

## Option 3: Untyped Block with Metadata

Use `untyped` blocks with metadata for compiler interpretation.

```haxe
// Spawn
untyped @:spawn(() -> doWork());

var handle = untyped @:spawn(() -> 42);
var result = untyped @:join(handle);

// Channels
var ch = untyped @:channel(10);
untyped @:send(ch, msg);
var msg = untyped @:receive(ch);
```

**Pros:**
- ✅ Valid Haxe syntax
- ✅ Explicit "compiler magic" marker
- ✅ Won't conflict with normal code

**Cons:**
- ❌ Verbose (`untyped` everywhere)
- ❌ Looks ugly
- ❌ `untyped` usually means "skip type checking" (confusing semantics)

## Option 4: Metadata on Variable Declarations

Use metadata where Haxe actually supports it.

```haxe
// Spawn with metadata on var
@:spawn var handle = (() -> 42);
@:join(handle) var result;

// Channels
@:channel(10) var ch;
@:send(ch, msg) var _;  // Dummy var for side-effect
@:receive(ch) var msg;
```

**Pros:**
- ✅ Valid Haxe syntax
- ✅ Metadata is supported on variables

**Cons:**
- ❌ Extremely awkward
- ❌ Requires dummy variables
- ❌ Doesn't feel natural

## Option 5: Static Extension Methods (Hybrid)

Combine compiler built-in types with static extension methods.

```haxe
using rayzor.Concurrent;

// Spawn - compiler provides Thread type
var handle = Thread.spawn(() -> 42);
var result = handle.join();

// Channels - compiler provides Channel<T> type
var ch = Channel.create(10);
ch.send(msg);
var msg = ch.receive();
var opt = ch.tryReceive();

// Select - special compiler syntax
select {
    case msg1 = ch1.receive():
        process1(msg1);
    case msg2 = ch2.receive():
        process2(msg2);
}
```

**Pros:**
- ✅ Valid Haxe syntax
- ✅ Clean, readable
- ✅ Compiler can optimize the types
- ✅ Explicit about what's special (Thread, Channel types)

**Cons:**
- ⚠️ `select` still needs special syntax
- ⚠️ Requires `using` statement

## Option 6: Metadata on Function Calls (Parser Extension)

Extend Rayzor parser to accept metadata before function calls (breaking Haxe compat).

```haxe
// Rayzor-specific syntax
@:spawn doWork();
var handle = @:spawn (() -> 42);
var result = @:join handle;

var ch = @:channel 10;
@:send ch, msg;
var msg = @:receive ch;
```

**Pros:**
- ✅ Compiler built-in
- ✅ Clean syntax
- ✅ Explicit metadata markers

**Cons:**
- ❌ **BREAKS HAXE COMPATIBILITY** ❌
- ❌ Requires custom parser
- ❌ Can't use standard Haxe tools

## Recommended Approach: Option 2 + Option 5 Hybrid

**Use special identifiers for operations, compiler-provided types for values:**

```haxe
// Thread spawning - 'spawn' is a compiler built-in function
var handle = spawn(() -> 42);
var result = join(handle);

// Channels - Channel<T> is a compiler-provided generic type
var ch = channel<Message>(10);  // or just: var ch = channel(10);
send(ch, msg);
var msg = receive(ch);
var opt = try_receive(ch);

// Select - 'select' is a compiler built-in statement form
select {
    case msg1 = receive(ch1):
        process1(msg1);
    case msg2 = receive(ch2):
        process2(msg2);
}
```

**Implementation:**
- Parser recognizes `spawn`, `channel`, `send`, `receive`, `try_receive`, `join`, `select` as special keywords (like `trace`)
- These lower to compiler built-in operations
- `Channel<T>` is a compiler-provided generic type (like `Array<T>`)
- Type system validates Send/Sync traits at compile time

**Haxe Compatibility:**
- These are just identifiers in Haxe (users could theoretically define them)
- But in practice, they're de-facto reserved (like `trace`, `Type`, `Reflect`)
- Standard Haxe could parse it, just wouldn't have special semantics
- Users can conditionally compile: `#if rayzor spawn(...) #else regularCall(...) #end`

## Alternative: Macro-Based API for Full Compatibility

If strict Haxe compatibility is required:

```haxe
import rayzor.concurrent.Concurrent.*;

// Macros expand to compiler intrinsics
var handle = spawn(() -> 42);      // macro expands to compiler built-in
var result = join(handle);

var ch = channel(10);
send(ch, msg);
var msg = receive(ch);
```

**The macro definitions:**
```haxe
// In rayzor.concurrent.Concurrent
@:macro public static function spawn(expr:Expr):Expr {
    // Macro expands to compiler intrinsic call
    return macro @:rayzor_spawn $expr;
}
```

This gives us:
- ✅ Valid Haxe (macros are standard)
- ✅ Compiler can optimize (macros expand to intrinsics)
- ✅ Clean syntax
- ✅ Requires import (explicit opt-in)

## Decision Needed

Which approach should we take?

1. **Option 2** - Special identifiers (Go-like, simple, some compat concerns)
2. **Option 5** - Static extensions (Haxe-native, clean, requires `using`)
3. **Macro-based** - Maximum Haxe compatibility, requires import

Each has tradeoffs between compatibility, ergonomics, and implementation complexity.
