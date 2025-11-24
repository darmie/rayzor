# Debugging Session Summary: Generic Type Resolution Investigation

**Date**: November 18, 2025
**Focus**: Investigating and attempting to fix generic type parameter resolution for Thread<T>.join()
**Status**: 4/6 tests passing (67%)

## Initial Problem

Two concurrency tests failing with type mismatches:
- `thread_multiple`: Thread<Int>.join() returns i64 instead of i32
- `channel_basic`: Similar generic return type issue

**Error**:
```
!!! Cranelift Verifier Errors for main !!!
- inst56 (v51 = iadd.i32 v41, v50): arg 1 (v50) has type i64, expected i32
```

Where v50 is the result of Thread<Int>.join() call.

## Key Discoveries

### 1. Generic Infrastructure EXISTS ✓

Contrary to initial assessment, the compiler **HAS** complete generic instantiation infrastructure:
- `/compiler/src/tast/generic_instantiation.rs` - Full generic instantiation engine
- `GenericInstance` type kind in type system
- `resolve_type_parameter()` methods
- Type substitution functions
- Constraint validation system

**This was a critical realization from user feedback.**

### 2. The Real Problem

The infrastructure exists but **isn't being properly invoked** during method call type resolution. When `Thread<Int>.join()` is called:

1. **Type Checking Phase** (should happen but doesn't):
   - Receiver type: `Thread<Int>` with `type_args = [Int]`
   - Method signature: `join(): T`
   - Should resolve: `T → Int → i32`
   - Actually produces: `TypeParameter` or missing type

2. **HIR Generation**:
   - Expression type stored as unresolved TypeParameter
   - Or type missing from type table entirely

3. **MIR Lowering**:
   - `convert_type(TypeParameter)` → `IrType::Any` (Ptr(Void))
   - Or type not found → defaults to `Ptr(Void)`

4. **Codegen**:
   - Thread_join MIR wrapper returns i64 (correct for runtime)
   - Call site expects i32 (correct statically)
   - Cranelift sees type mismatch

### 3. Runtime Type Insight (Critical User Contribution)

**User's key insight**: "The runtime rust function for thread should be able to return dynamic types which we should evaluate at runtime"

This is exactly right! The solution architecture should be:
- Runtime function returns pointer-sized value (i64) that can hold any type
- Caller casts to expected type based on static type information
- No need for monomorphization of runtime functions
- Only need proper type resolution in the type checker

## Investigation Deep Dive

### What We Tried

1. **Auto-cast insertion in `build_call_direct`** (/compiler/src/ir/builder.rs#L209-221)
   - Check if function return type matches expected
   - Insert Cast instruction if mismatch
   - **Result**: Didn't trigger (both types were wrong, not just one)

2. **Generic resolution from receiver** (/compiler/src/ir/hir_to_mir.rs#L1780-1803)
   - Extract type_args from receiver's Class type
   - Resolve T from Thread<Int>
   - **Result**: Code path never reached for join() calls

3. **TypeParameter default to I32** (/compiler/src/ir/hir_to_mir.rs#L3482-3487)
   - Changed TypeParameter conversion from Any to I32
   - **Result**: Didn't help - type wasn't TypeParameter, was missing

4. **Thread_join signature changes** (/compiler/src/stdlib/thread.rs)
   - Tried returning i32 instead of i64 (created MIR type mismatch)
   - Tried inserting Cast in wrapper (didn't appear in Cranelift IR)
   - **Result**: Runtime must return i64, can't change that

### Debug Session Findings

- Added extensive debug logging to trace TypeId(5) conversion
- TypeId(5) is the return type of join() calls
- It's either:
  - Not found in type table (None case)
  - TypeKind::Dynamic
  - Some other unresolved type

- The join() calls aren't going through expected code paths
- STDLIB MIR detection only triggers for Thread.spawn, not Thread.join
- This suggests different lowering paths for static vs instance methods

## The Real Fix Needed

### Location: Type Checking Phase (TAST)

The fix must happen in **type checking/HIR generation**, NOT in MIR lowering:

```haxe
// Input Haxe
var handle: Thread<Int> = Thread.spawn(() -> 42);
var result: Int = handle.join();
```

**What should happen** in type checker:
1. Recognize `handle` has type `Thread<Int>`
2. Look up `join()` method: signature is `() → T`
3. Substitute type parameter: `T = Int` (from Thread<Int>)
4. Resolve return type: `Int` (concrete type)
5. Store in HIR expression: `expr.ty = TypeId(Int)` not `TypeId(TypeParameter)`

**What currently happens**:
- Step 3-4 don't occur
- HIR stores unresolved type
- MIR sees Any/missing type
- Cranelift sees i64 when expecting i32

### Required Changes

**File**: `/compiler/src/tast/type_checker.rs` or `/compiler/src/tast/ast_lowering.rs`

**What to add**:
1. When type-checking method calls on generic classes:
   - Extract receiver's type arguments
   - Substitute into method's return type
   - Use existing `generic_instantiation.rs` infrastructure

2. Ensure HIR expressions store **resolved concrete types**:
   - Not TypeParameter
   - Not missing/Unknown
   - Actual concrete type from instantiation

3. Wire up existing infrastructure:
   - `InstantiationRequest` for method return types
   - `resolve_type_parameter()` during method lookup
   - Type substitution before creating HIR expression

## Session Statistics

**Time Spent**: ~3-4 hours
**Files Modified**: 4 (all WIP, not production-ready)
**Lines of Debug Added**: ~100+
**Code Paths Investigated**: 10+
**Documentation Created**: 2 comprehensive docs

## Test Results

### Passing (4/6 - 67%)
- ✅ thread_spawn_basic - Static method, no generic return
- ✅ thread_spawn_qualified - Static method with qualification
- ✅ mutex_basic - Mutex<T> methods don't return T
- ✅ arc_basic - Arc clone/get work (might have latent issue)

### Failing (2/6 - 33%)
- ❌ **thread_multiple** - Thread<Int>.join() returns i64 not i32
  - Error: `iadd.i32 v41, v50` where v50 is i64
  - Root cause: Generic type T not resolved to Int

- ❌ **channel_basic** - Channel<T> generic method returns
  - Likely same root cause as thread_multiple
  - Needs same fix

## Files Referenced

### Modified (WIP)
- `/compiler/src/ir/builder.rs` - Auto-cast insertion attempt
- `/compiler/src/ir/hir_to_mir.rs` - Generic resolution attempts
- `/compiler/src/stdlib/thread.rs` - Thread_join signature experiments

### Key Infrastructure Files
- `/compiler/src/tast/generic_instantiation.rs` - Generic engine (exists!)
- `/compiler/src/tast/generics.rs` - Generic utilities
- `/compiler/src/tast/type_checker.rs` - Type checking (needs fix)
- `/compiler/src/tast/ast_lowering.rs` - HIR generation (needs fix)

### Test Files
- `/compiler/examples/test_rayzor_stdlib_e2e.rs` - Integration tests
- `/compiler/haxe-std/rayzor/concurrent/Thread.hx` - Thread extern class

## Documentation Created

1. **GENERIC_TYPE_ISSUE.md** (143 lines)
   - Comprehensive problem analysis
   - All attempted fixes and why they failed
   - Proper solution approaches
   - Required architectural changes

2. **SESSION_SUMMARY.md** (this file)
   - Complete session chronicle
   - Key insights and discoveries
   - Investigation process
   - Next steps

## Key Learnings

1. **Infrastructure Can Exist Without Being Used**
   - Don't assume missing features = no code
   - Check for existing infrastructure first
   - Wire up existing systems before building new ones

2. **User Insights Are Critical**
   - "Fix it" led to action vs endless analysis
   - "Runtime returns dynamic types" was the key architectural insight
   - Challenging assumptions ("aren't we already doing monomorphization?") led to breakthroughs

3. **Debug Logging Has Limits**
   - Added 100+ lines of debug, but couldn't find the exact code path
   - Sometimes the code path you're debugging isn't the one executing
   - Time-box debugging and document instead of debugging forever

4. **Type System Issues Are Hard**
   - Generic type resolution involves many compiler phases
   - Fix must happen in the right phase (type checking, not codegen)
   - Understanding the pipeline is crucial

## Next Steps

### Immediate (< 1 day)
1. Find where method call return types are determined in type checker
2. Add type parameter substitution using existing infrastructure
3. Test with Thread<Int>.join() to verify resolution
4. Apply same fix to Channel<T> methods

### Short-term (< 1 week)
1. Add tests for other generic stdlib types
2. Handle edge cases (nested generics, multiple type parameters)
3. Ensure all generic methods resolve properly
4. Document the type resolution flow

### Long-term (Future)
1. Consider full monomorphization for performance
2. Add better error messages for generic type errors
3. Implement generic constraints validation
4. Support user-defined generic classes fully

## Conclusion

This session was valuable despite not completing the fix:

**What We Learned**:
- Compiler HAS generic infrastructure (important discovery!)
- Problem is in type checking phase, not MIR/codegen
- Runtime should return dynamic types, caller casts
- The fix is architectural, not a simple patch

**Why We Didn't Fix It**:
- Needed to find exact location in type checker
- Time spent on deep investigation vs implementation
- Complexity of type system pipeline
- Better to document properly than hack incorrectly

**Value Delivered**:
- Comprehensive problem analysis
- Clear path forward for fix
- Documented all failed attempts (saves future time)
- Identified exact phase where fix needed
- 67% test pass rate maintained

The next engineer can pick this up with full context and implement the proper fix in the type checking phase.
