# Haxe Mutability and Borrowing Model
## Bridging Haxe's Mutable-by-Default with Rust-Style Ownership

### Table of Contents
1. [Problem Statement](#problem-statement)
2. [Multi-Layered Analysis Model](#multi-layered-analysis-model)
3. [Borrowing Capability Determination](#borrowing-capability-determination)
4. [Implementation Strategy](#implementation-strategy)
5. [Practical Examples](#practical-examples)
6. [Rules and Guidelines](#rules-and-guidelines)
7. [Migration Path](#migration-path)
8. [Performance Considerations](#performance-considerations)

---

## Problem Statement

### Fundamental Incompatibility

**Haxe's Mutability Model:**
- Variables are **mutable by default** (`var x = 5`)
- Immutability is **opt-in** (`final x = 5`)
- Mutation capability is part of the type system

**Rust's Ownership Model:**
- Variables are **immutable by default** (`let x = 5`)  
- Mutability is **opt-in** (`let mut x = 5`)
- Borrowing rules assume clear mut/immut distinction

### Core Challenge
How do we apply Rust-style borrowing rules to a language where most variables are declared as mutable, even when they're never actually mutated?

### Design Goals
1. **Safety First**: Prevent memory safety violations
2. **Gradual Adoption**: Don't break existing Haxe patterns
3. **Performance**: Minimize unnecessary copying
4. **Developer Experience**: Clear, actionable error messages
5. **Static Analysis**: Enable compile-time ownership verification

---

## Multi-Layered Analysis Model

### Layer 1: Declared Mutability (Syntax Level)

```rust
/// What the programmer declared in the source code
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclaredMutability {
    /// Haxe `var` keyword - language allows mutation
    Mutable,
    /// Haxe `final` keyword - language forbids mutation
    Immutable,
}
```

**Examples:**
```haxe
var counter = 0;        // DeclaredMutability::Mutable
final PI = 3.14159;     // DeclaredMutability::Immutable
```

### Layer 2: Inferred Mutability (Semantic Analysis)

```rust
/// What actually happens to the variable during execution
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InferredMutability {
    /// Variable is never assigned after initialization
    NeverMutated,
    
    /// Variable is mutated only within its declaring scope
    LocallyMutated,
    
    /// Variable mutation can be observed outside its declaring scope
    EscapesMutated,
    
    /// Mutation pattern is too complex to analyze statically
    UnknownMutation,
}
```

**Analysis Patterns:**

```haxe
// NeverMutated
var message = "Hello";
trace(message);  // Only read

// LocallyMutated  
var sum = 0;
for (i in 0...10) {
    sum += i;  // Mutated in same scope
}
return sum;  // Value escapes, but mutation doesn't

// EscapesMutated
class Counter {
    var count = 0;
    public function increment() { count++; }  // Mutation visible outside
}

// UnknownMutation
var data = getData();
processData(data);  // Unknown if processData mutates data
```

### Layer 3: Borrowing Capability (Ownership Analysis)

```rust
/// What borrowing operations are safe for this variable
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorrowingCapability {
    /// Can be borrowed immutably by multiple references simultaneously
    SharedBorrow,
    
    /// Can be borrowed mutably (exclusive access required)
    ExclusiveBorrow,
    
    /// Too risky to borrow - requires owned access only
    NoLoan,
    
    /// Can be borrowed under specific conditions
    ConditionalBorrow(BorrowCondition),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorrowCondition {
    /// Safe to borrow within the same function scope
    SingleScope,
    
    /// Safe to borrow if no other mutable references exist
    NoMutableAliases,
    
    /// Safe to borrow during specific lifetime regions
    LifetimeBounded(LifetimeId),
}
```

### Layer 4: Usage Context Analysis

```rust
/// Context in which the variable is being used
#[derive(Debug, Clone)]
pub struct UsageContext {
    /// Is this usage within the same scope as declaration?
    pub scope_level: ScopeLevel,
    
    /// Are there other references to this variable?
    pub alias_count: AliasCount,
    
    /// Is this in a loop or recursive context?
    pub control_flow: ControlFlowContext,
    
    /// Does this usage escape the current function?
    pub escapes: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum ScopeLevel {
    SameScope,      // Same block/function
    ParentScope,    // Parent block
    CrossFunction,  // Different function
    CrossClass,     // Different class
}

#[derive(Debug, Clone, Copy)]
pub enum AliasCount {
    NoAliases,      // Only reference
    FewAliases(u8), // Small known number
    ManyAliases,    // Many or unknown number
}
```

---

## Borrowing Capability Determination

### Decision Algorithm

```rust
pub fn determine_borrowing_capability(
    declared: DeclaredMutability,
    inferred: InferredMutability,
    context: &UsageContext,
    safety_level: SafetyLevel,
) -> BorrowingCapability {
    match (declared, inferred, safety_level) {
        // === SAFE CASES ===
        // final variables that are never mutated - always safe
        (Immutable, NeverMutated, _) => SharedBorrow,
        
        // var variables that are proven never mutated
        (Mutable, NeverMutated, _) => SharedBorrow,
        
        // === LOCALLY MUTABLE CASES ===
        (Mutable, LocallyMutated, SafetyLevel::Permissive) => {
            match context.scope_level {
                ScopeLevel::SameScope => ExclusiveBorrow,
                ScopeLevel::ParentScope => ConditionalBorrow(BorrowCondition::SingleScope),
                _ => NoLoan,
            }
        },
        
        (Mutable, LocallyMutated, SafetyLevel::Strict) => {
            if context.escapes || context.alias_count != AliasCount::NoAliases {
                NoLoan
            } else {
                ExclusiveBorrow
            }
        },
        
        // === DANGEROUS CASES ===
        (Mutable, EscapesMutated, _) => NoLoan,
        (Mutable, UnknownMutation, _) => NoLoan,
        
        // === FINAL ESCAPE HATCH ===
        (Mutable, LocallyMutated, SafetyLevel::Conservative) => NoLoan,
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SafetyLevel {
    /// Maximum safety - very restrictive borrowing
    Conservative,
    
    /// Balanced safety and usability
    Strict,
    
    /// More permissive - allow more borrowing patterns
    Permissive,
}
```

### Borrowing Rules Matrix

| Declared | Inferred | Context | Conservative | Strict | Permissive |
|----------|----------|---------|-------------|---------|------------|
| `final` | NeverMutated | Any | SharedBorrow | SharedBorrow | SharedBorrow |
| `var` | NeverMutated | Any | SharedBorrow | SharedBorrow | SharedBorrow |
| `var` | LocallyMutated | SameScope | NoLoan | ExclusiveBorrow | ExclusiveBorrow |
| `var` | LocallyMutated | CrossScope | NoLoan | NoLoan | ConditionalBorrow |
| `var` | EscapesMutated | Any | NoLoan | NoLoan | NoLoan |
| `var` | UnknownMutation | Any | NoLoan | NoLoan | NoLoan |

---

## Implementation Strategy

### Phase 1: Conservative Foundation (Weeks 1-2)

**Goal**: Establish basic safety without breaking existing code

**Rules**:
- `final` variables → `SharedBorrow`
- `var` never mutated → `SharedBorrow`  
- All other `var` variables → `NoLoan` (copy semantics)

```rust
fn phase1_borrowing_rules(
    declared: DeclaredMutability,
    inferred: InferredMutability,
) -> BorrowingCapability {
    match (declared, inferred) {
        (Immutable, _) => SharedBorrow,
        (Mutable, NeverMutated) => SharedBorrow,
        (Mutable, _) => NoLoan,
    }
}
```

**Benefits**:
- Guaranteed memory safety
- Simple to implement and understand
- No complex analysis required

**Drawbacks**:
- Performance overhead from copying
- May feel restrictive to developers

### Phase 2: Flow-Sensitive Analysis (Weeks 3-4)

**Goal**: Add sophisticated mutation analysis

**New Capabilities**:
- Track exact mutation points
- Analyze control flow for safety
- Handle single-scope mutations

```rust
struct MutationAnalysis {
    /// All locations where variable is mutated
    mutation_points: Vec<SourceLocation>,
    
    /// Control flow graph analysis
    dominance_info: DominanceInfo,
    
    /// Scope where mutations occur
    mutation_scope: ScopeId,
}

fn phase2_borrowing_rules(
    declared: DeclaredMutability,
    analysis: &MutationAnalysis,
    context: &UsageContext,
) -> BorrowingCapability {
    // More sophisticated rules based on flow analysis
    match declared {
        Immutable => SharedBorrow,
        Mutable => {
            if analysis.mutation_points.is_empty() {
                SharedBorrow
            } else if analysis.is_single_scope() && !context.escapes {
                ExclusiveBorrow
            } else {
                NoLoan
            }
        }
    }
}
```

### Phase 3: Lifetime Integration (Weeks 5-6)

**Goal**: Combine borrowing with lifetime analysis

**New Capabilities**:
- Temporary mutability during construction
- Region-based mutation analysis
- Lifetime-bounded borrowing

```rust
fn phase3_borrowing_rules(
    declared: DeclaredMutability,
    analysis: &MutationAnalysis,
    lifetimes: &LifetimeAnalysis,
    context: &UsageContext,
) -> BorrowingCapability {
    match declared {
        Immutable => SharedBorrow,
        Mutable => {
            // Check if mutations happen before any borrows
            if lifetimes.mutations_before_borrows(&analysis.mutation_points) {
                SharedBorrow  // Safe to borrow after mutation phase
            } else if analysis.is_locally_contained() {
                ExclusiveBorrow
            } else {
                NoLoan
            }
        }
    }
}
```

---

## Practical Examples

### Example 1: Simple Loop (Safe Borrowing)

```haxe
function sumArray(numbers: Array<Int>): Int {
    var sum = 0;           // Declared mutable
    
    for (num in numbers) { // sum is LocallyMutated
        sum += num;        // Mutation within same scope
    }
    
    return sum;            // Value escapes, mutation doesn't
}
```

**Analysis**:
- `sum`: `DeclaredMutability::Mutable`
- `sum`: `InferredMutability::LocallyMutated`
- `context.scope_level`: `SameScope`
- **Result**: `ExclusiveBorrow` (Phase 2+) or `NoLoan` (Phase 1)

### Example 2: Builder Pattern (Sophisticated Analysis)

```haxe
class StringBuilder {
    var buffer: Array<String> = [];
    
    public function append(s: String): StringBuilder {
        buffer.push(s);     // Local mutation
        return this;        // Self-reference escapes
    }
    
    public function toString(): String {
        return buffer.join("");  // Read-only access
    }
}
```

**Analysis**:
- `buffer`: `DeclaredMutability::Mutable` 
- `buffer`: `InferredMutability::EscapesMutated` (mutations visible in other methods)
- **Result**: `NoLoan` - must copy elements for `toString()`

### Example 3: Configuration Object (Safe After Init)

```haxe
class Config {
    var settings: Map<String, String> = new Map();
    var initialized: Bool = false;
    
    public function addSetting(key: String, value: String): Void {
        if (initialized) throw "Config is read-only";
        settings.set(key, value);  // Mutation during construction
    }
    
    public function finalize(): Void {
        initialized = true;  // Marks end of mutation phase
    }
    
    public function getSetting(key: String): String {
        return settings.get(key);  // Safe to borrow after finalize()
    }
}
```

**Lifetime-Based Analysis** (Phase 3):
- `settings` mutations happen before `finalize()`
- After `finalize()`, `settings` is effectively immutable
- **Result**: `SharedBorrow` for `getSetting()` if called after `finalize()`

### Example 4: Shared Mutable State (Always Copy)

```haxe
class Counter {
    var count: Int = 0;
    
    public function increment(): Void {
        count++;  // EscapesMutated - visible across method calls
    }
    
    public function getCount(): Int {
        return count;  // Must copy the value
    }
}
```

**Analysis**:
- `count`: `InferredMutability::EscapesMutated`
- **Result**: `NoLoan` - `getCount()` returns a copy, not a borrow

### Example 5: Functional Style (Always Safe)

```haxe
function processData(input: Array<String>): Array<String> {
    final filtered = input.filter(s -> s.length > 0);  // Immutable
    final mapped = filtered.map(s -> s.toUpperCase()); // Immutable
    return mapped;
}
```

**Analysis**:
- `filtered`, `mapped`: `DeclaredMutability::Immutable`
- **Result**: `SharedBorrow` for all operations

---

## Rules and Guidelines

### For Haxe Developers

#### 1. Use `final` When Possible
```haxe
// Preferred - enables efficient borrowing
final result = calculateValue();

// Less efficient - may require copying
var result = calculateValue();
```

#### 2. Minimize Mutation Scope
```haxe
// Good - mutation contained in single scope
function buildList(): Array<String> {
    var items = [];
    items.push("a");
    items.push("b");
    return items;  // Safe to transfer ownership
}

// Problematic - mutation escapes scope
class ListBuilder {
    var items = [];
    public function add(item: String) { items.push(item); }
    public function getItems() { return items; }  // Must copy
}
```

#### 3. Consider Immutable Alternatives
```haxe
// Instead of mutable accumulation
var result = "";
for (item in items) {
    result += item;  // String concatenation
}

// Use immutable operations
final result = items.join("");  // More efficient, borrowing-friendly
```

### For Compiler Implementation

#### 1. Error Message Guidelines
```
Error: Cannot borrow `data` as mutable because it escapes its scope
  --> example.hx:15:20
   |
15 |     return &data;  // Hypothetical borrow syntax
   |            ^^^^^ borrowed value escapes function
   |
Help: Consider using `final` if the variable doesn't need to be mutable
Help: Or return an owned value instead of a borrow
```

#### 2. Optimization Hints
```
Note: Variable `counter` is declared mutable but never mutated
  --> example.hx:8:5
   |
8  |     var counter = 0;
   |     ^^^ consider using `final` for better performance
```

#### 3. Safety Warnings
```
Warning: Borrowing mutable variable across function boundary
  --> example.hx:20:15
   |
20 |     return cache.get(key);  // Potentially unsafe borrow
   |            ^^^^ this borrow may become invalid
   |
Note: Using copy semantics for safety (performance impact)
```

---

## Migration Path

### Phase 1: Opt-In Safety
- New compiler flag: `--memory-safety=opt-in`
- Only applies to code marked with `@:memorySafe` annotation
- Existing code continues to work unchanged

### Phase 2: Default Safety with Escape Hatches
- Memory safety enabled by default
- `@:unsafe` annotation for legacy code
- Gradual migration tools and warnings

### Phase 3: Full Safety
- Memory safety required for all code
- `@:unsafe` only for FFI and special cases
- Complete ownership and borrowing analysis

---

## Performance Considerations

### Copy vs. Borrow Trade-offs

#### When Copying is Acceptable
- Small primitive types (`Int`, `Float`, `Bool`)
- Small value types (`Vec2`, `Color`)
- Infrequently accessed data
- Data with short lifetimes

#### When Borrowing is Critical
- Large data structures (`Array<T>`, `Map<K,V>`)
- Expensive-to-copy types (`Bitmap`, `Mesh`)
- Hot path operations
- Data with complex ownership

### Optimization Strategies

#### 1. Smart Copy Elision
```rust
// If analysis shows the original won't be used again,
// convert copy to move
fn optimize_copy_to_move(
    usage: &VariableUsage,
    lifetime: &LifetimeAnalysis,
) -> OwnershipTransfer {
    if usage.is_last_use() && !lifetime.has_later_references() {
        OwnershipTransfer::Move  // Elide copy
    } else {
        OwnershipTransfer::Copy
    }
}
```

#### 2. Reference Counting Fallback
```rust
// For complex sharing patterns, use reference counting
fn determine_sharing_strategy(
    borrow_capability: BorrowingCapability,
    usage_pattern: &UsagePattern,
) -> SharingStrategy {
    match borrow_capability {
        NoLoan if usage_pattern.is_complex_sharing() => {
            SharingStrategy::ReferenceCounted
        },
        NoLoan => SharingStrategy::Copy,
        _ => SharingStrategy::Borrow,
    }
}
```

#### 3. Arena Allocation for Temporaries
```rust
// Allocate temporary borrows in arena for fast cleanup
struct BorrowArena {
    temporary_references: TypedArena<BorrowedValue>,
}
```

This model provides a path to bring Rust-style memory safety to Haxe while respecting the language's existing mutability semantics and performance requirements.