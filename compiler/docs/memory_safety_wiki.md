# Memory Safety in Rayzor

## Overview

Rayzor implements a Rust-inspired memory safety system that prevents common memory bugs like use-after-free, double-free, and dangling pointers at compile time. Unlike traditional garbage collected languages, Rayzor uses **ownership**, **borrowing**, and **lifetimes** to guarantee memory safety without runtime overhead.

## Table of Contents

1. [Core Concepts](#core-concepts)
2. [Ownership System](#ownership-system)
3. [Borrowing and References](#borrowing-and-references)
4. [Lifetimes](#lifetimes)
5. [MIR Representation](#mir-representation)
6. [Codegen Implementation](#codegen-implementation)
7. [Safety Annotations](#safety-annotations)
8. [Diagnostic Messages](#diagnostic-messages)
9. [Implementation Architecture](#implementation-architecture)
10. [Examples](#examples)

---

## Core Concepts

### Memory Safety Goals

Rayzor's memory safety system prevents:

1. **Use-after-free**: Accessing memory after it has been deallocated
2. **Double-free**: Freeing the same memory twice
3. **Dangling pointers**: References pointing to invalid memory
4. **Data races**: Concurrent mutable access to the same data
5. **Memory leaks**: Automatic cleanup when values go out of scope

### Safety Modes

Rayzor supports two safety modes:

```haxe
@:safety(strict)  // Enforce all memory safety rules (default)
@:safety(unsafe)  // Allow unsafe operations (opt-in)
```

---

## Ownership System

### Ownership Rules

Every value in Rayzor has a single **owner**:

1. Each value has exactly one owner at a time
2. When the owner goes out of scope, the value is dropped
3. Ownership can be **moved** or **borrowed**

### Move Semantics

When you assign a heap-allocated value to another variable, ownership **moves**:

```haxe
class Resource {
    public var data: String;
}

function example() {
    var x = new Resource();  // x owns the Resource
    var y = x;               // Ownership moves to y, x is now invalid

    trace(y.data);  // ✅ OK
    trace(x.data);  // ❌ ERROR: Use after move
}
```

### Copy Types

Primitive types implement **Copy** semantics and are duplicated instead of moved:

```haxe
var x: Int = 42;
var y = x;  // x is copied, both x and y are valid

trace(x);  // ✅ OK: 42
trace(y);  // ✅ OK: 42
```

**Copy types**: `Int`, `Float`, `Bool`, `Null<T>` where T is Copy

### Clone

For deep copying heap-allocated objects, use `.clone()`:

```haxe
var x = new Resource();
var y = x.clone();  // Deep copy, both x and y own separate Resources

trace(x.data);  // ✅ OK
trace(y.data);  // ✅ OK
```

---

## Borrowing and References

### Immutable Borrows

Create read-only references with `@:borrow`:

```haxe
function printResource(@:borrow resource: Resource) {
    trace(resource.data);  // Can read but not modify
    // resource is borrowed, not moved
}

var x = new Resource();
printResource(x);  // x is borrowed
trace(x.data);     // ✅ OK: x is still valid
```

**Borrowing rules**:
- Multiple immutable borrows are allowed simultaneously
- Original owner can still read (but not modify) while borrowed

### Mutable Borrows

Create exclusive mutable references with `@:borrowMut`:

```haxe
function modifyResource(@:borrowMut resource: Resource) {
    resource.data = "modified";
}

var x = new Resource();
modifyResource(x);  // x is mutably borrowed
trace(x.data);      // ✅ OK: "modified"
```

**Mutable borrow rules**:
- Only **one** mutable borrow allowed at a time
- No other borrows (mutable or immutable) can exist simultaneously
- Ensures exclusive access to prevent data races

### Borrow Violations

```haxe
var x = new Resource();

function reader1(@:borrow r: Resource) { }
function reader2(@:borrow r: Resource) { }
function writer(@:borrowMut r: Resource) { }

reader1(x);  // ✅ OK: immutable borrow
reader2(x);  // ✅ OK: multiple immutable borrows allowed

writer(x);   // ❌ ERROR: Cannot borrow mutably while immutable borrows exist
```

---

## Lifetimes

### Lifetime Basics

Lifetimes ensure that references never outlive the data they point to:

```haxe
function dangling(): Resource {
    var x = new Resource();
    return @:borrow x;  // ❌ ERROR: Borrow outlives owner
}
```

### Lifetime Tracking

The compiler tracks the **scope** of borrows:

```haxe
var x = new Resource();

{
    var y = @:borrow x;  // Borrow starts
    trace(y.data);       // ✅ OK: y is valid
}  // Borrow ends here

trace(x.data);  // ✅ OK: x is valid again
```

### Static Lifetime

Use `@:static` for values that live for the entire program:

```haxe
@:static var GLOBAL_CONFIG = new Config();

function getConfig(): @:borrow Config {
    return @:borrow GLOBAL_CONFIG;  // ✅ OK: static lifetime
}
```

---

## MIR Representation

### Ownership Instructions

Rayzor's MIR (Mid-level IR) has explicit instructions for ownership operations:

#### Move
```rust
Move { dest: IrId, src: IrId }
```
- Transfers ownership from `src` to `dest`
- `src` becomes invalid after the move
- Example: `var y = x;` (where x is not Copy)

#### Copy
```rust
Copy { dest: IrId, src: IrId }
```
- Duplicates the value for Copy types
- Both `src` and `dest` remain valid
- Example: `var y = x;` (where x is Int)

#### BorrowImmutable
```rust
BorrowImmutable { dest: IrId, src: IrId, lifetime: LifetimeId }
```
- Creates immutable reference to `src`
- `src` remains valid and readable
- Multiple immutable borrows allowed
- Example: Function call with `@:borrow` parameter

#### BorrowMutable
```rust
BorrowMutable { dest: IrId, src: IrId, lifetime: LifetimeId }
```
- Creates exclusive mutable reference to `src`
- `src` cannot be accessed until borrow ends
- Only one mutable borrow allowed
- Example: Function call with `@:borrowMut` parameter

#### Clone
```rust
Clone { dest: IrId, src: IrId }
```
- Deep copies heap-allocated object
- Allocates new memory and copies data
- Both `src` and `dest` own independent objects
- Example: `var y = x.clone();`

#### EndBorrow
```rust
EndBorrow { borrow: IrId }
```
- Marks the end of a borrow's lifetime
- Allows validator to track borrow scopes
- Automatically inserted at scope end

#### Drop
```rust
Drop { value: IrId }
```
- Deallocates owned value
- Calls destructor if type has one
- Automatically inserted at scope end
- Example: When variable goes out of scope

### Function Calls with Ownership

```rust
CallDirect {
    dest: Option<IrId>,
    func_id: IrFunctionId,
    args: Vec<IrId>,
    arg_ownership: Vec<OwnershipMode>,  // How each argument is passed
}
```

**OwnershipMode enum**:
- `Move` - Transfer ownership to callee
- `Copy` - Duplicate value (Copy types only)
- `BorrowImmutable` - Pass read-only reference
- `BorrowMutable` - Pass exclusive mutable reference
- `Clone` - Deep copy before passing

---

## Codegen Implementation

### Cranelift Lowering

Each ownership operation is lowered to Cranelift IR:

#### Copy Implementation
```rust
IrInstruction::Copy { dest, src } => {
    let src_value = value_map[src];
    value_map[dest] = src_value;  // Simple value copy
}
```

#### Move Implementation
```rust
IrInstruction::Move { dest, src } => {
    let src_value = value_map[src];
    value_map[dest] = src_value;  // Transfer pointer
    // MIR validator ensures src is not used after this
}
```

#### BorrowImmutable Implementation
```rust
IrInstruction::BorrowImmutable { dest, src, lifetime } => {
    let src_value = value_map[src];
    value_map[dest] = src_value;  // Copy pointer (reference)
    // Source remains valid, multiple borrows allowed
}
```

#### BorrowMutable Implementation
```rust
IrInstruction::BorrowMutable { dest, src, lifetime } => {
    let src_value = value_map[src];
    value_map[dest] = src_value;  // Copy pointer (exclusive reference)
    // MIR validator ensures exclusive access
}
```

#### Clone Implementation
```rust
IrInstruction::Clone { dest, src } => {
    let src_value = value_map[src];

    // Allocate new memory
    let size = get_type_size(src);
    let malloc_fn = runtime_functions["rayzor_malloc"];
    let new_ptr = call(malloc_fn, [size]);

    // Deep copy data
    memcpy(new_ptr, src_value, size);

    value_map[dest] = new_ptr;  // Independent ownership
}
```

#### Drop Implementation
```rust
IrInstruction::Drop { value } => {
    let ptr = value_map[value];

    // Call destructor if exists
    if let Some(dtor) = get_destructor(type_id) {
        call(dtor, [ptr]);
    }

    // Free memory
    let free_fn = runtime_functions["rayzor_free"];
    call(free_fn, [ptr]);
}
```

### Runtime Functions

**rayzor_malloc(size: i64) -> *void**
- Allocates `size` bytes on the heap
- Returns pointer to allocated memory

**rayzor_free(ptr: *void)**
- Deallocates memory at `ptr`
- Called automatically when values are dropped

---

## Derived Traits

### @:derive Metadata

Similar to Rust's `#[derive(...)]`, Rayzor supports automatic trait derivation using `@:derive`:

```haxe
@:derive([Clone, Copy])
class Point {
    public var x: Int;
    public var y: Int;
}
```

**Supported Traits:**

- **Clone** - Explicit cloning via `.clone()` method (deep copy)
- **Copy** - Implicit copying (bitwise copy is safe, all fields must be Copy)
- **Debug** - Generates `toString()` implementation
- **Default** - Generates static `default()` method
- **PartialEq** - Equality operators (`==`, `!=`)
- **Eq** - Full equivalence relation (requires PartialEq)
- **PartialOrd** - Ordering operators (`<`, `<=`, `>`, `>=`)
- **Ord** - Total ordering (requires PartialOrd and Eq)
- **Hash** - Hash function for HashMap usage

**Syntax:**

```haxe
@:derive(Clone)              // Single trait
@:derive([Clone, Copy])      // Multiple traits
@:derive([Clone, Copy, Eq])  // Many traits
```

### Clone Trait

Classes with `@:derive(Clone)` automatically get a `.clone()` method:

```haxe
@:derive(Clone)
class Buffer {
    public var data: Array<Int>;
    public var size: Int;
}

var buf1 = new Buffer();
buf1.data = [1, 2, 3];

var buf2 = buf1.clone();  // Deep copy
buf2.data.push(4);

trace(buf1.data);  // [1, 2, 3]
trace(buf2.data);  // [1, 2, 3, 4]
```

### Copy Trait

Classes with `@:derive(Copy)` are implicitly copied instead of moved:

```haxe
@:derive([Clone, Copy])
class Color {
    public var r: Int;
    public var g: Int;
    public var b: Int;
}

var c1 = new Color();
c1.r = 255;

var c2 = c1;  // Copy, not move!

trace(c1.r);  // ✅ 255 (c1 is still valid)
trace(c2.r);  // ✅ 255 (c2 is an independent copy)
```

**Copy Requirements:**
- All fields must be Copy types (Int, Float, Bool, or other Copy classes)
- Bitwise copy must be safe (no heap allocations or resources)
- Copy implies Clone

### Trait Dependencies

Some traits require other traits to be implemented:

```haxe
// ❌ ERROR: Eq requires PartialEq
@:derive(Eq)
class Bad { }

// ✅ OK: PartialEq is included
@:derive([PartialEq, Eq])
class Good { }

// ✅ OK: Ord requires PartialOrd and Eq, all included
@:derive([PartialEq, Eq, PartialOrd, Ord])
class Ordered { }
```

**Dependency Rules:**
- `Eq` requires `PartialEq`
- `Ord` requires `PartialOrd` + `Eq`
- `PartialOrd` requires `PartialEq`
- `Copy` requires `Clone` (auto-added)

### Debug Trait

```haxe
@:derive(Debug)
class Person {
    public var name: String;
    public var age: Int;
}

var p = new Person();
p.name = "Alice";
p.age = 30;

trace(p);  // "Person { name: Alice, age: 30 }"
```

### Default Trait

```haxe
@:derive(Default)
class Config {
    public var timeout: Int = 5000;
    public var retries: Int = 3;
}

var config = Config.default();
trace(config.timeout);  // 5000
```

### Equality Traits

```haxe
@:derive([PartialEq, Eq])
class Point {
    public var x: Int;
    public var y: Int;
}

var p1 = new Point();
p1.x = 10;
p1.y = 20;

var p2 = new Point();
p2.x = 10;
p2.y = 20;

trace(p1 == p2);  // true (derived equality)
```

---

## Safety Annotations

### Class-Level Annotations

```haxe
@:rc              // Enable reference counting (shared ownership)
@:arc             // Enable atomic reference counting (thread-safe)
@:unsafe          // Disable safety checks for this class
```

### Method-Level Annotations

```haxe
@:borrow          // Parameter is borrowed immutably
@:borrowMut       // Parameter is borrowed mutably
@:move            // Explicit move (default for non-Copy types)
@:unsafe          // Allow unsafe operations in method
```

### Examples

```haxe
@:rc
class SharedResource {
    // Reference counted - can have multiple owners
}

class Processor {
    // Takes immutable borrow - doesn't take ownership
    public function analyze(@:borrow data: Data): Result {
        return processData(data);
    }

    // Takes mutable borrow - can modify but doesn't own
    public function transform(@:borrowMut data: Data): Void {
        data.value = computeNewValue();
    }

    // Takes ownership - data is moved
    public function consume(data: Data): Void {
        // data is dropped at end of scope
    }
}
```

---

## Diagnostic Messages

### Use After Move

```
error[E0300]: Use after move: variable 'x' was moved
  --> example.hx:15:11
   |
15 |     trace(x.data);
   |           ^ Use after move: variable 'x' was moved

     help: To fix this use-after-move error, you can:
     1. Clone the value before moving: `var y = x.clone();`
     2. Use a borrow instead: Add `@:borrow` annotation to the parameter
     3. Use the value after the move instead of before
     4. For shared ownership, use `@:rc` or `@:arc` on the class
     Note: Haxe variables are mutable by default (var). Use 'final' for immutable bindings.
```

### Double Move

```
error[E0301]: Cannot move from moved value
  --> example.hx:16:15
   |
16 |     consume2(x);
   |              ^ Cannot move 'x' - already moved

     help: The variable 'x' was moved earlier and cannot be moved again.
     Consider:
     1. Using `.clone()` to create an independent copy
     2. Using `@:borrow` to pass by reference instead of moving
     3. Using `@:rc` for shared ownership
```

### Borrow Conflict

```
error[E0302]: Cannot borrow mutably while immutably borrowed
  --> example.hx:20:11
   |
18 |     var r1 = @:borrow x;
   |              ---------- immutable borrow occurs here
19 |     var r2 = @:borrow x;  // OK: multiple immutable borrows allowed
20 |     modify(@:borrowMut x);
   |            ^^^^^^^^^^^^^ mutable borrow attempted here

     help: Mutable borrows require exclusive access.
     Wait for all immutable borrows to end before creating a mutable borrow.
```

### Dangling Reference

```
error[E0303]: Borrow outlives owner
  --> example.hx:25:12
   |
23 |     var x = new Resource();
   |         - owner created here
24 |     var y = @:borrow x;
   |             ---------- borrow created here
25 |     return y;
   |            ^ borrow returned here
   |
   = note: The borrowed value must not outlive its owner

     help: Consider:
     1. Returning a moved value instead: `return x;`
     2. Using `@:rc` for shared ownership that can be returned
     3. Ensuring the owner outlives all borrows
```

---

## Implementation Architecture

### Compilation Pipeline

```
Haxe Source
    ↓
AST (Abstract Syntax Tree)
    ↓
TAST (Typed AST)
    ↓ [populate_ownership_graph()]
Semantic Analysis
    - Build Ownership Graph
    - Track moves, borrows, aliases
    ↓
HIR (High-level IR)
    ↓ [check_memory_safety_violations()]
Memory Safety Validation
    - Detect use-after-move
    - Validate borrow rules
    - Check lifetime constraints
    ↓ (if strict mode + violations: ERROR)
MIR (Mid-level IR with ownership ops)
    - Move, Copy, Borrow, Clone, Drop
    ↓ [validate_ownership()]
MIR Validation
    - Track register states
    - Validate ownership transfers
    ↓
Cranelift JIT / LLVM AOT
    - Lower ownership ops to native code
    ↓
Native Code
```

### Key Components

**1. Ownership Graph** ([compiler/src/tast/semantic_analysis.rs](../src/tast/semantic_analysis.rs))
- Tracks ownership relationships between variables
- Records moves, borrows, and aliases
- Built during TAST analysis

**2. Lifetime Analyzer** ([compiler/src/tast/semantic_analysis.rs](../src/tast/semantic_analysis.rs))
- Computes lifetimes for all borrows
- Ensures borrows don't outlive owners
- Detects dangling references

**3. MIR Lowering** ([compiler/src/ir/hir_lowering.rs](../src/ir/hir_lowering.rs))
- Converts HIR to MIR with explicit ownership ops
- Inserts Drop instructions at scope ends
- Analyzes @:borrow annotations

**4. MIR Validator** ([compiler/src/ir/validation.rs](../src/ir/validation.rs))
- Validates ownership rules on MIR
- Tracks register states (Valid/Moved/Borrowed)
- Enforces borrow exclusivity

**5. Codegen** ([compiler/src/codegen/cranelift_backend.rs](../src/codegen/cranelift_backend.rs))
- Lowers ownership ops to Cranelift/LLVM
- Generates runtime calls (malloc, free, memcpy)
- Optimizes away no-op operations

**6. Diagnostics** ([diagnostics/src/lib.rs](../../diagnostics/src/lib.rs))
- Formats error messages with colors
- Provides helpful suggestions
- Shows source context

---

## Examples

### Example 1: Basic Ownership

```haxe
class Buffer {
    var data: Array<Int>;

    public function new() {
        data = [1, 2, 3];
    }
}

function transfer(buffer: Buffer) {
    // buffer is moved here
}

var buf = new Buffer();
transfer(buf);  // Ownership moves to transfer()
// buf is now invalid
```

### Example 2: Borrowing

```haxe
class DataProcessor {
    public function analyze(@:borrow buffer: Buffer): Int {
        // Read-only access, buffer is not moved
        return buffer.data.length;
    }

    public function modify(@:borrowMut buffer: Buffer): Void {
        // Exclusive mutable access
        buffer.data.push(4);
    }
}

var buf = new Buffer();
var processor = new DataProcessor();

var len = processor.analyze(buf);    // buf is borrowed
processor.modify(buf);                // buf is mutably borrowed
trace(buf.data);                      // ✅ buf is still valid
```

### Example 3: Clone

```haxe
var original = new Buffer();
var copy = original.clone();  // Deep copy

copy.data.push(999);

trace(original.data);  // [1, 2, 3]
trace(copy.data);      // [1, 2, 3, 999]
```

### Example 4: Reference Counting

```haxe
@:rc
class SharedBuffer {
    var data: Array<Int>;
}

var buf1 = new SharedBuffer();  // RC = 1
var buf2 = buf1;                 // RC = 2 (both own the same object)

trace(buf1.data);  // ✅ OK
trace(buf2.data);  // ✅ OK
// When both go out of scope, RC reaches 0 and memory is freed
```

### Example 5: Lifetime Constraints

```haxe
class Container {
    var item: Resource;

    // Borrow has same lifetime as 'this'
    public function getItem(): @:borrow Resource {
        return @:borrow item;
    }
}

var container = new Container();
var itemRef = container.getItem();  // Borrow tied to container's lifetime

// Use itemRef...
trace(itemRef.data);  // ✅ OK while container is valid
```

---

## Current Status

### ✅ Implemented
- MIR instruction definitions (Move, Copy, Borrow, Clone, EndBorrow)
- Cranelift codegen for all ownership operations
- Professional diagnostic formatting with colors
- Memory safety enforcement (blocks compilation on violations)
- Lifetime tracking infrastructure
- Function call ownership tracking

### 🚧 In Progress
- Ownership graph population from TAST
- MIR validator ownership rules
- HIR→MIR ownership analysis
- Drop instruction implementation
- Lifetime validation logic

### 📋 Planned
- Type-aware Clone with custom clone() methods
- Reference counting (@:rc/@:arc)
- Comprehensive test suite
- Unsafe block support
- Interior mutability (RefCell equivalent)

---

## References

- [Rust Ownership System](https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html)
- [MIR Instructions](../src/ir/instructions.rs)
- [Cranelift Backend](../src/codegen/cranelift_backend.rs)
- [Semantic Analysis](../src/tast/semantic_analysis.rs)
