# Stdlib Parser Compatibility Plan

**Goal**: Ensure the Rayzor compiler parser can successfully parse all Haxe 4.3.7 standard library files.

**Current Status**: 0/6 e2e tests passing (down from 83% before stdlib integration)

## Root Cause Analysis

The official Haxe standard library uses language features that our parser doesn't currently support:

### 1. Conditional Compilation (CRITICAL - Blocks Everything)
**Status**: ❌ Not Implemented
**Impact**: StdTypes.hx fails to parse, blocking all compilation

**Examples Found**:
```haxe
#if jvm
@:runtimeValue
#end
@:coreType abstract Void {}

#if (java || cs || hl || cpp)
@:runtimeValue
#end
@:coreType abstract Int to Float {}
```

**Syntax to Support**:
- `#if <condition>` - Start conditional block
- `#elseif <condition>` - Else-if branch
- `#else` - Else branch
- `#end` - End conditional block

**Conditions**:
- Platform checks: `cpp`, `js`, `python`, `java`, etc.
- Logical operators: `||` (or), `&&` (and), `!` (not)
- Parentheses: `(cpp || java)`
- We compile for our own target, so most conditions should evaluate to false

### 2. Metadata on Separate Lines
**Status**: ❌ Not Implemented
**Impact**: Parser expects metadata on same line as declaration

**Current Parser Expectation**:
```haxe
@:coreType abstract Void {}  // Works
```

**Real Haxe Syntax**:
```haxe
#if jvm
@:runtimeValue
#end
@:coreType abstract Void {}  // Fails - parser sees @:coreType on wrong line
```

### 3. Null<T> Type Resolution
**Status**: ⚠️ Defined but Unresolved
**Impact**: HIR lowering fails with "UnresolvedType: Null"

**Cause**: Since StdTypes.hx fails to parse, `Null<T>` never gets registered in the type system.

**Fix**: Once parsing works, this should resolve automatically.

## Implementation Plan

### Phase 1: Conditional Compilation Preprocessor (PRIORITY 1)

**Approach**: Implement as a preprocessing pass that strips out irrelevant platform code.

**Steps**:
1. **Create preprocessor module** (`compiler/src/preprocessor.rs`)
   - Input: Raw source code string
   - Output: Processed source with conditionals resolved
   - Target platform: Define our own target identifier (e.g., `rayzor`)

2. **Implement directive parser**
   - Recognize `#if`, `#elseif`, `#else`, `#end`
   - Parse condition expressions (platforms, boolean operators)
   - Build condition evaluation tree

3. **Implement condition evaluator**
   - Set `rayzor = true`
   - Set all other platforms to `false` (cpp, js, python, etc.)
   - Evaluate boolean expressions
   - Special handling for common patterns like `#if !macro`

4. **Implement code stripping**
   - Track nesting level of conditionals
   - Remove lines for false branches
   - Preserve line numbers for error reporting (insert blank lines)

5. **Integration**
   - Call preprocessor before lexer/parser
   - Update file loading to preprocess stdlib files
   - Add tests for common conditional patterns

**Testing**:
```haxe
// Test 1: Simple condition
#if cpp
var x = 1;  // Should be removed
#end

// Test 2: Multiple conditions with OR
#if (java || cpp)
var y = 2;  // Should be removed
#end

// Test 3: Negation
#if !rayzor
var z = 3;  // Should be removed
#end

// Test 4: Nested conditionals
#if cpp
  #if java
  var a = 4;
  #end
#else
  var b = 5;  // Should be kept
#end
```

### Phase 2: Metadata on Separate Lines (PRIORITY 2)

**Current Issue**: Parser expects:
```rust
metadata class_keyword identifier
```

**Need to Support**:
```rust
metadata*
newline*
class_keyword identifier
```

**Approach**:
1. Accumulate metadata attributes in a buffer
2. When encountering class/interface/typedef/abstract/enum, attach all buffered metadata
3. Clear buffer after attaching

**Steps**:
1. Add metadata buffer to parser state
2. When parsing metadata (`@:something`), add to buffer instead of immediately requiring class
3. Modify declaration parsers to consume buffered metadata
4. Handle whitespace/newlines between metadata and declaration
5. Clear buffer after each complete declaration

**Edge Cases**:
- Multiple metadata on same line: `@:meta1 @:meta2 class Foo`
- Mixed: `@:meta1\n@:meta2 class Foo`
- With conditionals: `#if cpp\n@:meta\n#end\nclass Foo`

### Phase 3: Comprehensive Stdlib Parsing (PRIORITY 3)

**Goal**: Identify ALL remaining syntax issues by attempting to parse every stdlib file.

**Steps**:
1. **Create stdlib parser test**
   - Iterate through all `.hx` files in `compiler/haxe-std`
   - Attempt to parse each file
   - Collect and categorize errors

2. **Categorize errors**
   - Syntax not supported
   - Keywords we don't recognize
   - Constructs we haven't implemented

3. **Prioritize fixes**
   - Core types (StdTypes.hx, String.hx, Array.hx) - Highest priority
   - Common utilities (Math.hx, Std.hx, Reflect.hx) - High priority
   - Advanced features (macros, inline XML) - Lower priority

4. **Fix incrementally**
   - Fix highest priority issues first
   - Re-run parser test after each fix
   - Track progress (files parsed successfully)

### Phase 4: Validation & Testing (PRIORITY 4)

**Steps**:
1. Verify all core stdlib files parse (StdTypes, String, Array, Math, etc.)
2. Verify rayzor.concurrent namespace still parses
3. Re-run e2e tests - should go from 0% → 83%+ passing
4. Document any stdlib files we intentionally skip (platform-specific, macro-only, etc.)

## Success Criteria

✅ **Minimum Success** (Unblocks Development):
- StdTypes.hx parses successfully
- Core types (String, Array, Math, Std) parse successfully
- e2e tests return to 83%+ pass rate
- rayzor.concurrent namespace works

✅ **Full Success** (Production Ready):
- All stdlib files in `haxe/` directory parse successfully
- All stdlib files in `sys/` directory parse successfully
- All stdlib files in root parse successfully
- 95%+ of stdlib files parse without errors
- Comprehensive test suite for conditional compilation
- Documentation of unsupported features (if any)

## Known Unsupported Features (To Document)

These are advanced Haxe features we may choose not to support initially:

1. **Macros** (`@:macro`, `macro function`) - Complex, separate compilation phase
2. **Build macros** (`@:build`, `@:autoBuild`) - Compile-time code generation
3. **Native code blocks** (e.g., `untyped __cpp__()`) - Platform-specific
4. **Inline XML** - Special syntax not common in modern Haxe
5. **SWF bytecode** (`@:functionCode`) - Flash-specific

## Timeline Estimate

- **Phase 1 (Conditional Compilation)**: 2-3 days
  - Critical path, most complex
  - Requires new preprocessor module
  - Testing edge cases

- **Phase 2 (Metadata Lines)**: 1 day
  - Parser modification
  - Relatively straightforward

- **Phase 3 (Comprehensive Parsing)**: 2-3 days
  - Discovery phase
  - Iterative fixing
  - May uncover unexpected issues

- **Phase 4 (Validation)**: 1 day
  - Testing and verification
  - Documentation

**Total**: 6-8 days of focused development

## Open Questions

1. Should we define `rayzor` as the target platform, or reuse `cpp`/`hl`/another?
2. Do we need to support compile-time defines beyond platform checks?
3. How do we handle `#if macro` - do we strip macro code?
4. Should we support `#error` and `#warning` directives?

## Next Steps

1. ✅ Create this plan document
2. ⏭️ Start Phase 1: Implement conditional compilation preprocessor
3. ⏭️ Test with StdTypes.hx until it parses
4. ⏭️ Move to Phase 2 and beyond

---

**Document Status**: Draft v1.0
**Last Updated**: 2025-11-18
**Owner**: Rayzor Compiler Team
