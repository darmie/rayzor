# What's Next to Implement

## Current Status Summary

### ✅ COMPLETED
1. **Abstract Type Infrastructure** - Complete
   - Symbol creation, AST lowering, type checking
   - Implicit conversions (`from`/`to`)
   - Method calls and inlining

2. **Binary Operator Overloading** - Complete
   - Metadata extraction (`@:op(A + B)`)
   - Pattern parsing and validation
   - Runtime execution with zero-cost inlining
   - All 11 binary operators working

3. **Unary Operator Overloading** - Complete
   - Metadata extraction and pattern parsing
   - Prefix and postfix operators
   - Zero-cost inlining
   - All unary operators working

4. **Array Access Operator Overloading** - Complete
   - `@:arrayAccess` metadata extraction
   - Array read (get) operations
   - Array write (set) operations
   - Zero-cost inlining for both get and set

5. **Real-World Testing** - Complete
   - Time units (Seconds/Milliseconds)
   - Branded IDs (UserId/OrderId)
   - Validated strings (Email)
   - Abstracts with and without `from`/`to`

### ❌ NOT IMPLEMENTED
None! All Priority 1 features complete! 🎉

---

## Next Priorities (Ranked by Impact & Difficulty)

### 🥇 Priority 1: ✅ COMPLETE - Constructor Expression Bug FIXED!

**Status**: ✅ Fixed and tested
**Difficulty**: ⭐⭐ Medium (4 hours actual)
**Impact**: ⭐⭐⭐⭐ High - Enables complex abstract methods

#### The Problem (SOLVED)
Methods with `new Counter(...)` were returning pointers instead of values:

```haxe
abstract Counter(Int) {
    @:op(A + B)
    public inline function add(rhs:Counter):Counter {
        return new Counter(this + rhs.toInt());  // ❌ Returns pointer!
    }
}
```

**Error**:
```
Cranelift Verifier Errors:
- return v2: result 0 has type i64, must match function signature of i32
```

#### Investigation Needed
1. Check MIR lowering of `New` expressions
2. Verify type propagation for constructor results
3. Ensure constructors return values, not pointers

**Files to Investigate**:
- `compiler/src/ir/hir_to_mir.rs` - MIR lowering
- `compiler/src/ir/tast_to_hir.rs` - New expression handling
- `compiler/examples/test_abstract_operators_runtime.rs` - Failing test

**Estimated Time**: 2-3 hours

---

### 🥈 Priority 2: Error Messages & Diagnostics (POLISH)

**Status**: Basic implementation
**Difficulty**: ⭐⭐ Medium (2-3 hours)
**Impact**: ⭐⭐ Medium - Better developer experience

#### What to Implement

1. **Better Operator Error Messages**
   ```
   Error: No operator overload for '+' on type Counter
   Hint: Add @:op(A + B) metadata to a method
   Available operators: -, *, /
   ```

2. **Type Mismatch Warnings**
   ```
   Warning: Operator @:op(A + B) returns Int but Counter expected
   Help: Consider wrapping result in Counter constructor
   ```

3. **Invalid Pattern Warnings** (already partially done)
   ```
   Warning: Invalid operator pattern '@:op(A + B + C)'
   Expected: @:op(A + B) for binary operators
   ```

**Files to Modify**:
- `compiler/src/ir/tast_to_hir.rs` (improve warnings)
- `compiler/src/tast/type_checker.rs` (add hints)

**Estimated Time**: 2-3 hours

---

## Recommended Implementation Order

### Week 1: Complete Operator Overloading
1. **Day 1**: Implement Unary Operators (1 hour)
2. **Day 2**: Fix Constructor Bug (2-3 hours)
3. **Day 3**: Test & Document (1-2 hours)

### Week 2: Polish & Additional Features
4. **Day 4-5**: Array Access Operators (2 hours)
5. **Day 6**: Better Error Messages (2-3 hours)
6. **Day 7**: Comprehensive Testing (2 hours)

---

## Long-Term Enhancements (Future)

### Advanced Operator Features
1. **Compound Assignment** (`+=`, `-=`, etc.)
   - Automatically generate from `@:op(A + B)`
   - Rewrite `a += b` to `a = a + b`

2. **Commutative Operators**
   - Support `@:commutative` metadata
   - Allow `@:op(A + B)` to handle `b + a` too

3. **Operator Chaining**
   - Optimize `a + b + c` to single call
   - Avoid intermediate objects

### Type System Enhancements
1. **Multi-Statement Method Inlining**
   - Currently only handles single `return` statements
   - Support full method bodies with blocks

2. **Generic Abstract Types**
   - Support `abstract Wrapper<T>(T)`
   - Type parameter inference for operators

3. **Conditional Inlining**
   - Only inline methods below complexity threshold
   - Generate real method calls for complex operators

---

## Success Criteria

### Minimum Viable Product (MVP)
- ✅ Binary operators working
- ✅ Unary operators working
- ✅ Array access operators working
- ⬜ Constructor bug fixed
- ⬜ Basic error messages

### Production Ready
- ⬜ Comprehensive test suite
- ⬜ Good error messages
- ⬜ Performance benchmarks

### Future Proof
- ⬜ Compound assignments
- ⬜ Generic abstracts
- ⬜ Advanced optimizations

---

## Current Test Coverage

### ✅ Working Tests
1. `test_operator_execution.rs` - Binary operators ✅
2. `test_operator_metadata_extraction.rs` - Metadata ✅
3. `test_simple_operator_overload.rs` - Detection ✅
4. `test_operator_any_identifiers.rs` - Custom identifiers ✅
5. `test_abstract_intermediate.rs` - Intermediate variables ✅
6. `test_abstract_real_world.rs` - Real-world patterns ✅
7. `test_unary_operators_execution.rs` - Unary operators ✅
8. `test_array_access_execution.rs` - Array access operators ✅

### ⚠️ Partially Working
1. `test_abstract_operators_runtime.rs` - Constructor bug

### ❌ Not Created Yet
1. `test_operator_errors.rs` - Error message quality

---

## Conclusion

**Next Immediate Step**: Fix Constructor Expression Bug (2-3 hours, high impact)

This will:
1. Enable complex operator methods with `new` expressions
2. Complete operator overloading feature set
3. Allow abstracts to return wrapped values from operators
4. Unlock advanced abstract type patterns

**Current Status**: All three major operator overloading features are now complete!
- ✅ Binary operators (`@:op(A + B)`)
- ✅ Unary operators (`@:op(-A)`, `@:op(++A)`)
- ✅ Array access (`@:arrayAccess` get/set)

**Remaining Work**:
1. Fix constructor bug to enable `return new Abstract(value)` in operators
2. Add comprehensive error messages and diagnostics
3. Build full test suite for edge cases
4. Optimize performance and measure benchmarks

---

*Last Updated: 2025-11-14*
*Status: Binary/Unary/Array Access Operators Complete, Constructor Bug Next*
