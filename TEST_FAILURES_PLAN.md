# Test Failures Investigation & Fix Plan

**Created**: 2025-12-03
**Status**: Planning Phase
**Context**: After fixing phi node bug, investigating remaining test suite failures

## Executive Summary

Two test suites have pre-existing failures:
- `test_core_types_e2e`: 5/25 tests failing (20 passing)
- `test_vec_e2e`: 5/5 tests failing (0 passing)

All failures are **unrelated to the phi node fix** (verified by running tests before/after changes).

---

## Test Suite 1: test_core_types_e2e

### Status: 20/25 PASS, 5/25 FAIL

### Failing Tests Summary

| Test | Failure Stage | Error Type | Priority |
|------|--------------|------------|----------|
| `string_split` | Compilation | Missing field index | Medium |
| `array_index` | MirValidation | Missing extern `haxe_array_get` | High |
| `array_slice` | Compilation | Field 'length' not found | High |
| `integration_string_array` | Compilation | Class not registered | Medium |
| `integration_math_array` | Codegen | Wrong instruction type (iadd.f64) | High |

---

## Failure Analysis & Fix Plan

### 1. array_index - Missing `haxe_array_get` Extern ⭐ HIGH PRIORITY

**Error:**
```
Expected extern function 'haxe_array_get' not found in MIR.
```

**Root Cause:**
- Array element access `arr[index]` requires `haxe_array_get` extern function
- Function not declared in runtime or not mapped in stdlib

**Investigation Steps:**
1. Search for `haxe_array_get` in runtime (`runtime/src/`)
2. Check if function exists but not exported
3. Verify `compiler/src/stdlib/runtime_mapping.rs` has mapping
4. Check `compiler/haxe-std/Array.hx` for get() method

**Fix Plan:**
- [ ] Add `haxe_array_get` to `runtime/src/array.rs` or equivalent
- [ ] Signature: `pub unsafe extern "C" fn haxe_array_get(arr: *mut u8, index: i64) -> *mut u8`
- [ ] Add mapping in `runtime_mapping.rs`
- [ ] Test with simple array indexing: `var x = arr[0];`

**Estimated Complexity:** Medium (2-3 hours)

---

### 2. array_slice - Field 'length' Not Found ⭐ HIGH PRIORITY

**Error:**
```
Field 'length' (SymbolId(670)) index not found for raw pointer access
```

**Root Cause:**
- Array.slice() method tries to access `length` field directly
- Raw pointer access to struct fields not properly handled
- Field index calculation failing

**Investigation Steps:**
1. Examine `Array.slice()` implementation in stdlib
2. Check how `length` field is accessed in TAST/HIR
3. Review field access lowering in `lowering.rs`
4. Verify Array struct layout in runtime

**Fix Plan:**
- [ ] Review `compiler/src/ir/lowering.rs::lower_field_access`
- [ ] Check if Array type is registered with field indices
- [ ] Add Array struct registration if missing
- [ ] Implement proper field offset calculation for Array.length
- [ ] Alternative: Use `haxe_array_length` extern instead of direct field access

**Estimated Complexity:** Medium-High (3-4 hours)

---

### 3. integration_math_array - Wrong Instruction Type (iadd.f64) ⭐ HIGH PRIORITY

**Error:**
```
inst64 (v61 = iadd.f64 v38, v60): has an invalid controlling type f64
```

**Root Cause:**
- Using integer addition instruction `iadd` with f64 (float) type
- Type mismatch in codegen - should use `fadd.f64` for float addition
- Bug in type-to-instruction mapping

**Investigation Steps:**
1. Find where `iadd` is generated for float types
2. Check `compiler/src/codegen/cranelift_backend.rs` instruction selection
3. Review binary operation lowering for arithmetic ops
4. Identify if this is HIR→MIR or MIR→Cranelift issue

**Fix Plan:**
- [ ] Locate instruction selection logic for BinaryOp::Add
- [ ] Add type checking before selecting iadd vs fadd
- [ ] Fix pattern:
  ```rust
  match (op, operand_type) {
      (BinaryOp::Add, IrType::F32) => builder.fadd(),
      (BinaryOp::Add, IrType::F64) => builder.fadd(),
      (BinaryOp::Add, IrType::I32) => builder.iadd(),
      // ...
  }
  ```
- [ ] Test with: `var sum: Float = arr[0] + arr[1];`

**Estimated Complexity:** Low-Medium (1-2 hours)

---

### 4. string_split - Missing Field Index

**Error:**
```
Field 'InternedString(23)' (SymbolId(660)) index not found - class may not be registered
```

**Root Cause:**
- String.split() method references a field that doesn't have index mapping
- Similar to array_slice issue - class field registration problem

**Investigation Steps:**
1. Identify what field InternedString(23) represents
2. Check String class registration
3. Review String.split() stdlib implementation

**Fix Plan:**
- [ ] Debug what InternedString(23) is (add logging to print interned string)
- [ ] Register String class with proper field indices
- [ ] Implement String.split() if missing in runtime
- [ ] Add `haxe_string_split` extern function

**Estimated Complexity:** Medium (2-3 hours)

---

### 5. integration_string_array - Class Not Registered

**Error:**
```
Field 'InternedString(23)' (SymbolId(665/669)) index not found - class may not be registered
```

**Root Cause:**
- Same as string_split - missing class registration
- Affects integration test that uses both strings and arrays

**Fix Plan:**
- [ ] Will be fixed by string_split fix
- [ ] Verify Array and String classes both registered
- [ ] Test integration scenario

**Estimated Complexity:** Low (covered by string_split fix)

---

## Test Suite 2: test_vec_e2e

### Status: 0/5 PASS, 5/5 FAIL

**All tests fail with same error pattern:**

**Error:**
```
Cranelift Verifier Errors for [function_name]:
- inst0 (return): arguments of return must match function signature
```

**Example from `vec_growth::last`:**
```
function u0:0(i64, i64) -> i64 apple_aarch64 {
block0(v0: i64, v1: i64):
    return      // ❌ Missing return value!
}
```

### Root Cause Analysis

**The function signature expects `-> i64` but `return` has no arguments.**

**Possible causes:**
1. Function body lowering produces no return value
2. Return statement lowering broken for Vec methods
3. Generic type handling not instantiating return values correctly
4. HIR→MIR conversion dropping return value register

### Investigation Steps

1. **Examine test code:**
   - [ ] Read `test_vec_e2e.rs` to see test structure
   - [ ] Identify what Vec methods are being tested
   - [ ] Check if issue is generic-specific or affects all Vec methods

2. **Trace compilation pipeline:**
   - [ ] Add debug logging to Vec method lowering
   - [ ] Check TAST for return expressions
   - [ ] Verify HIR has return values
   - [ ] Check MIR return instructions
   - [ ] Examine Cranelift IR generation

3. **Compare working vs broken:**
   - [ ] Find a similar working method (e.g., in stdlib tests)
   - [ ] Compare IR output for working vs broken methods
   - [ ] Identify where return value is lost

### Fix Plan

**Phase 1: Diagnosis**
- [ ] Add `dump_tast` example for Vec test case
- [ ] Add debug logging in `hir_to_mir.rs::lower_return`
- [ ] Track IrId of return value through pipeline
- [ ] Identify exact stage where return value is dropped

**Phase 2: Fix Implementation**
Based on diagnosis, likely fixes:

**Option A: Return statement lowering**
```rust
// In hir_to_mir.rs
HirStmt::Return { value } => {
    if let Some(val) = value {
        let ret_reg = self.lower_expr(val)?;
        self.builder.build_return(Some(ret_reg));  // ✓ Pass value
    } else {
        self.builder.build_return(None);
    }
}
```

**Option B: Function body return value**
```rust
// Ensure last expression becomes return value
let last_value = /* evaluate body */;
if function.return_type != Void {
    self.builder.build_return(Some(last_value));
}
```

**Phase 3: Testing**
- [ ] Run `test_vec_e2e` after each fix
- [ ] Verify all 5 tests pass
- [ ] Check no regression in other suites

**Estimated Complexity:** Medium-High (4-6 hours)

---

## Implementation Priority & Timeline

### Phase 1: Quick Wins (1-2 days)
1. **integration_math_array** (iadd.f64 bug) - Highest ROI
   - Simple fix, immediate impact
   - Validates instruction selection logic

2. **array_index** (missing haxe_array_get)
   - Enables basic array operations
   - Foundation for other array tests

### Phase 2: Core Functionality (2-3 days)
3. **test_vec_e2e** (all 5 tests)
   - Critical for vector operations
   - Likely reveals generic/return value bugs
   - Fix helps multiple areas

4. **array_slice** (field length access)
   - Enables array manipulation
   - Tests field access lowering

### Phase 3: Polish (1-2 days)
5. **string_split** (class registration)
   - String utility function
   - Lower priority than core ops

6. **integration_string_array**
   - Covered by string_split fix
   - Integration validation

---

## Success Criteria

**Immediate Goals:**
- [ ] All `test_vec_e2e` tests pass (0/5 → 5/5)
- [ ] `test_core_types_e2e` improves to 23/25 (currently 20/25)

**Stretch Goals:**
- [ ] All `test_core_types_e2e` tests pass (25/25)
- [ ] Zero Cranelift verifier errors across all test suites

**Validation:**
- [ ] No regressions in `test_rayzor_stdlib_e2e` (maintain 8/8)
- [ ] No regressions in `test_deque_condition` (maintain 3/3)
- [ ] Clean git diff showing only intended changes

---

## Investigation Tools & Techniques

### Debug Logging Points
```rust
// Add to relevant locations:
eprintln!("DEBUG [stage_name]: var={:?}, type={:?}, value={:?}", var, ty, val);
```

**Key locations:**
- `lowering.rs::lower_return` - Check return expression lowering
- `hir_to_mir.rs::lower_function` - Verify return value propagation
- `cranelift_backend.rs::translate_return` - Check Cranelift IR generation

### Test Case Isolation
```bash
# Run single test from suite
RUST_LOG=debug cargo run --example test_vec_e2e 2>&1 | grep "vec_growth"
```

### TAST/HIR/MIR Dumping
- Create `dump_*.rs` examples for failing test cases
- Compare with working examples
- Track data transformation through pipeline

---

## Risk Assessment

**Low Risk Fixes:**
- integration_math_array (instruction selection)
- array_index (add missing extern)

**Medium Risk Fixes:**
- array_slice (field access changes)
- string_split (class registration)

**High Risk Fixes:**
- test_vec_e2e (return value handling - touches core compilation)

**Mitigation:**
- Run full test suite after each fix
- Commit fixes incrementally
- Keep changes focused and minimal

---

## Notes & Observations

### Pattern Recognition

**Common theme in failures:**
1. Missing runtime functions (haxe_array_get)
2. Field access issues (class registration, indices)
3. Type/instruction mismatches (iadd vs fadd)
4. Return value handling in generated code

**This suggests:**
- Runtime-to-compiler mapping needs audit
- Class registration system needs documentation
- Type-aware instruction selection needs review
- Return value propagation needs hardening

### Future Improvements

After fixing immediate issues, consider:
1. **Runtime Function Audit:** Document all required extern functions
2. **Class Registration Guide:** How to properly register new types
3. **Instruction Selection:** Type-safe dispatch for ops
4. **Pipeline Validation:** Add IR validation between stages

---

## Next Steps

1. **Start with integration_math_array** - quickest fix, validates approach
2. **Move to array_index** - enables array operations for other tests
3. **Tackle test_vec_e2e** - largest impact, reveals systemic issues
4. **Clean up remaining** - array_slice, string_split
5. **Document findings** - update BACKLOG.md with solutions

---

**Plan Author:** Claude Code
**Last Updated:** 2025-12-03
**Tracking:** See BACKLOG.md for implementation progress

---

## Investigation Updates (2025-12-03 Evening)

### ✅ integration_math_array - FIXED
**Commit:** 95a6594

**Root Cause:** Binary operations (Add, Sub, Mul) always used integer instructions (iadd, isub, imul) regardless of operand type.

**Fix:** Added type checking in `instruction_lowering.rs` to select fadd/fsub/fmul for float operations.

**Result:** test_core_types_e2e improved from 20/25 to 21/25 (84%)

---

### ✅ array_index - FIXED
**Commit:** 83854c4

**Root Cause:** Array index operations used GEP (Get Element Pointer) instructions for direct memory access, which doesn't work with HaxeArray's dynamic structure.

**Fix:** Modified HIR→MIR lowering to call runtime functions instead:
- **Array read** (`arr[i]`): Call `haxe_array_get_ptr()` in `lower_index_access()` (hir_to_mir.rs:7269)
  - Returns pointer to boxed element (*mut u8)
- **Array write** (`arr[i] = val`): Call `haxe_array_set()` in `lower_lvalue_write()` (hir_to_mir.rs:6854)
  - Automatically boxes primitive values (Int, Float, Bool) before storing
  - Uses box_int/box_float/box_bool helper functions

**Result:** test_core_types_e2e improved from 21/25 to 22/25 (88%)


---

## Current Status (2025-12-03 Late Evening)

### Progress Summary
- ✅ **2 of 5 high-priority issues fixed**
- **Test Score**: 22/25 PASS (88%) - up from 20/25 (80%)
- **Commits**: 95a6594 (integration_math_array), 83854c4 (array_index)

### Fixed Issues
1. ✅ integration_math_array - Float arithmetic instruction selection
2. ✅ array_index - Array access runtime function calls with automatic boxing

### Remaining Issues (3 tests)
1. **array_slice** - Field 'length' access on Array
2. **string_split** - Class registration for String methods
3. **integration_string_array** - Combination of string_split + array_slice issues

### Next Steps
Priority order for remaining fixes:
1. **array_slice** (HIGH) - Likely simple fix for Array.length field access
2. **string_split** (MEDIUM) - May require String class registration
3. **integration_string_array** (LOW) - Should be fixed by above two

Target: 25/25 PASS (100%)
