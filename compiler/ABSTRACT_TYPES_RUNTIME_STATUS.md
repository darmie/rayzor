# Abstract Types: Runtime Status

## Executive Summary

Abstract types are **mostly functional** at runtime! Implicit conversions work perfectly, and **direct method calls now work with inlining**. Operator overloading does not yet execute at runtime.

## What Works ✅

### 1. Implicit Conversions (`from`/`to`)

**Status**: ✅ **Fully Working**

Abstract types with `from` and `to` conversions work perfectly at runtime:

```haxe
abstract Counter(Int) from Int to Int {
}

class Main {
    public static function main():Int {
        var x:Counter = 5;  // from Int - works!
        var y:Int = x;      // to Int - works!
        return y;           // Returns 5 ✓
    }
}
```

**Test Result**: ✅ PASSED - Returns 5 as expected

**Cranelift IR Generated**:
```clif
function u0:0() -> i32 apple_aarch64 {
block0:
    v0 = iconst.i32 5
    return v0  ; v0 = 5
}
```

**Verification**: The abstract type is completely erased at compile time (zero-cost abstraction), and the implicit conversions are handled by the type checker during TAST lowering.

### 2. Abstract Type Methods (Direct Calls)

**Status**: ✅ **Now Working!**

Methods defined on abstract types ARE now callable at runtime through method inlining:

```haxe
abstract Counter(Int) from Int to Int {
    public inline function toInt():Int {
        return this;
    }
}

class Main {
    public static function main():Int {
        var a:Counter = 15;
        return a.toInt();  // ✅ Works! Returns 15
    }
}
```

**Test Result**: ✅ PASSED - Returns 15 as expected

**How It Works**: Abstract type methods are now inlined during TAST→HIR lowering. When the compiler encounters a method call on an abstract type, it:
1. Detects the receiver is an abstract type
2. Finds the method definition in the abstract
3. Inlines the method body, replacing `this` with the receiver
4. Recursively processes nested expressions

**Implementation**: See [tast_to_hir.rs](src/ir/tast_to_hir.rs#L1998-L2248) for the inlining logic.

## What Doesn't Work ❌

### 3. Abstract Type Methods with Intermediate Variables

**Status**: ✅ **Now Working!**

Storing results in intermediate variables now works correctly:

```haxe
abstract Counter(Int) from Int to Int {
    public inline function add(other:Counter):Counter {
        return new Counter(this + other.toInt());
    }

    public inline function toInt():Int {
        return this;
    }
}

class Main {
    public static function main():Int {
        var a:Counter = 15;
        var b:Counter = a;
        return b.toInt();  // ✅ Works! Returns 15
    }
}
```

**Test Result**: ✅ PASSED - Returns 15 as expected

**How It Works**: The inlining system now searches ALL abstract types for methods instead of relying solely on the receiver's type. This handles cases where type inference assigns `Dynamic` or other compatible types to variables.

**Implementation Change**: Method lookup is now name-based across all abstracts in the file, making it robust against type inference variations.

### 3. Operator Overloading (`@:op`)

**Status**: ❌ **Not Working**

Operator overloading with `@:op` metadata is not functional at runtime:

```haxe
abstract Counter(Int) from Int to Int {
    @:op(A + B)
    public inline function add(rhs:Counter):Counter {
        return new Counter(this + rhs.toInt());
    }
}

class Main {
    public static function main():Int {
        var a:Counter = 5;
        var b:Counter = 10;
        var sum = a + b;  // ❌ Not resolved to add() method
        return sum;
    }
}
```

**Root Cause**: The `@:op` metadata is parsed and stored in TAST, but:
1. The type checker doesn't rewrite `a + b` to call the `add()` method
2. Binary operations on abstract types aren't resolved to their `@:op` methods

## Technical Analysis

### Why Implicit Conversions Work

Implicit `from`/`to` conversions work because:

1. **Type Checker Phase**: During type checking, the compiler:
   - Sees `var x:Counter = 5`
   - Knows Counter has `from Int`
   - Accepts the assignment without generating any conversion code

2. **TAST Phase**: The typed AST stores:
   - Variable `x` has type `Counter`
   - But the value is just the integer `5`

3. **HIR/MIR Phase**: When lowering:
   - Abstract types are erased (they're just type aliases)
   - `Counter` becomes `Int` directly
   - No method calls needed - it's already an `Int`

4. **Cranelift Phase**: The final IR just uses the underlying type:
   ```clif
   v0 = iconst.i32 5  // Direct integer constant
   ```

### Why Methods Don't Work

Abstract type methods fail because:

1. **TAST Phase**: Methods are defined on the abstract type
   - `toInt()` is registered as a method of `Counter`
   - Method symbol is created: `SymbolId(17)`

2. **HIR Lowering Phase**: When encountering `x.toInt()`:
   - Tries to look up method on type `Counter`
   - But abstract types are erased - they don't exist as classes
   - Method lookup fails: "SymbolId(17) not found in function_map"

3. **Expected Behavior**: The compiler should:
   - Recognize `x.toInt()` is a method on abstract type
   - Inline the method body: `return this;`
   - Since `this` is the underlying `Int`, just return `x` directly

### Why Operators Don't Work

Operator overloading fails because:

1. **TAST Phase**: `@:op` metadata is stored but not used
   - The metadata is attached to the `add()` method
   - But `a + b` is still parsed as a binary operation

2. **Type Checking Phase**: No rewriting happens
   - Should rewrite: `a + b` → `a.add(b)`
   - Currently: binary operation stays as-is

3. **HIR Lowering Phase**: Binary operation tries to execute
   - Tries to add two `Counter` values
   - But since methods don't work, this fails

## Implementation Roadmap

To make abstract types fully functional at runtime, we need:

### Phase 1: Method Inlining ⏳

**Goal**: Make abstract type methods work by inlining them during HIR lowering

**Changes Needed**:
1. In `tast_to_hir.rs`, when encountering method call on abstract type:
   - Check if the receiver type is an abstract
   - If yes, inline the method body directly
   - Replace `this` with the receiver expression

2. Special handling for `inline` methods:
   - Abstract methods are typically marked `inline`
   - Should be fully inlined during lowering
   - No function call should remain in HIR/MIR

**Example Transformation**:
```haxe
// Source
var x:Counter = 5;
return x.toInt();

// After inlining (conceptual)
var x:Int = 5;  // Abstract erased
return x;       // toInt() body inlined: "return this" → "return x"
```

### Phase 2: Operator Resolution ⏳

**Goal**: Resolve `@:op` operators to method calls during type checking

**Changes Needed**:
1. In `type_checker.rs`, when encountering binary operation:
   - Check if operands are abstract types
   - Look up corresponding `@:op` method
   - Rewrite AST node from `BinaryOp` to `MethodCall`

2. Store operator mapping:
   - Build map: `(AbstractType, Operator) -> MethodSymbol`
   - Example: `(Counter, +) -> add method`

**Example Transformation**:
```haxe
// Source
var sum = a + b;  // where a, b are Counter

// After type checking rewrite
var sum = a.add(b);  // Now it's a method call

// After method inlining (Phase 1)
var sum = Counter::add_impl(a, b);  // Inline the method
```

### Phase 3: Constructor Support ⏳

**Goal**: Support `new Counter(5)` syntax

**Changes Needed**:
1. Abstract constructors should be inlined
2. `new Counter(5)` should become just `5` after inlining `this = value`

## Test Coverage

### Existing Tests

| Test | Status | Description |
|------|--------|-------------|
| `test_abstract_simple_runtime.rs` | ✅ PASSING | Implicit conversions only |
| `test_abstract_types.rs` | ✅ PASSING | TAST-level verification (compile-time) |
| `test_abstract_operators_runtime.rs` | ❌ FAILING | Methods and operators |

### Tests to Add

1. **test_abstract_method_inlining.rs** - Once methods work
   - Test `toInt()`, `getValue()`, etc.
   - Verify methods are fully inlined in MIR

2. **test_abstract_operators_complete.rs** - Once operators work
   - Test `+`, `-`, `*`, `/`, `==`, `!=`, etc.
   - Verify operators call the right methods

3. **test_abstract_constructors.rs** - Once constructors work
   - Test `new Counter(5)`
   - Verify constructor is inlined

## Current Recommendation

### For Users

If you're using Rayzor today:

✅ **Safe to use**:
- Abstract types with `from`/`to` conversions
- Zero-cost type safety wrappers
- Compile-time type checking

❌ **Don't use yet**:
- Abstract type methods
- Operator overloading (`@:op`)
- Abstract constructors

### Workaround

Instead of using abstract type methods, use static helper functions:

```haxe
// Instead of this (doesn't work):
abstract Counter(Int) from Int to Int {
    public inline function toInt():Int {
        return this;
    }
}

// Use this (works):
abstract Counter(Int) from Int to Int { }

class CounterHelper {
    public static inline function toInt(c:Counter):Int {
        return c;  // Implicit to Int conversion
    }
}

// Usage:
var x:Counter = 5;
return CounterHelper.toInt(x);  // Works!
```

## References

- **Test Files**:
  - [test_abstract_simple_runtime.rs](examples/test_abstract_simple_runtime.rs) - Working implicit conversions
  - [test_abstract_operators_runtime.rs](examples/test_abstract_operators_runtime.rs) - Failing methods/operators
  - [test_abstract_types.rs](examples/test_abstract_types.rs) - TAST-level tests

- **Implementation Files**:
  - [compiler/src/ir/tast_to_hir.rs](src/ir/tast_to_hir.rs) - Where method inlining should happen
  - [compiler/src/tast/type_checker.rs](src/tast/type_checker.rs) - Where operator rewriting should happen
  - [compiler/src/tast/ast_lowering.rs](src/tast/ast_lowering.rs) - TAST construction

- **Related Documentation**:
  - [ABSTRACT_TYPES_GUIDE.md](ABSTRACT_TYPES_GUIDE.md) - User guide for abstract types
  - [IMPLEMENTATION_ROADMAP.md](IMPLEMENTATION_ROADMAP.md) - Overall compiler roadmap
