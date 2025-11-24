# Generic Type Resolution Issue

## Problem Summary

Tests `thread_multiple` and `channel_basic` fail during Cranelift codegen due to type mismatches when calling generic methods that return type parameters.

### Failing Case
```haxe
var handle: Thread<Int> = Thread.spawn(() -> 42);
var result: Int = handle.join();  // Should return i32, actually returns i64
sum += result;  // ERROR: iadd.i32 expects i32, got i64
```

### Root Cause

**Type parameters (T in Thread<T>) are not resolved to concrete types during HIR->MIR lowering.**

1. `Thread<Int>.join()` should return `Int` (i32)
2. HIR expression type is `TypeParameter` (representing generic T)
3. `convert_type(TypeParameter)` returns `IrType::Any` (or now `IrType::I32` as workaround)
4. MIR function `Thread_join` returns `Ptr(U8)` (i64) to match extern `rayzor_thread_join`
5. Cranelift IR shows: `v50 = call fn4(v49)` where fn4 returns i64
6. Then: `v51 = iadd.i32 v40, v50` - ERROR: v50 is i64, expected i32

## Investigation Summary

### Attempted Fixes

1. **Auto-cast insertion in `build_call_direct`** ([builder.rs:209-221](compiler/src/ir/builder.rs#L209-L221))
   - Check if function return type matches expected type
   - Insert Cast instruction if mismatch detected
   - **Status**: Didn't trigger because both types were wrong (i64 vs Any)

2. **Generic type resolution from receiver** ([hir_to_mir.rs:1780-1803](compiler/src/ir/hir_to_mir.rs#L1780-L1803))
   - Extract generic type arguments from receiver's class type
   - For `Thread<Int>.join()`, resolve T=Int from receiver
   - **Status**: Code path not reached - join calls bypass this path

3. **TypeParameter default to I32** ([hir_to_mir.rs:3482-3487](compiler/src/ir/hir_to_mir.rs#L3482-L3487))
   - Changed `TypeParameter` conversion from `Any` to `I32`
   - **Status**: Helped for some cases, but doesn't solve root issue

4. **Thread_join signature changes**
   - Tried: Return i32 instead of i64 (type mismatch in MIR itself)
   - Tried: Insert Cast in MIR wrapper (Cast didn't appear in Cranelift IR)
   - Current: Returns i64, relies on caller to cast (doesn't work)

### Why Fixes Failed

The fundamental issue is architectural:

1. **No monomorphization**: Generic classes like `Thread<T>` aren't monomorphized to `Thread_Int`, `Thread_String`, etc.

2. **Type parameters aren't tracked through HIR**: When HIR has `Thread<Int>.join()`, the return type is stored as `TypeParameter(T)` without the binding `T=Int`.

3. **For-loop method calls bypass normal lowering**: `for (handle in handles) { handle.join() }` doesn't go through the Call expression lowering where our fixes were added.

4. **MIR doesn't support generics**: MIR functions must have concrete types, but we're trying to make `Thread_join` work for all T.

## Proper Solution

Implement **generic instantiation/monomorphization** in type checking:

### Option 1: Monomorphization (like Rust/C++)
```
Thread<Int>.join() -> Thread_Int_join() -> returns i32
Thread<String>.join() -> Thread_String_join() -> returns Ptr(U8)
```

**Pros**: Clean, type-safe, optimizable
**Cons**: Code bloat, complex implementation

### Option 2: Type Parameter Substitution
```
1. In TAST: Track generic instantiations (Thread<Int> has T=Int)
2. In HIR: When lowering method calls, substitute T with Int
3. In MIR: Use concrete types everywhere
```

**Pros**: Simpler than full monomorphization
**Cons**: Still requires substantial type system changes

### Option 3: Runtime Type Casting (workaround)
```
1. Thread_join returns i64 (ptr-sized value)
2. Runtime stores return value in i64 slot
3. Caller casts to actual type based on static knowledge
```

**Pros**: Minimal changes
**Cons**: Type unsafe, requires careful manual casts

## Required Changes

1. **Type Table Enhancement**
   - Track generic instantiations: `(ClassDef, [TypeArgs]) -> ConcreteType`
   - Store binding: `TypeParameter(id) -> ConcreteType` in context

2. **HIR Expression Types**
   - Replace `TypeParameter` in expression types with resolved concrete types
   - Resolve at type checking time, not MIR lowering time

3. **Method Call Resolution**
   - When resolving `obj.method()` where obj: `Class<T>`, lookup T's binding
   - Substitute T in method signature before generating HIR

4. **For-Loop Handling**
   - Ensure for-loop element type resolution includes generic arguments
   - Make sure method calls in loop bodies go through proper resolution

## Test Status

- ✅ thread_spawn_basic (no generic return type)
- ✅ thread_spawn_qualified (no generic return type)
- ❌ **thread_multiple** (Thread<Int>.join() returns i64 instead of i32)
- ❌ **channel_basic** (likely Channel<T> generic return issue)
- ✅ mutex_basic (Mutex<T> methods don't return T)
- ✅ arc_basic (Arc<T> clone/get tested, but may have similar latent issue)

## Error Message
```
!!! Cranelift Verifier Errors for main !!!
- inst56 (v51 = iadd.i32 v40, v50): arg 1 (v50) has type i64, expected i32
```

Where:
- v50 = result from Thread_join (i64)
- Should be i32 for Thread<Int>

## References

- [Rust Generic Monomorphization](https://doc.rust-lang.org/book/ch10-01-syntax.html#performance-of-code-using-generics)
- Type parameter resolution: [hir_to_mir.rs](compiler/src/ir/hir_to_mir.rs)
- Thread MIR wrapper: [thread.rs:121-149](compiler/src/stdlib/thread.rs#L121-L149)
- Runtime mapping: [runtime_mapping.rs:438-455](compiler/src/stdlib/runtime_mapping.rs#L438-L455)

## Next Steps

1. Design generic instantiation tracking in type table
2. Implement type parameter substitution during type checking
3. Update HIR expression types to use resolved concrete types
4. Test with Thread<Int>.join() and Channel<T> methods
5. Extend to other generic stdlib types (Arc, Mutex, etc.)
