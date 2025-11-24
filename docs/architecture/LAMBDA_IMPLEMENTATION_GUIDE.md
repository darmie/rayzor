# Lambda Lowering: Two-Pass Implementation Guide

## Quick Reference: What to Implement

This guide provides **concrete, copy-paste-ready code** for implementing the two-pass lambda lowering architecture.

---

## Part 1: Environment Layout Abstraction

### File: `compiler/src/ir/environment_layout.rs` (NEW FILE)

```rust
use crate::ir::{IrType, IrId, BinaryOp};
use crate::tast::SymbolId;

/// Represents a single captured variable in the environment
#[derive(Debug, Clone)]
pub struct EnvironmentField {
    /// Index in the captures array
    pub index: usize,
    /// Symbol of the captured variable
    pub symbol: SymbolId,
    /// Final type after conversion (what the lambda code expects)
    pub ty: IrType,
    /// Storage type in environment (how it's actually stored)
    pub storage_ty: IrType,
    /// Byte offset in environment struct
    pub offset: usize,
    /// Whether a cast is needed from storage_ty to ty
    pub needs_cast: bool,
}

/// Describes the layout of a closure environment
#[derive(Debug, Clone)]
pub struct EnvironmentLayout {
    /// Fields in the environment, in order
    pub fields: Vec<EnvironmentField>,
    /// Total size in bytes
    pub total_size: usize,
    /// Alignment requirement
    pub alignment: usize,
}

impl EnvironmentLayout {
    /// Create a new environment layout from captures
    pub fn new<F>(captures: &[HirCapture], type_converter: F) -> Self
    where
        F: Fn(TypeId) -> IrType,
    {
        let mut offset = 0;
        let mut fields = Vec::with_capacity(captures.len());

        for (index, capture) in captures.iter().enumerate() {
            let final_ty = type_converter(capture.ty);
            let storage_ty = IrType::I64;  // Always store as I64 (pointer-sized)

            // Determine if cast is needed
            let needs_cast = match final_ty {
                IrType::I32 => true,   // I64 → I32 cast needed
                IrType::I64 => false,  // Already I64
                _ => false,            // Other types stored as-is
            };

            fields.push(EnvironmentField {
                index,
                symbol: capture.symbol,
                ty: final_ty,
                storage_ty,
                offset,
                needs_cast,
            });

            // Always use 8-byte alignment for simplicity
            offset += 8;
        }

        EnvironmentLayout {
            fields,
            total_size: offset,
            alignment: 8,
        }
    }

    /// Find field by symbol
    pub fn find_field(&self, symbol: SymbolId) -> Option<&EnvironmentField> {
        self.fields.iter().find(|f| f.symbol == symbol)
    }

    /// Generate code to load a captured variable from the environment
    ///
    /// Returns the register containing the final value (after cast if needed)
    pub fn load_field(
        &self,
        builder: &mut IrBuilder,
        env_ptr: IrId,
        symbol: SymbolId,
    ) -> Option<IrId> {
        let field = self.find_field(symbol)?;

        // Calculate field address: env_ptr + offset
        let offset_const = builder.build_int(field.offset as i64, IrType::I64)?;
        let field_ptr = builder.build_binop(BinaryOp::Add, env_ptr, offset_const)?;

        // Load the value (always as I64 from storage)
        let loaded = builder.build_load(field_ptr, field.storage_ty.clone())?;

        // Cast if needed
        if field.needs_cast {
            builder.build_cast(loaded, field.storage_ty.clone(), field.ty.clone())
        } else {
            Some(loaded)
        }
    }
}

use crate::ir::HirCapture;
use crate::tast::TypeId;
use crate::ir::IrBuilder;
```

---

## Part 2: Enhanced IrBuilder Methods

### File: `compiler/src/ir/builder.rs` (ADD METHODS)

```rust
impl IrBuilder {
    /// Register a local variable with type tracking
    /// This should be called by ALL instruction builders that create registers
    fn register_local(&mut self, reg: IrId, ty: IrType) -> Option<()> {
        let func = self.current_function_mut()?;

        func.locals.insert(reg, IrLocal {
            name: format!("r{}", reg.0),
            ty,
            mutable: false,
            source_location: IrSourceLocation::unknown(),
            allocation: AllocationHint::Register,
        });

        Some(())
    }

    /// Build a load instruction with automatic type registration
    pub fn build_load(&mut self, ptr: IrId, ty: IrType) -> Option<IrId> {
        let dest = self.alloc_reg()?;

        // Register type BEFORE adding instruction
        self.register_local(dest, ty.clone())?;

        self.add_instruction(IrInstruction::Load { dest, ptr, ty })?;
        Some(dest)
    }

    /// Build a cast instruction with automatic type registration
    pub fn build_cast(&mut self, src: IrId, from_ty: IrType, to_ty: IrType) -> Option<IrId> {
        let dest = self.alloc_reg()?;

        // Register type BEFORE adding instruction
        self.register_local(dest, to_ty.clone())?;

        self.add_instruction(IrInstruction::Cast { dest, src, from_ty, to_ty })?;
        Some(dest)
    }

    /// Build a binary operation with automatic type registration
    pub fn build_binop(&mut self, op: BinaryOp, left: IrId, right: IrId) -> Option<IrId> {
        let dest = self.alloc_reg()?;

        // Infer result type from operation and operands
        let result_ty = self.infer_binop_type(op, left, right)?;
        self.register_local(dest, result_ty)?;

        self.add_instruction(IrInstruction::BinOp { dest, op, left, right })?;
        Some(dest)
    }

    /// Infer the result type of a binary operation
    fn infer_binop_type(&self, op: BinaryOp, left: IrId, right: IrId) -> Option<IrType> {
        let func = self.current_function()?;
        let left_ty = &func.locals.get(&left)?.ty;
        let right_ty = &func.locals.get(&right)?.ty;

        // Type inference rules
        Some(match op {
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => {
                // Arithmetic: result type matches operands (prefer I64 if mixed)
                if left_ty == right_ty {
                    left_ty.clone()
                } else {
                    IrType::I64
                }
            }
            BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                // Comparison: result is always Bool
                IrType::Bool
            }
            BinaryOp::And | BinaryOp::Or => {
                // Logical: result is Bool
                IrType::Bool
            }
            _ => IrType::I64, // Default
        })
    }
}
```

---

## Part 3: Lambda Context Structure

### File: `compiler/src/ir/hir_to_mir.rs` (ADD STRUCT)

```rust
/// Context for lambda function generation
struct LambdaContext {
    /// The function ID of the lambda
    func_id: IrFunctionId,
    /// Entry block of the lambda
    entry_block: IrBlockId,
    /// Offset for parameter registers (0 if no env, 1 if has env)
    param_offset: u32,
    /// Environment layout (if captures exist)
    env_layout: Option<EnvironmentLayout>,
}

/// Saved state for restoring after lambda generation
struct SavedLoweringState {
    current_function: Option<IrFunctionId>,
    current_block: Option<IrBlockId>,
    symbol_map: HashMap<SymbolId, IrId>,
}

impl HirToMirLowering {
    fn save_state(&self) -> SavedLoweringState {
        SavedLoweringState {
            current_function: self.builder.current_function,
            current_block: self.builder.current_block,
            symbol_map: self.symbol_map.clone(),
        }
    }

    fn restore_state(&mut self, state: SavedLoweringState) {
        self.builder.current_function = state.current_function;
        self.builder.current_block = state.current_block;
        self.symbol_map = state.symbol_map;
    }
}
```

---

## Part 4: Two-Pass Lambda Generation

### File: `compiler/src/ir/hir_to_mir.rs` (REPLACE `generate_lambda_function`)

```rust
impl HirToMirLowering {
    /// PASS 1: Create lambda skeleton with placeholder signature
    fn generate_lambda_skeleton(
        &mut self,
        params: &[HirParam],
        captures: &[HirCapture],
    ) -> LambdaContext {
        // Allocate function ID
        let func_id = self.builder.module.alloc_function_id();
        let lambda_name = format!("<lambda_{}>", self.lambda_counter);
        self.lambda_counter += 1;

        // Build environment layout if we have captures
        let env_layout = if !captures.is_empty() {
            Some(EnvironmentLayout::new(captures, |ty| self.convert_type(ty)))
        } else {
            None
        };

        // Build parameters: [env*,] lambda_params...
        let mut func_params = Vec::new();
        let mut next_reg_id = 0u32;

        if env_layout.is_some() {
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
            let param_name = self.string_interner
                .get(param.name)
                .unwrap_or("<param>")
                .to_string();

            func_params.push(IrParameter {
                name: param_name,
                ty: param_type,
                reg: IrId::new(next_reg_id),
                by_ref: false,
            });
            next_reg_id += 1;
        }

        // Create PLACEHOLDER signature
        let signature = IrFunctionSignature {
            parameters: func_params,
            return_type: IrType::Any,  // PLACEHOLDER - will be inferred
            calling_convention: CallingConvention::Haxe,
            can_throw: false,
            type_params: vec![],
            uses_sret: false,
        };

        // Create empty function
        let symbol_id = SymbolId::from_raw(1000000 + func_id.as_raw() as u32);
        let lambda_function = IrFunction::new(func_id, symbol_id, lambda_name, signature);
        let entry_block = lambda_function.entry_block();

        // Add to module
        self.builder.module.add_function(lambda_function);

        LambdaContext {
            func_id,
            entry_block,
            param_offset: if env_layout.is_some() { 1 } else { 0 },
            env_layout,
        }
    }

    /// PASS 2: Lower lambda body and infer signature
    fn lower_lambda_body(
        &mut self,
        context: LambdaContext,
        params: &[HirParam],
        body: &HirExpr,
    ) -> Option<IrFunctionId> {
        let LambdaContext { func_id, entry_block, param_offset, env_layout } = context;

        // Save state
        let saved_state = self.save_state();

        // Switch to lambda context
        self.builder.current_function = Some(func_id);
        self.builder.current_block = Some(entry_block);
        self.symbol_map.clear();

        // Map lambda parameters to registers
        for (i, param) in params.iter().enumerate() {
            let param_reg = IrId::new(param_offset + i as u32);
            self.symbol_map.insert(param.symbol_id, param_reg);
        }

        // Setup captured variables using environment layout
        if let Some(layout) = &env_layout {
            let env_ptr = IrId::new(0); // First parameter

            for field in &layout.fields {
                // Use layout to load field (handles casting automatically)
                let value_reg = layout.load_field(&mut self.builder, env_ptr, field.symbol)?;
                self.symbol_map.insert(field.symbol, value_reg);
            }
        }

        // Lower the body expression
        let body_result = self.lower_expression(body);

        // Get function (now populated with instructions)
        let lambda_func = self.builder.module.functions.get_mut(&func_id)?;

        // CRITICAL: Infer return type from actual generated code
        let return_type = self.infer_lambda_return_type(lambda_func, entry_block, body_result);

        // Update signature with inferred type
        lambda_func.signature.return_type = return_type.clone();

        // Add terminator if needed
        self.finalize_lambda_terminator(lambda_func, entry_block, body_result, &return_type)?;

        // Validate consistency
        if let Err(errors) = self.validate_lambda_consistency(lambda_func, &return_type) {
            self.errors.extend(errors);
            return None;
        }

        // Restore state
        self.restore_state(saved_state);

        Some(func_id)
    }

    /// Infer the return type from generated MIR instructions
    fn infer_lambda_return_type(
        &self,
        function: &IrFunction,
        entry_block: IrBlockId,
        body_result: Option<IrId>,
    ) -> IrType {
        // Strategy 1: Check for explicit return terminator
        if let Some(block) = function.cfg.get_block(entry_block) {
            if let IrTerminator::Return { value: Some(ret_reg) } = &block.terminator {
                if let Some(local) = function.locals.get(ret_reg) {
                    eprintln!("DEBUG: Inferred return type from terminator: {:?}", local.ty);
                    return local.ty.clone();
                }
            }
        }

        // Strategy 2: Use body result register type
        if let Some(result_reg) = body_result {
            if let Some(local) = function.locals.get(&result_reg) {
                eprintln!("DEBUG: Inferred return type from body result: {:?}", local.ty);
                return local.ty.clone();
            }
        }

        // Strategy 3: Scan all blocks for return instructions
        for block in function.cfg.blocks() {
            if let IrTerminator::Return { value: Some(ret_reg) } = &block.terminator {
                if let Some(local) = function.locals.get(ret_reg) {
                    eprintln!("DEBUG: Inferred return type from CFG scan: {:?}", local.ty);
                    return local.ty.clone();
                }
            }
        }

        // Fallback: Void return
        eprintln!("DEBUG: No return type found, using Void");
        IrType::Void
    }

    /// Add terminator to lambda if not already present
    fn finalize_lambda_terminator(
        &mut self,
        function: &mut IrFunction,
        entry_block: IrBlockId,
        body_result: Option<IrId>,
        return_type: &IrType,
    ) -> Option<()> {
        let block = function.cfg.get_block_mut(entry_block)?;

        // Check if terminator already exists
        if !matches!(block.terminator, IrTerminator::Unreachable) {
            return Some(()); // Already has terminator
        }

        // Create appropriate terminator
        let terminator = match return_type {
            IrType::Void => IrTerminator::Return { value: None },
            _ => {
                if let Some(result_reg) = body_result {
                    IrTerminator::Return { value: Some(result_reg) }
                } else {
                    // Create default value
                    let default_reg = function.alloc_reg();
                    let default_value = match return_type {
                        IrType::I32 => IrValue::I32(0),
                        IrType::I64 => IrValue::I64(0),
                        IrType::Bool => IrValue::Bool(false),
                        _ => IrValue::I64(0),
                    };

                    block.add_instruction(IrInstruction::Const {
                        dest: default_reg,
                        value: default_value,
                    });

                    IrTerminator::Return { value: Some(default_reg) }
                }
            }
        };

        block.set_terminator(terminator);
        Some(())
    }

    /// Validate that all returns are consistent with signature
    fn validate_lambda_consistency(
        &self,
        function: &IrFunction,
        expected_return_type: &IrType,
    ) -> Result<(), Vec<LoweringError>> {
        let mut errors = Vec::new();

        for block in function.cfg.blocks() {
            if let IrTerminator::Return { value } = &block.terminator {
                match (value, expected_return_type) {
                    (Some(ret_reg), ty) if ty != &IrType::Void => {
                        if let Some(local) = function.locals.get(ret_reg) {
                            if &local.ty != expected_return_type {
                                errors.push(LoweringError {
                                    message: format!(
                                        "Lambda return type mismatch: expected {:?}, got {:?}",
                                        expected_return_type, local.ty
                                    ),
                                    location: SourceLocation::unknown(),
                                });
                            }
                        } else {
                            errors.push(LoweringError {
                                message: format!("Return register {:?} not found in locals", ret_reg),
                                location: SourceLocation::unknown(),
                            });
                        }
                    }
                    (None, IrType::Void) => { /* OK */ }
                    (None, ty) => {
                        errors.push(LoweringError {
                            message: format!("Lambda expects return type {:?} but has void return", ty),
                            location: SourceLocation::unknown(),
                        });
                    }
                    (Some(_), IrType::Void) => {
                        errors.push(LoweringError {
                            message: "Lambda has void return type but returns a value".to_string(),
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

    /// Main entry point: generate complete lambda function (two-pass)
    fn generate_lambda_function(
        &mut self,
        params: &[HirParam],
        body: &HirExpr,
        captures: &[HirCapture],
        _lambda_type: TypeId, // No longer used - type inferred from body
    ) -> Option<IrFunctionId> {
        // PASS 1: Create skeleton
        let context = self.generate_lambda_skeleton(params, captures);

        // PASS 2: Lower body and infer signature
        let func_id = self.lower_lambda_body(context, params, body)?;

        // Log final signature for debugging
        if let Some(function) = self.builder.module.functions.get(&func_id) {
            eprintln!("Info: Generated lambda function '{}' (ID {:?})", function.name, func_id);
            eprintln!("  Signature: ({} params) -> {:?}",
                     function.signature.parameters.len(),
                     function.signature.return_type);
            for (i, param) in function.signature.parameters.iter().enumerate() {
                eprintln!("    param{}: {} ({:?})", i, param.name, param.ty);
            }
            eprintln!("  Captures: {} variables", captures.len());
        }

        Some(func_id)
    }
}
```

---

## Part 5: Update Module Imports

### File: `compiler/src/ir/mod.rs`

```rust
pub mod environment_layout;  // Add this

pub use environment_layout::{EnvironmentLayout, EnvironmentField};
```

### File: `compiler/src/ir/hir_to_mir.rs`

```rust
use crate::ir::environment_layout::{EnvironmentLayout, EnvironmentField};
```

---

## Part 6: Testing

### File: `compiler/tests/lambda_lowering_tests.rs` (NEW FILE)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lambda_no_captures_explicit_return() {
        let haxe = r#"
            var f = () -> { return 42; };
        "#;
        // Should generate:
        // function u0:0() -> i32 { ... return 42; }
        assert_compiles_and_verifies(haxe);
    }

    #[test]
    fn test_lambda_no_captures_expression_return() {
        let haxe = r#"
            var f = () -> 42;
        "#;
        // Should generate:
        // function u0:0() -> i32 { ... return 42; }
        assert_compiles_and_verifies(haxe);
    }

    #[test]
    fn test_lambda_with_capture_i32() {
        let haxe = r#"
            var x = 42;
            var f = () -> x;
        "#;
        // Should generate:
        // function u0:0(i64 env) -> i32 {
        //     v0 = load env[0] -> i64
        //     v1 = cast v0 i64->i32
        //     return v1
        // }
        assert_compiles_and_verifies(haxe);
    }

    #[test]
    fn test_lambda_with_capture_object() {
        let haxe = r#"
            class Message { public var value: Int; }
            var msg = new Message(42);
            var f = () -> msg.value;
        "#;
        // Should generate:
        // function u0:0(i64 env) -> i32 {
        //     v0 = load env[0] -> i64  (object pointer)
        //     v1 = field_access v0.value -> i32
        //     return v1
        // }
        assert_compiles_and_verifies(haxe);
    }

    #[test]
    fn test_lambda_with_parameters_and_captures() {
        let haxe = r#"
            var x = 10;
            var f = (y: Int) -> x + y;
        "#;
        // Should generate:
        // function u0:0(i64 env, i32 y) -> i32 {
        //     v0 = load env[0] -> i64
        //     v1 = cast v0 i64->i32
        //     v2 = add v1, y -> i32
        //     return v2
        // }
        assert_compiles_and_verifies(haxe);
    }
}
```

---

## Part 7: Migration Checklist

```
Phase 1: Preparation
[ ] Create environment_layout.rs
[ ] Add EnvironmentLayout tests
[ ] Add enhanced builder methods (build_load, build_cast, build_binop with auto-tracking)
[ ] Add tests for builder auto-tracking

Phase 2: Refactoring
[ ] Add LambdaContext and SavedLoweringState structs
[ ] Extract generate_lambda_skeleton
[ ] Implement infer_lambda_return_type
[ ] Implement validate_lambda_consistency
[ ] Add unit tests for each function

Phase 3: Integration
[ ] Replace generate_lambda_function with two-pass version
[ ] Update lower_lambda call site
[ ] Remove all manual locals.insert() calls from capture setup
[ ] Run test_rayzor_stdlib_e2e - should see 6/6 passing

Phase 4: Cleanup
[ ] Remove debug eprintln! statements (or make them conditional)
[ ] Update documentation
[ ] Code review
[ ] Performance profiling
```

---

## Expected Results

### Before:
```
❌ thread_spawn_basic FAILED (type mismatch: i32 vs i64)
❌ thread_spawn_qualified FAILED (type mismatch: i32 vs i64)
❌ thread_multiple FAILED (argument count mismatch)
❌ channel_basic FAILED (capture not found)
✅ mutex_basic PASSED
✅ arc_basic PASSED

Tests passing: 2/6 (33%)
```

### After:
```
✅ thread_spawn_basic PASSED
✅ thread_spawn_qualified PASSED
✅ thread_multiple PASSED
✅ channel_basic PASSED
✅ mutex_basic PASSED
✅ arc_basic PASSED

Tests passing: 6/6 (100%)
```

---

## Quick Start

1. **Copy `environment_layout.rs`** to `compiler/src/ir/`
2. **Add enhanced methods** to `IrBuilder` in `builder.rs`
3. **Add new structs** to `hir_to_mir.rs`
4. **Replace `generate_lambda_function`** in `hir_to_mir.rs`
5. **Run tests**: `cargo test test_rayzor_stdlib_e2e`
6. **Verify**: Should see 6/6 tests passing

Total implementation time: ~2-3 hours for experienced developer

