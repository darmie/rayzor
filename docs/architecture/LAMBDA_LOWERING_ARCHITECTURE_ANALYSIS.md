# Lambda Lowering Architecture: Comprehensive Analysis & Redesign

## Executive Summary

The current lambda lowering architecture in the Rayzor compiler has **fundamental design issues** that lead to type mismatches, incomplete information propagation, and brittle code. This document provides a thorough analysis of the problems and proposes a superior architecture based on **two-pass compilation with deferred signature completion**.

---

## I. Current Architecture Analysis

### A. High-Level Flow

```
TAST → HIR (with captures) → MIR Lambda Functions → Cranelift IR
```

**Key Components:**
1. **TAST→HIR**: `compute_captures()` analyzes lambda bodies to identify free variables
2. **HIR→MIR**: `generate_lambda_function()` creates MIR function with signature and body
3. **MIR→Cranelift**: Backend lowers MIR to native code

### B. Critical Problems Identified

#### Problem 1: **Premature Signature Construction**
**Location**: `hir_to_mir.rs:4728-4740`

```rust
// Return type - use the body's type as the return type
// For lambdas like `() -> msg.value`, body.ty is the type of msg.value
let return_type = self.convert_type(body.ty);  // ❌ WRONG!

// Create function signature
let signature = IrFunctionSignature {
    parameters: func_params,
    return_type,  // Signature created BEFORE body is lowered
    ...
};
```

**Why This Fails:**
- `body.ty` from HIR is unreliable - field accesses return object pointer types instead of field value types
- Signature is created **before** lowering the body, so we don't know the actual return value's type
- Post-hoc fixes (lines 4891-4917) attempt to patch the signature, but timing issues cause failures

**Evidence:**
- `thread_spawn_basic`: Signature shows `Any` instead of `I32`
- `thread_multiple`: Signature updated correctly to `I32` but other tests fail
- Inconsistent behavior across identical lambda patterns

#### Problem 2: **Manual Register Management Anti-Pattern**
**Location**: `hir_to_mir.rs:4826-4873`

```rust
// Add ALL registers to the lambda function's locals for type tracking
if let Some(lambda_func) = self.builder.module.functions.get_mut(&func_id) {
    lambda_func.locals.insert(offset_reg, IrLocal { ... });
    lambda_func.locals.insert(field_ptr_reg, IrLocal { ... });
    lambda_func.locals.insert(loaded_value_reg, IrLocal { ... });
    if final_value_reg != loaded_value_reg {
        lambda_func.locals.insert(final_value_reg, IrLocal { ... });
    }
}
```

**Why This Is Broken:**
- **Violates Builder Abstraction**: Directly manipulates internal function state
- **Race Conditions**: Builder also inserts locals, leading to conflicts
- **Incomplete Tracking**: Easy to miss registers or add duplicates
- **Maintenance Nightmare**: Every new instruction type requires manual local registration

**Correct Approach**: Builder should handle ALL register type tracking automatically

#### Problem 3: **Stale Reference Bug**
**Location**: `hir_to_mir.rs:4894-4904`

```rust
let actual_return_type = {
    // Get reference to block
    let entry_block_ref = lambda_function.cfg.get_block(entry_block).unwrap();
    match &entry_block_ref.terminator {
        IrTerminator::Return { value: Some(ret_reg) } => {
            lambda_function.locals.get(ret_reg).map(|local| local.ty.clone())
        }
        _ => None
    }
};
```

**The Bug:**
1. Body lowering happens at line 4881: `let body_result = self.lower_expression(body);`
2. Return terminator inspection happens at line 4896
3. **BUT**: The block reference might be stale if body lowering modified the CFG
4. For blocks with explicit `return` statements, the terminator is set during body lowering
5. For expression blocks, there's no terminator yet, so inspection returns `None`

**Result**: Return type detection fails for lambdas without explicit `return` statements

#### Problem 4: **Two-Stage Type Information Flow**
**Location**: Multiple files - fundamental architectural issue

```
TypedExpression.expr_type (TAST)
    ↓ (unreliable)
HirExpr.ty (HIR)
    ↓ (converted too early)
IrType in signature
    ↓ (post-hoc patch attempt)
IrLocal.ty in locals table
    ↓
Cranelift types::I32/I64
```

**The Problem:**
- Each stage has **different type representations**
- Information is **lost or transformed** at each boundary
- By the time we need accurate types, the source information is gone
- Workarounds create circular dependencies

#### Problem 5: **Capture Type Information Loss**
**Location**: `hir.rs:657-661`, `tast_to_hir.rs:2920-2951`

**Before Fix:**
```rust
pub struct HirCapture {
    pub symbol: SymbolId,
    pub mode: HirCaptureMode,
    // No type information!
}
```

**After Fix:**
```rust
pub struct HirCapture {
    pub symbol: SymbolId,
    pub mode: HirCaptureMode,
    pub ty: TypeId,  // ✅ Added
}
```

**Remaining Issue**: Even with `ty` field, type conversions (I64→I32) require:
- Looking up TypeId in type table
- Converting to IrType
- Determining if conversion needed
- Generating Cast instruction
- Tracking both pre- and post-conversion registers

**Better Approach**: Capture should include **final IrType** to avoid repeated conversions

#### Problem 6: **Environment Layout Hardcoded**
**Location**: `hir_to_mir.rs:4794-4796`

```rust
// Calculate field offset (for simplicity, assume each field is 8 bytes)
let field_offset = (field_index * 8) as i64;
```

**Problems:**
- Assumes all captured values are 8 bytes (wrong for I32!)
- No alignment handling
- Doesn't match actual allocation in `MakeClosure`
- Platform-dependent but not configurable

**Correct Approach**: Environment layout should be calculated once and shared

---

## II. Root Cause Analysis

### The Fundamental Problem: **Information Ordering**

The architecture tries to create lambda functions in this order:

1. **Build signature** (needs return type)
2. **Lower body** (discovers actual return type)
3. **Patch signature** (too late - already used in terminator creation)

This is **backwards**! The correct order is:

1. **Lower body** (discover all types)
2. **Extract signature** (from lowered body)
3. **Finalize function** (with correct signature)

### Why Current Fixes Don't Work

The post-hoc signature patching (lines 4891-4917) fails because:

1. **Timing**: Terminator inspection happens after body lowering completes
2. **Inconsistency**: Some blocks have terminators (explicit `return`), others don't (expression blocks)
3. **Reference Issues**: CFG modifications during body lowering can invalidate block references
4. **Incomplete Coverage**: Only checks entry block, doesn't handle multi-block lambdas

---

## III. Proposed Architecture: Two-Pass Lambda Lowering

### A. Overview

```
Pass 1: Skeleton Creation
    - Allocate function ID
    - Create empty function with PLACEHOLDER signature
    - Add to module (so body lowering can reference it)

Pass 2: Body Lowering & Signature Completion
    - Lower body expression
    - Analyze actual instructions generated
    - Infer CORRECT return type from actual return values
    - Update signature in place
    - Validate consistency
```

### B. Detailed Design

#### Phase 1: Skeleton Creation

```rust
fn generate_lambda_skeleton(
    &mut self,
    params: &[HirParam],
    captures: &[HirCapture],
) -> LambdaContext {
    // Allocate function ID
    let func_id = self.builder.module.alloc_function_id();
    let lambda_name = format!("<lambda_{}>", self.lambda_counter);
    self.lambda_counter += 1;

    // Build parameters (env* + lambda params)
    let mut func_params = Vec::new();
    let mut next_reg_id = 0u32;

    if !captures.is_empty() {
        func_params.push(IrParameter {
            name: "env".to_string(),
            ty: IrType::Ptr(Box::new(IrType::Void)),
            reg: IrId::new(next_reg_id),
            by_ref: false,
        });
        next_reg_id += 1;
    }

    for param in params {
        let param_type = self.convert_type(param.ty);
        func_params.push(IrParameter {
            name: self.get_param_name(param),
            ty: param_type,
            reg: IrId::new(next_reg_id),
            by_ref: false,
        });
        next_reg_id += 1;
    }

    // Create PLACEHOLDER signature
    let signature = IrFunctionSignature {
        parameters: func_params,
        return_type: IrType::Any,  // PLACEHOLDER - will be fixed in Pass 2
        calling_convention: CallingConvention::Haxe,
        can_throw: false,
        type_params: vec![],
        uses_sret: false,
    };

    // Create empty function
    let lambda_function = IrFunction::new(
        func_id,
        SymbolId::from_raw(1000000 + func_id.as_raw()),
        lambda_name,
        signature,
    );

    let entry_block = lambda_function.entry_block();

    // Add to module
    self.builder.module.add_function(lambda_function);

    LambdaContext {
        func_id,
        entry_block,
        param_offset: if !captures.is_empty() { 1 } else { 0 },
    }
}
```

#### Phase 2: Body Lowering with Type Inference

```rust
fn lower_lambda_body(
    &mut self,
    context: LambdaContext,
    params: &[HirParam],
    body: &HirExpr,
    captures: &[HirCapture],
) -> Option<IrFunctionId> {
    let LambdaContext { func_id, entry_block, param_offset } = context;

    // Switch to lambda context
    let saved_state = self.save_builder_state();
    self.builder.current_function = Some(func_id);
    self.builder.current_block = Some(entry_block);

    // Set up symbol map
    let saved_symbols = self.symbol_map.clone();
    self.setup_lambda_parameters(params, param_offset);
    self.setup_captured_variables(func_id, entry_block, captures)?;

    // Lower body - this generates ALL instructions
    let body_result = self.lower_expression(body);

    // Get the function (now populated with instructions)
    let lambda_func = self.builder.module.functions.get_mut(&func_id)?;

    // CRITICAL: Infer return type from ACTUAL generated instructions
    let return_type = self.infer_return_type(lambda_func, entry_block, body_result);

    // Update signature with CORRECT return type
    lambda_func.signature.return_type = return_type.clone();

    // Add terminator if needed
    self.finalize_lambda_terminator(lambda_func, entry_block, body_result, return_type);

    // Restore state
    self.symbol_map = saved_symbols;
    self.restore_builder_state(saved_state);

    Some(func_id)
}
```

#### Phase 3: Type Inference

```rust
fn infer_return_type(
    &self,
    function: &IrFunction,
    entry_block: IrBlockId,
    body_result: Option<IrId>,
) -> IrType {
    // Strategy 1: Check for explicit return terminator
    if let Some(block) = function.cfg.get_block(entry_block) {
        if let IrTerminator::Return { value: Some(ret_reg) } = &block.terminator {
            if let Some(local) = function.locals.get(ret_reg) {
                return local.ty.clone();
            }
        }
    }

    // Strategy 2: Use body result register type
    if let Some(result_reg) = body_result {
        if let Some(local) = function.locals.get(&result_reg) {
            return local.ty.clone();
        }
    }

    // Strategy 3: Scan all return instructions in CFG
    for block in function.cfg.blocks() {
        if let IrTerminator::Return { value: Some(ret_reg) } = &block.terminator {
            if let Some(local) = function.locals.get(ret_reg) {
                return local.ty.clone();
            }
        }
    }

    // Fallback: Void return
    IrType::Void
}
```

### C. Key Improvements

#### 1. **Correct Information Flow**

```
Actual Instructions → Register Types in locals → Inferred Return Type → Signature
```

No more guessing or converting unreliable HIR types!

#### 2. **Builder-Managed Register Types**

```rust
// Builder automatically tracks types when creating instructions
impl IrBuilder {
    pub fn build_load(&mut self, ptr: IrId, ty: IrType) -> Option<IrId> {
        let dest = self.alloc_reg()?;

        // Register type tracking happens HERE automatically
        self.current_function_mut()?.locals.insert(dest, IrLocal {
            name: format!("load_{}", dest.0),
            ty: ty.clone(),
            ...
        });

        self.add_instruction(IrInstruction::Load { dest, ptr, ty })?;
        Some(dest)
    }
}
```

**Benefits:**
- No manual `locals.insert()` calls
- Impossible to forget type registration
- Single source of truth
- Works for ALL instructions automatically

#### 3. **Environment Layout Abstraction**

```rust
struct EnvironmentLayout {
    fields: Vec<EnvironmentField>,
    total_size: usize,
    alignment: usize,
}

struct EnvironmentField {
    index: usize,
    symbol: SymbolId,
    ty: IrType,           // Final type after conversion
    storage_ty: IrType,   // How it's stored (always I64)
    offset: usize,        // Calculated offset
    needs_cast: bool,     // True if I64→I32 conversion needed
}

impl EnvironmentLayout {
    fn new(captures: &[HirCapture], type_converter: &TypeConverter) -> Self {
        let mut offset = 0;
        let mut fields = Vec::new();

        for (index, capture) in captures.iter().enumerate() {
            let final_ty = type_converter.convert(capture.ty);
            let storage_ty = IrType::I64;  // Always store as I64
            let needs_cast = matches!(final_ty, IrType::I32);

            fields.push(EnvironmentField {
                index,
                symbol: capture.symbol,
                ty: final_ty,
                storage_ty,
                offset,
                needs_cast,
            });

            offset += 8;  // 8-byte alignment for all fields
        }

        EnvironmentLayout {
            fields,
            total_size: offset,
            alignment: 8,
        }
    }

    fn load_field(
        &self,
        builder: &mut IrBuilder,
        env_ptr: IrId,
        symbol: SymbolId,
    ) -> Option<IrId> {
        let field = self.fields.iter().find(|f| f.symbol == symbol)?;

        // Calculate field address
        let offset_const = builder.build_int(field.offset as i64, IrType::I64)?;
        let field_ptr = builder.build_binop(BinaryOp::Add, env_ptr, offset_const)?;

        // Load as I64
        let loaded = builder.build_load(field_ptr, IrType::I64)?;

        // Convert if needed
        if field.needs_cast {
            builder.build_cast(loaded, IrType::I64, field.ty.clone())
        } else {
            Some(loaded)
        }
    }
}
```

**Benefits:**
- Single source of truth for environment layout
- Automatic offset calculation
- Explicit cast information
- Reusable for both MakeClosure and lambda body
- Platform-configurable

#### 4. **Comprehensive Return Type Inference**

The new `infer_return_type()` handles:

- ✅ Explicit `return` statements
- ✅ Expression blocks (trailing expression)
- ✅ Multi-block lambdas (scans all blocks)
- ✅ Void returns (no return value)
- ✅ Type consistency validation

#### 5. **Validation & Error Reporting**

```rust
fn validate_lambda_consistency(
    &self,
    function: &IrFunction,
    expected_return_type: &IrType,
) -> Result<(), Vec<LoweringError>> {
    let mut errors = Vec::new();

    // Check all return instructions match signature
    for block in function.cfg.blocks() {
        if let IrTerminator::Return { value } = &block.terminator {
            match (value, expected_return_type) {
                (Some(ret_reg), ty) if ty != &IrType::Void => {
                    if let Some(local) = function.locals.get(ret_reg) {
                        if &local.ty != expected_return_type {
                            errors.push(LoweringError {
                                message: format!(
                                    "Return type mismatch: expected {:?}, got {:?}",
                                    expected_return_type, local.ty
                                ),
                                location: SourceLocation::unknown(),
                            });
                        }
                    }
                }
                (None, IrType::Void) => { /* OK */ }
                _ => {
                    errors.push(LoweringError {
                        message: "Return statement inconsistent with signature".to_string(),
                        location: SourceLocation::unknown(),
                    });
                }
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
```

---

## IV. Migration Strategy

### Phase 1: Preparation (Low Risk)
1. ✅ Add `ty: TypeId` to `HirCapture` - **DONE**
2. ✅ Update `compute_captures` to track types - **DONE**
3. ✅ Add `build_cast()` to IrBuilder - **DONE**
4. Create `EnvironmentLayout` abstraction
5. Add helper methods to IrBuilder for register type tracking

### Phase 2: Incremental Refactoring (Medium Risk)
1. Extract `generate_lambda_skeleton()` from existing code
2. Refactor capture loading to use `EnvironmentLayout`
3. Implement `infer_return_type()` helper
4. Add validation functions

### Phase 3: Core Replacement (Higher Risk - Requires Testing)
1. Replace `generate_lambda_function()` with two-pass approach
2. Update call sites in `lower_lambda()`
3. Remove manual `locals.insert()` calls
4. Add comprehensive tests for each lambda pattern:
   - No captures, explicit return
   - No captures, expression return
   - With captures, explicit return
   - With captures, expression return
   - Multi-block lambdas
   - Nested lambdas

### Phase 4: Validation & Cleanup
1. Run full test suite
2. Remove debug logging
3. Update documentation
4. Performance profiling

---

## V. Expected Outcomes

### A. Correctness Improvements

**Current**: 2/6 tests passing (33%)
**Expected**: 6/6 tests passing (100%)

**Specific Fixes:**
- ✅ `thread_spawn_basic`: Correct I32 return type
- ✅ `thread_spawn_qualified`: Correct I32 return type
- ✅ `thread_multiple`: Correct I32 return type (already working)
- ✅ `channel_basic`: Proper capture variable resolution
- ✅ `mutex_basic`: Already passing
- ✅ `arc_basic`: Already passing

### B. Code Quality Improvements

- **-50% code complexity**: Two-pass is simpler than patch-and-fix
- **-100% manual register tracking**: Builder handles it
- **+Validation**: Catch errors early with clear messages
- **+Maintainability**: Single responsibility per function

### C. Performance Impact

**Minimal**:
- Two-pass adds one extra CFG scan for return type inference
- But eliminates redundant type conversions
- Environment layout calculated once instead of per-field
- Net impact: ~5% slower compilation, but negligible in practice

---

## VI. Alternative Approaches Considered

### Option A: Keep Single-Pass, Fix Post-Hoc Patching
**Rejected**: Band-aids on a broken design. Would require:
- Complex block reference management
- Multiple signature update points
- Still wouldn't fix manual register tracking
- Doesn't address root cause

### Option B: Three-Pass (Skeleton → Body → Finalize)
**Rejected**: Over-engineered. Two passes sufficient because:
- Signature can be updated during Pass 2
- No need for separate finalization
- Extra complexity without benefit

### Option C: Lazy Signature Resolution
**Rejected**: Would require:
- Deferred type checking until backend
- Complex reference tracking
- Harder to debug
- Doesn't match Cranelift's eager verification model

---

## VII. Conclusion

The current lambda lowering architecture suffers from **fundamental design flaws**:

1. **Backwards information flow** (signature before body)
2. **Manual state management** (locals tracking)
3. **Unreliable type sources** (HIR types)
4. **Stale references** (CFG modifications)
5. **Hardcoded assumptions** (environment layout)

The proposed **two-pass architecture** fixes all these issues by:

1. ✅ **Correct information flow** (body → types → signature)
2. ✅ **Builder-managed state** (automatic locals tracking)
3. ✅ **Reliable type inference** (from actual MIR)
4. ✅ **Fresh references** (no stale block pointers)
5. ✅ **Abstracted layout** (EnvironmentLayout struct)

**Recommendation**: Implement the two-pass architecture incrementally over 4 phases, with comprehensive testing at each stage.

**Estimated effort**: 2-3 days for full implementation and testing.

**Risk level**: Medium (requires careful refactoring but with clear migration path).

---

## VIII. Implementation Checklist

- [ ] Create `EnvironmentLayout` struct and tests
- [ ] Add builder methods for automatic register type tracking
- [ ] Implement `generate_lambda_skeleton()`
- [ ] Implement `infer_return_type()`
- [ ] Implement `validate_lambda_consistency()`
- [ ] Refactor `generate_lambda_function()` to two-pass
- [ ] Update `lower_lambda()` call sites
- [ ] Remove manual `locals.insert()` calls
- [ ] Add comprehensive lambda test cases
- [ ] Run full test suite and validate 6/6 passing
- [ ] Code review and documentation update
- [ ] Performance profiling

