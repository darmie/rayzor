# Rayzor Memory Management Strategy

## Overview

Rayzor uses **ownership-based memory management** inspired by Rust's model. The compiler performs compile-time analysis to determine when objects should be allocated and freed, eliminating the need for a garbage collector in the common case. GC is reserved exclusively for `Dynamic` types and objects whose sizes cannot be determined at compile time.

This document covers the full memory safety pipeline: ownership analysis, lifetime analysis, borrow checking, drop analysis, and escape analysis.

## Design Philosophy

| Approach | When Used |
| --- | --- |
| Ownership + Automatic Drop | Default for all heap-allocated classes when drop conditions are met |
| Reference Counting (`@:rc`, `@:arc`) | Shared ownership, thread-safe sharing |
| Runtime Managed | Thread, Channel, Arc, Mutex (runtime handles cleanup) |
| No Drop | Primitives (Int, Float, Bool), Dynamic |
| GC | `Dynamic` types or objects with unknown compile-time size only |

The key insight: most Haxe programs use concrete types with known sizes. For these, the compiler can statically determine ownership, insertion of `Free` instructions at the correct points, and verify safety -- all without runtime overhead.

## Memory Annotations

Rayzor extends Haxe with opt-in memory annotations that control ownership semantics:

### Class-Level Annotations

```haxe
// Default: no annotation = Dynamic/runtime-managed (no safety analysis)
class DefaultClass { }

// Opt into memory safety analysis
@:safety
class SafeClass { }

// Ownership models
@:safety @:move    class MoveOnly { }     // Transfer ownership on assignment
@:safety @:unique  class UniquePtr { }    // No aliasing allowed
@:safety @:rc      class SharedLocal { }  // Reference counted (single-thread)
@:safety @:arc     class SharedAtomic { } // Atomic reference counted (thread-safe)

// Concurrency traits
@:safety @:derive([Send, Sync])
class ThreadSafe { }
```

### Parameter-Level Annotations

```haxe
@:safety
class Resource {
    // Borrow: caller retains ownership, function gets read access
    public function inspect(@:borrow ref: OtherResource): Void { }

    // Owned: function takes ownership, caller can no longer use it
    public function consume(@:owned ref: OtherResource): Void { }

    // Move: explicit ownership transfer
    public function transfer(@:move ref: OtherResource): Void { }
}
```

### Safety Mode Configuration

The `@:safety` annotation on the `Main` class controls program-wide behavior:

| Annotation | Mode | Behavior |
| --- | --- | --- |
| None | Default | No safety analysis; runtime-managed memory |
| `@:safety` or `@:safety(false)` | Non-Strict | Safety analysis on annotated classes; unannotated classes auto-wrapped in Rc |
| `@:safety(true)` | Strict | All classes must have `@:safety`; compile error otherwise |

## Pipeline Overview

The memory safety pipeline runs as part of semantic analysis, before code generation:

```text
Source Code (with memory annotations)
        |
   Type Checking (TAST)
        |
   Semantic Graph Construction (CFG, DFG/SSA, Call Graph)
        |
   Ownership Graph Analysis
   - Track ownership kinds per variable
   - Record borrow edges and move edges
   - Detect aliasing violations and use-after-move
        |
   Lifetime Analysis
   - Create lifetime regions from CFG scopes
   - Assign lifetimes to SSA variables
   - Generate constraints from code structure
        |
   Constraint Solver
   - Union-Find for equality constraints
   - Outlives graph for ordering constraints
   - Tarjan's SCC for cycle detection
   - Kahn's topological sort for ordering
        |
   Global Lifetime Constraints
   - Inter-procedural analysis across call graph
   - Call site constraint generation
   - Recursive function group handling (SCCs)
   - Virtual method lifetime polymorphism
        |
   Escape Analysis
   - Detect allocation sites in DFG
   - Trace escape via def-use chains
   - Identify stack allocation opportunities
        |
   Send/Sync Validation
   - Thread::spawn closure capture validation
   - Channel<T>: T must be Send
   - Arc<T>: T must be Send + Sync
        |
   Drop Analysis (during HIR/MIR lowering)
   - Last-use analysis per variable
   - Insert Free instructions at drop points
   - Handle lambda captures as escaping
        |
   Code Generation (with memory instructions)
```

---

## 1. Ownership Analysis

**File:** `compiler/src/semantic_graph/ownership_graph.rs`

The ownership graph tracks every variable's ownership state and all borrowing/moving relationships.

### Ownership Kinds

```text
Owned       - Full ownership. Can move, mutate, and drop.
Borrowed    - Immutable borrow. Read-only access, cannot move or mutate.
BorrowedMut - Mutable borrow. Exclusive modification access.
Shared      - Reference-counted shared ownership (for Haxe interop / @:rc / @:arc).
Moved       - Ownership transferred. Variable is no longer accessible.
Unknown     - Analysis could not determine ownership (conservative).
```

### Core Data Structures

**OwnershipNode** -- per-variable tracking:
- `ownership_kind`: Current ownership state
- `lifetime`: Assigned lifetime ID
- `borrowed_by`: List of borrow edges pointing to this variable
- `borrows_from`: List of borrow edges this variable holds
- `is_moved`: Whether ownership has been transferred away
- `allocation_site`: Where the variable was allocated (from DFG)

**BorrowEdge** -- borrowing relationship:
- `borrower` / `borrowed`: The two ends of the borrow
- `borrow_type`: Immutable, Mutable, or Weak
- `borrow_scope`: The scope in which the borrow is active
- `borrow_lifetime`: How long the borrow persists

**MoveEdge** -- ownership transfer:
- `source` / `destination`: The two ends of the move
- `move_type`: Explicit, Implicit, Call (argument passing), or Destruction
- `invalidates_source`: Whether the source becomes unusable

### Violation Detection

The ownership graph detects four categories of violations:

1. **Use After Move** -- accessing a variable after its ownership was transferred
2. **Aliasing Violation** -- holding both mutable and immutable borrows simultaneously
3. **Dangling Pointer** -- using a reference after the referent's lifetime has ended
4. **Double Free** -- multiple deallocation of the same resource

---

## 2. Lifetime Analysis

**File:** `compiler/src/semantic_graph/analysis/lifetime_analyzer.rs`

Lifetime analysis determines how long each variable and reference must remain valid.

### Analysis Phases

The analyzer runs 5 phases per function:

1. **Create Lifetime Regions** -- build a hierarchy of regions from CFG scopes (global, function-level, block-scoped, with parent-child relationships)
2. **Assign Initial Lifetimes** -- map SSA variables to lifetime IDs based on their defining scope, refine from uses (flow-sensitive)
3. **Generate Constraints** -- walk the MIR and produce constraints from field access, array access, memory load/store, function calls, return statements, and phi nodes
4. **Solve Constraint System** -- invoke the constraint solver
5. **Check Violations** -- detect use-after-free, dangling references, and return-of-local-reference

### Constraint Types

```text
Outlives { longer, shorter }          -- 'a must outlive 'b
Equal { left, right }                 -- 'a and 'b are the same lifetime
CallConstraint { callee, args, ret }  -- function call flows
BorrowConstraint { var, lifetime }    -- borrow must not outlive referent
ReturnConstraint { func, ret, params} -- return lifetime bounds
FieldConstraint { object, field }     -- field access lifetime
TypeConstraint { variable, type }     -- type-based lifetime bounds
```

### Violation Types

```text
UseAfterFree          -- Variable used after its lifetime ended
DanglingReference     -- Reference outlives its referent
ReturnLocalReference  -- Returning a reference to a local variable
ConflictingConstraints-- Unsatisfiable constraint system
```

### Inter-Procedural Analysis

**File:** `compiler/src/semantic_graph/analysis/global_lifetime_constraints.rs`

For cross-function analysis, the global constraint system tracks:

- **Function Lifetime Signatures** -- parameter lifetimes, return lifetime, generic lifetime params, lifetime bounds
- **Call Site Constraints** -- how arguments flow to parameters and returns flow to callers
- **Cross-Function Flows** -- lifetime relationships that span function boundaries
- **Recursive Constraint Groups** -- handled via SCC detection in the call graph
- **Virtual Method Constraints** -- lifetime polymorphism for overridden methods

---

## 3. Constraint Solver

**File:** `compiler/src/semantic_graph/analysis/lifetime_solver.rs`

The solver resolves the constraint system produced by lifetime analysis.

### Algorithm (7 phases)

1. **Hash + Cache Check** -- LRU cache lookup for previously solved constraint sets (85-95% hit rate in incremental scenarios)
2. **Union-Find for Equality** -- `Equal` constraints are resolved in O(alpha(n)) using union-find with path compression and union by rank
3. **Build Outlives Graph** -- `Outlives` constraints form a directed graph of lifetime ordering
4. **Cycle Detection** -- Tarjan's algorithm identifies strongly connected components in the outlives graph (O(V+E))
5. **Topological Sort** -- Kahn's algorithm produces a longest-lived to shortest-lived ordering (O(V+E))
6. **Generate Assignments** -- map variables to canonical lifetime representatives
7. **Cache Solution** -- store result in LRU cache for future queries

### Conflict Detection

When constraints are unsatisfiable, the solver reports:

- **OutlivesCycle** -- cyclic outlives relationships (A: B, B: A)
- **EqualityOutlivesConflict** -- equal lifetimes with conflicting outlives
- **ImpossibleConstraints** -- fundamentally unsatisfiable system

### Performance

- Constraint solving: <1ms for typical systems
- Memory: ~20 bytes/constraint + ~40 bytes/variable
- Cache hit ratio: 85-95% for incremental compilation
- Max constraint system: 50,000 constraints (configurable)

---

## 4. Drop Analysis

**File:** `compiler/src/ir/drop_analysis.rs`

Drop analysis determines when and how each variable should be deallocated.

### Drop Behaviors

```text
AutoDrop        -- Compiler inserts a Free instruction at the drop point.
                   Used for heap-allocated classes when drop conditions are met
                   (heap-allocated, non-escaping, at last use). Works regardless
                   of @:safety annotation -- the compiler automatically determines
                   whether a Free is needed based on analysis.

RuntimeManaged  -- The runtime handles cleanup.
                   Used for Thread, Channel, Arc, Mutex.
                   These types have custom Drop implementations in the
                   rayzor-runtime library.

NoDrop          -- No cleanup needed.
                   Used for primitives (Int, Float, Bool), arrays,
                   and Dynamic types. Primitives are value types;
                   Dynamic uses runtime management.
```

### Last-Use Analysis

The drop point analyzer traverses each function body to identify:

1. **All variable uses** -- every statement and expression that references a variable
2. **Last use** -- the final statement index where a variable is referenced
3. **Heap allocations** -- variables created via `new` or allocation calls
4. **Reassignments** -- variables assigned multiple times (drop at reassignment, not last use)
5. **Escaping variables** -- variables returned, passed to functions, or captured by lambdas
6. **Lambda captures** -- variables captured by closures are marked as truly escaping (the closure owns them)

### Drop Point Rules

A `Free` instruction is inserted for a variable when ALL of these conditions hold:
- The variable is heap-allocated
- The variable is NOT escaping (not returned, not captured by lambda, not stored globally)
- The current statement is the variable's last use
- The variable's type has `AutoDrop` behavior

Variables in loops receive special handling -- if the last use is inside a loop, the drop must account for multiple iterations.

---

## 5. Escape Analysis

**File:** `compiler/src/semantic_graph/analysis/escape_analyzer.rs`

Escape analysis determines whether heap-allocated objects can be optimized to stack allocation.

### Escape Classifications

```text
NoEscape            -- Object does not escape its defining scope.
                       Candidate for stack allocation.

EscapesViaReturn    -- Object is returned from the function.
                       Must remain on the heap.

EscapesViaCall      -- Object is passed as an argument to another function.
                       May need heap allocation depending on callee.

EscapesViaGlobal    -- Object is stored in a global variable.
                       Must remain on the heap.

EscapesViaContainer -- Object is stored in another object that itself escapes.
                       Transitively requires heap allocation.

Unknown             -- Conservative assumption when analysis is incomplete.
                       Treated as escaping (heap allocation).
```

### Analysis Algorithm

1. **Find Allocation Sites** -- scan the DFG for `Allocation` nodes, constructor calls, and implicit allocations (string concatenation, array operations)
2. **Trace Def-Use Chains** -- for each allocation, follow all uses through the DFG
3. **Classify Escapes** -- each use is checked: return -> EscapesViaReturn, call argument -> EscapesViaCall, store -> EscapesViaGlobal, field/array access -> NoEscape
4. **Generate Optimization Hints** -- NoEscape allocations suggest stack allocation; small non-escaping functions suggest inlining; dead allocations suggest removal

### Optimization Hints

The escape analyzer generates actionable optimization hints:

- **StackAllocation** -- replace `malloc` with stack-based `alloca` for non-escaping objects
- **InlineFunction** -- inline small functions (<10 DFG nodes, single basic block) to expose more escape analysis opportunities
- **RemoveAllocation** -- eliminate allocations whose results are never used
- **CombineAllocations** -- merge multiple small allocations into a single larger one

---

## 6. Send/Sync Validation

**File:** `compiler/src/tast/send_sync_validator.rs`

Rayzor validates thread-safety properties at compile time for concurrent code.

### Validation Rules

**Thread::spawn(closure)**:
- All variables captured by the closure must implement `Send`
- The closure body is analyzed by `CaptureAnalyzer` to identify captures
- Each captured variable is validated against `Send` requirements

**Channel\<T\>**:
- The type `T` must implement `Send`
- Checked at Channel construction time

**Arc\<T\>**:
- The type `T` must implement both `Send` and `Sync`
- Enforced at Arc instantiation

### Deriving Send/Sync

Classes can derive Send and Sync traits:

```haxe
@:safety
@:derive([Send, Sync])
class SharedState {
    var counter: Int;    // Int is Send + Sync
    var name: String;    // String is Send + Sync
}
```

The validator checks that all fields of a `Send`/`Sync` class are themselves `Send`/`Sync`. If a field fails the check, a compile error is emitted.

---

## Runtime Memory Primitives

The `rayzor-runtime` crate provides the low-level memory operations that generated code calls:

```text
rayzor_malloc(size: u64) -> *mut u8       -- Allocate memory
rayzor_realloc(ptr, old_size, new_size)   -- Resize allocation
rayzor_free(ptr: *mut u8, size: u64)      -- Deallocate memory
```

These are pure Rust functions using `std::alloc`, with no C dependencies. They work for both JIT (linked into process) and AOT (compiled into binary) modes.

### Size Tracking

`rayzor_free` requires a size parameter because Rust's `dealloc` needs the `Layout` (size + alignment). This is more efficient than storing the size in a header because:
- `Vec` already tracks its capacity
- `String` already tracks its length
- User objects have compile-time known sizes

### Monomorphized Collections

`Vec<T>` is specialized at compile time to avoid runtime type dispatch:

```text
Vec<Int>   -> VecI32  -> rayzor_vec_i32_push, rayzor_vec_i32_get
Vec<Float> -> VecF64  -> rayzor_vec_f64_push, rayzor_vec_f64_get
Vec<Bool>  -> VecBool -> rayzor_vec_bool_push, rayzor_vec_bool_get
Vec<T*>    -> VecPtr  -> rayzor_vec_ptr_push, rayzor_vec_ptr_get
```

---

## Dynamic Types and GC

For `Dynamic` types -- values whose concrete type is not known at compile time -- Rayzor uses runtime-managed memory. This is the **only** case where garbage collection semantics apply:

- **Dynamic variables**: Type is resolved at runtime; the compiler cannot insert deterministic `Free` instructions
- **Unknown-size objects**: Objects whose size depends on runtime values and cannot be tracked statically
- **Unannotated classes in non-strict mode**: Auto-wrapped in `Rc` (reference counting), not traditional GC

In all other cases (the vast majority of typed Haxe code), ownership-based memory management eliminates the need for GC entirely.

---

## Summary

| Analysis | Purpose | Key Algorithm | Complexity |
| --- | --- | --- | --- |
| Ownership Graph | Track ownership state, borrows, moves | Graph traversal | O(V+E) |
| Lifetime Analysis | Determine variable validity periods | Constraint generation | O(V+E) per function |
| Constraint Solver | Resolve lifetime ordering | Union-Find + Tarjan's SCC + Kahn's sort | O(V+E) |
| Global Lifetimes | Cross-function lifetime flows | Call graph SCC analysis | O(F*C) |
| Escape Analysis | Stack vs heap allocation | Def-use chain tracing | O(V+E) |
| Send/Sync | Thread safety validation | Recursive type checking | O(T*F) |
| Drop Analysis | Determine deallocation points | Last-use analysis | O(V) per function |

Where V = variables, E = edges/constraints, F = functions, C = call sites, T = types.
