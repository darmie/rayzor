# Async and Exception Handling in Rayzor

## Overview

This document describes the current state and future plans for async/await and exception handling in the Rayzor compiler.

---

## Exception Handling

### Current Status: ✅ MIR Infrastructure Complete

#### What's Implemented

1. **HIR Support**
   - `HirStatement::Throw(expr)` - Throw exception
   - `HirStatement::TryCatch { try_block, catches, finally_block }` - Try-catch-finally

2. **MIR Instructions**
   - `Throw { exception }` - Throw an exception
   - `LandingPad { dest, ty, clauses }` - Exception landing pad
   - `Resume { exception }` - Resume exception propagation

3. **HIR → MIR Lowering** ([hir_to_mir.rs:2360-2456](../compiler/src/ir/hir_to_mir.rs#L2360-L2456))
   - Creates landing pad blocks for exception handling
   - Generates catch blocks for each catch clause
   - Implements finally block execution
   - Proper control flow for normal and exceptional paths

4. **Builder Methods** ([builder.rs:301-312](../compiler/src/ir/builder.rs#L301-L312))
   - `build_throw(exception)` - Emit throw instruction
   - `build_landing_pad(ty, clauses)` - Create landing pad

5. **Effect Analysis** ([effect_analysis.rs](../compiler/src/tast/effect_analysis.rs))
   - Tracks which functions can throw exceptions
   - Propagates throw effects through call graph
   - Stored in `FunctionEffects.can_throw`

#### What's Missing

1. **Cranelift Exception Codegen**
   - Cranelift has limited native exception support
   - Options:
     - **Option A**: Use setjmp/longjmp for simple exceptions (easier, slower)
     - **Option B**: Use Cranelift's partial unwinding support (harder, faster)
     - **Option C**: Implement custom exception tables (most work, most control)

2. **Runtime Exception Objects**
   - Need exception object representation
   - Type information for exception matching
   - Stack trace generation

3. **Exception Type Matching**
   - Catch clauses need to match exception types
   - Requires RTTI (Runtime Type Information)

### Exception Handling Architecture

```
try {
    throw new MyException("error");
} catch (e:MyException) {
    // Handle MyException
} catch (e:Exception) {
    // Handle all exceptions
} finally {
    // Always executed
}
```

**Lowered to MIR:**

```
block_try:
    %exc_obj = alloc MyException
    throw %exc_obj
    br continuation  // Not reached

block_landing_pad:
    %exc = landingpad { catch MyException, catch Exception }
    br catch_0 if typeof(%exc) == MyException
    br catch_1 if typeof(%exc) == Exception
    resume %exc  // Re-throw if no match

block_catch_0:
    // Handle MyException
    br finally_block

block_catch_1:
    // Handle Exception
    br finally_block

block_finally:
    // Finally code
    br continuation

block_continuation:
    // Rest of code
```

---

## Async/Await Support

### Current Status: ✅ Foundation Complete, Need State Machine Generation

#### What's Implemented

1. **Type System Support**
   - `AsyncKind` enum ([node.rs:205-218](../compiler/src/tast/node.rs#L205-L218)):
     ```rust
     pub enum AsyncKind {
         Sync,              // Normal synchronous function
         Async,             // Async function (returns Promise/Future)
         Generator,         // Generator function (yield)
         AsyncGenerator,    // Async generator (async + yield)
     }
     ```

2. **Effect Analysis** ([effect_analysis.rs](../compiler/src/tast/effect_analysis.rs))
   - Tracks which functions are async
   - Propagates async through call graph
   - Stored in `FunctionEffects.async_kind`

3. **Metadata Support** ([haxe_ast.rs:192-196](../../parser/src/haxe_ast.rs#L192-L196))
   - Parser supports `@:async` metadata
   - Can annotate functions, methods, closures

#### What's Missing

1. **Async/Await Syntax Parsing**
   - Need to parse `async function foo()` declarations
   - Need to parse `await expr` expressions
   - Currently can use `@:async` metadata as workaround

2. **State Machine Generation**
   - Transform async functions into state machines
   - Each `await` becomes a suspend point
   - Generate resume logic for continuation

3. **Promise/Future Runtime**
   - Need runtime to schedule async functions
   - Event loop implementation
   - Promise/Future type and operations

4. **Async Lowering**
   - Convert async functions to resumable functions in MIR
   - Generate state storage for local variables
   - Implement suspend/resume points

### Async/Await Architecture

**Source Code:**
```haxe
@:async
function fetchData(url:String):Promise<String> {
    var response = await http.get(url);
    var data = await response.json();
    return data;
}
```

**Conceptual Lowering (State Machine):**
```haxe
// Generated state machine
class FetchDataState {
    var state:Int = 0;
    var url:String;
    var response:HttpResponse;
    var data:String;

    function resume(value:Any):Promise<String> {
        switch (state) {
            case 0:
                // Initial state
                state = 1;
                return http.get(url).then(v -> {
                    this.response = v;
                    return resume(null);
                });

            case 1:
                // After first await
                state = 2;
                return response.json().then(v -> {
                    this.data = v;
                    return resume(null);
                });

            case 2:
                // Final state
                return Promise.resolve(data);
        }
    }
}
```

### Integration with Closures

Async functions can capture variables (closures + async):

```haxe
function createAsyncCounter() {
    var count = 0;

    return async function():Promise<Int> {
        await delay(100);
        count++;
        return count;
    };
}
```

This combines:
- **Closure capture analysis** (already implemented ✅)
- **Async state machine generation** (not yet implemented ❌)
- **Environment allocation** (already implemented ✅)

---

## Implementation Priorities

### Phase 1: Exception Handling (1-2 days)
1. ✅ Add `build_throw` and `build_landing_pad` methods
2. ⬜ Implement simple setjmp/longjmp-based exception handling in Cranelift
3. ⬜ Create test for throw and catch
4. ⬜ Test try-catch-finally execution

### Phase 2: Async Foundation (2-3 days)
1. ⬜ Add async/await syntax parsing
2. ⬜ Design state machine representation in MIR
3. ⬜ Implement basic state machine generation
4. ⬜ Create simple Promise/Future runtime

### Phase 3: Full Async Support (3-4 days)
1. ⬜ Complete async state machine lowering
2. ⬜ Implement async closure support
3. ⬜ Add async generator support
4. ⬜ Performance optimization

---

## Design Decisions

### Why Setjmp/Longjmp for Exceptions (Initially)?
- **Simpler**: No need for complex unwinding tables
- **Portable**: Works on all platforms
- **Fast to implement**: Can get working exceptions quickly
- **Upgrade path**: Can later replace with proper unwinding

**Downsides**:
- Slower than native unwinding
- Can't run destructors automatically
- Not ideal for performance-critical code

### Why State Machines for Async?
- **Explicit**: Clear control flow in generated code
- **Debuggable**: Each state is a concrete program point
- **Efficient**: No stack-copying overhead
- **Standard**: Used by Rust, C#, JavaScript

---

## Future Enhancements

### Exception Handling
- Zero-cost exceptions using unwinding tables
- Exception filtering and transformation
- Async exception handling (exceptions in async functions)
- Better error messages with stack traces

### Async/Await
- Async generators (async + yield)
- Async LINQ-style operations
- Parallel async execution
- Cancellation tokens
- Timeout support

---

## References

- **Cranelift Exception Handling**: https://docs.rs/cranelift-codegen/latest/cranelift_codegen/
- **Rust Async Book**: https://rust-lang.github.io/async-book/
- **C# Async State Machines**: https://devblogs.microsoft.com/dotnet/how-async-await-really-works/
- **JavaScript Promises**: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Promise

---

## Status Summary

| Feature | Status | Completion |
|---------|--------|------------|
| Exception Infrastructure (MIR) | ✅ Complete | 100% |
| Exception Codegen (Cranelift) | ❌ Not Started | 0% |
| Async Type System | ✅ Complete | 100% |
| Async Syntax Parsing | ❌ Not Started | 0% |
| Async State Machine | ❌ Not Started | 0% |
| Promise/Future Runtime | ❌ Not Started | 0% |

**Overall Exception Handling**: 50% complete (infrastructure done, codegen needed)
**Overall Async/Await**: 25% complete (types done, implementation needed)

---

Last Updated: 2025-01-13
