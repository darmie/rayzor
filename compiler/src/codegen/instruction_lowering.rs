/// Instruction lowering for MIR → Cranelift IR
///
/// This module handles the translation of MIR instructions to Cranelift IR.
/// Based on tested implementation from Zyntax compiler.

use cranelift::prelude::*;
use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use std::collections::HashMap;

use crate::ir::{BinaryOp, CompareOp, IrId, IrInstruction, IrType, UnaryOp};

use super::CraneliftBackend;

impl CraneliftBackend {
    /// Lower a binary operation to Cranelift IR
    pub(super) fn lower_binary_op(
        &mut self,
        builder: &mut FunctionBuilder,
        op: &BinaryOp,
        ty: &IrType,
        left: IrId,
        right: IrId,
    ) -> Result<Value, String> {
        let lhs = *self.value_map.get(&left).ok_or("Left operand not found")?;
        let rhs = *self.value_map.get(&right).ok_or("Right operand not found")?;

        // Check if operands have matching types and cast if needed
        // Same logic as lower_binary_op_static to handle i32/i64 type mismatches
        let lhs_ty = builder.func.dfg.value_type(lhs);
        let rhs_ty = builder.func.dfg.value_type(rhs);

        // Convert the MIR type to Cranelift type to get the expected operation type
        let expected_ty = match ty {
            IrType::I32 => types::I32,
            IrType::I64 => types::I64,
            IrType::U32 => types::I32,
            IrType::U64 => types::I64,
            IrType::F32 => types::F32,
            IrType::F64 => types::F64,
            IrType::Bool => types::I32,
            _ => types::I64, // Default to I64 for other types
        };

        // Coerce both operands to a common type - always use the larger of the two
        // to avoid truncating pointers (i64 values from generic functions)
        let operation_ty = if lhs_ty.bits() >= rhs_ty.bits() { lhs_ty } else { rhs_ty };
        let operation_ty = if operation_ty.bits() > expected_ty.bits() {
            operation_ty
        } else {
            expected_ty
        };

        let lhs = if lhs_ty != operation_ty && lhs_ty.is_int() && operation_ty.is_int() {
            if ty.is_signed() {
                builder.ins().sextend(operation_ty, lhs)
            } else {
                builder.ins().uextend(operation_ty, lhs)
            }
        } else {
            lhs
        };

        let rhs = if rhs_ty != operation_ty && rhs_ty.is_int() && operation_ty.is_int() {
            if ty.is_signed() {
                builder.ins().sextend(operation_ty, rhs)
            } else {
                builder.ins().uextend(operation_ty, rhs)
            }
        } else {
            rhs
        };

        let value = match op {
            BinaryOp::Add => builder.ins().iadd(lhs, rhs),
            BinaryOp::Sub => builder.ins().isub(lhs, rhs),
            BinaryOp::Mul => builder.ins().imul(lhs, rhs),
            BinaryOp::Div => {
                if ty.is_float() {
                    builder.ins().fdiv(lhs, rhs)
                } else if ty.is_signed() {
                    builder.ins().sdiv(lhs, rhs)
                } else {
                    builder.ins().udiv(lhs, rhs)
                }
            }
            BinaryOp::Rem => {
                if ty.is_signed() {
                    builder.ins().srem(lhs, rhs)
                } else {
                    builder.ins().urem(lhs, rhs)
                }
            }
            BinaryOp::And => builder.ins().band(lhs, rhs),
            BinaryOp::Or => builder.ins().bor(lhs, rhs),
            BinaryOp::Xor => builder.ins().bxor(lhs, rhs),
            BinaryOp::Shl => builder.ins().ishl(lhs, rhs),
            BinaryOp::Shr => {
                if ty.is_signed() {
                    builder.ins().sshr(lhs, rhs)
                } else {
                    builder.ins().ushr(lhs, rhs)
                }
            }
            // Floating point operations
            BinaryOp::FAdd => builder.ins().fadd(lhs, rhs),
            BinaryOp::FSub => builder.ins().fsub(lhs, rhs),
            BinaryOp::FMul => builder.ins().fmul(lhs, rhs),
            BinaryOp::FDiv => builder.ins().fdiv(lhs, rhs),
            BinaryOp::FRem => {
                // TODO: Cranelift doesn't have frem, would need libm fmod
                builder.ins().fdiv(lhs, rhs) // Placeholder
            }
        };

        Ok(value)
    }

    /// Lower a comparison operation to Cranelift IR
    pub(super) fn lower_compare_op(
        &mut self,
        builder: &mut FunctionBuilder,
        op: &CompareOp,
        ty: &IrType,
        left: IrId,
        right: IrId,
    ) -> Result<Value, String> {
        let lhs = *self.value_map.get(&left).ok_or("Left operand not found")?;
        let rhs = *self.value_map.get(&right).ok_or("Right operand not found")?;

        // Floating point comparisons
        if ty.is_float() || matches!(op, CompareOp::FEq | CompareOp::FNe | CompareOp::FLt | CompareOp::FLe | CompareOp::FGt | CompareOp::FGe | CompareOp::FOrd | CompareOp::FUno) {
            let cc = match op {
                CompareOp::Eq | CompareOp::FEq => FloatCC::Equal,
                CompareOp::Ne | CompareOp::FNe => FloatCC::NotEqual,
                CompareOp::Lt | CompareOp::FLt => FloatCC::LessThan,
                CompareOp::Le | CompareOp::FLe => FloatCC::LessThanOrEqual,
                CompareOp::Gt | CompareOp::FGt => FloatCC::GreaterThan,
                CompareOp::Ge | CompareOp::FGe => FloatCC::GreaterThanOrEqual,
                CompareOp::FOrd => FloatCC::Ordered,
                CompareOp::FUno => FloatCC::Unordered,
                _ => return Err(format!("Invalid float comparison: {:?}", op)),
            };
            let cmp = builder.ins().fcmp(cc, lhs, rhs);
            // Return the i8 boolean result directly - don't extend to i32
            // Bool is represented as i8 in the type system
            Ok(cmp)
        } else {
            // Integer comparisons
            let cc = match op {
                CompareOp::Eq => IntCC::Equal,
                CompareOp::Ne => IntCC::NotEqual,
                CompareOp::Lt => IntCC::SignedLessThan,
                CompareOp::Le => IntCC::SignedLessThanOrEqual,
                CompareOp::Gt => IntCC::SignedGreaterThan,
                CompareOp::Ge => IntCC::SignedGreaterThanOrEqual,
                CompareOp::ULt => IntCC::UnsignedLessThan,
                CompareOp::ULe => IntCC::UnsignedLessThanOrEqual,
                CompareOp::UGt => IntCC::UnsignedGreaterThan,
                CompareOp::UGe => IntCC::UnsignedGreaterThanOrEqual,
                _ => return Err(format!("Invalid int comparison: {:?}", op)),
            };
            let cmp = builder.ins().icmp(cc, lhs, rhs);
            // Return the i8 boolean result directly - don't extend to i32
            // Bool is represented as i8 in the type system
            Ok(cmp)
        }
    }

    /// Lower a unary operation to Cranelift IR
    pub(super) fn lower_unary_op(
        &mut self,
        builder: &mut FunctionBuilder,
        op: &UnaryOp,
        ty: &IrType,
        operand: IrId,
    ) -> Result<Value, String> {
        let val = *self.value_map.get(&operand).ok_or("Operand not found")?;

        let value = match op {
            UnaryOp::Neg => {
                if ty.is_float() {
                    builder.ins().fneg(val)
                } else {
                    builder.ins().ineg(val)
                }
            }
            UnaryOp::Not => builder.ins().bnot(val),
            UnaryOp::FNeg => builder.ins().fneg(val),
        };

        Ok(value)
    }

    /// Lower a load instruction to Cranelift IR
    pub(super) fn lower_load(
        &mut self,
        builder: &mut FunctionBuilder,
        ty: &IrType,
        ptr: IrId,
    ) -> Result<Value, String> {
        let ptr_val = *self.value_map.get(&ptr).ok_or("Pointer not found")?;
        let cranelift_ty = self.mir_type_to_cranelift(ty)?;

        let flags = MemFlags::new().with_aligned().with_notrap();
        let value = builder.ins().load(cranelift_ty, flags, ptr_val, 0);

        Ok(value)
    }

    /// Lower a store instruction to Cranelift IR
    pub(super) fn lower_store(
        &mut self,
        builder: &mut FunctionBuilder,
        value: IrId,
        ptr: IrId,
    ) -> Result<(), String> {
        let val = *self.value_map.get(&value).ok_or("Value not found")?;
        let ptr_val = *self.value_map.get(&ptr).ok_or("Pointer not found")?;

        let flags = MemFlags::new().with_aligned().with_notrap();
        builder.ins().store(flags, val, ptr_val, 0);

        Ok(())
    }

    /// Lower an alloca instruction to Cranelift IR (using stack slots)
    pub(super) fn lower_alloca(
        &mut self,
        builder: &mut FunctionBuilder,
        ty: &IrType,
        count: Option<u32>,
    ) -> Result<Value, String> {
        let size = type_size(ty)?;
        let alloc_size = if let Some(c) = count {
            size * c
        } else {
            size
        };

        let slot_data = StackSlotData::new(StackSlotKind::ExplicitSlot, alloc_size, 8); // 8-byte alignment
        let slot = builder.create_sized_stack_slot(slot_data);
        let addr = builder.ins().stack_addr(
            types::I64, // Pointer type
            slot,
            0,
        );

        Ok(addr)
    }

    // =========================================================================
    // Static versions of lowering methods (for use without &mut self borrow)
    // =========================================================================

    /// Lower a binary operation (static version)
    pub(super) fn lower_binary_op_static(
        value_map: &HashMap<IrId, Value>,
        builder: &mut FunctionBuilder,
        op: &BinaryOp,
        ty: &IrType,
        left: IrId,
        right: IrId,
    ) -> Result<Value, String> {
        let lhs = *value_map.get(&left).ok_or_else(|| {
            eprintln!("ERROR: Left operand {:?} not found. Available keys: {:?}", left, value_map.keys().collect::<Vec<_>>());
            format!("Left operand {:?} not found in value_map", left)
        })?;
        let rhs = *value_map.get(&right).ok_or_else(|| format!("Right operand {:?} not found in value_map", right))?;

        // Check if operands have matching types and cast if needed
        // IMPORTANT: Coerce operands to match the expected operation type (ty),
        // not just each other. This is critical for closure environments where
        // captured variables are stored as i64 but operations may expect i32.
        let lhs_ty = builder.func.dfg.value_type(lhs);
        let rhs_ty = builder.func.dfg.value_type(rhs);

        // Convert the MIR type to Cranelift type to get the expected operation type
        let expected_ty = match ty {
            IrType::I32 => types::I32,
            IrType::I64 => types::I64,
            IrType::U32 => types::I32,
            IrType::U64 => types::I64,
            IrType::F32 => types::F32,
            IrType::F64 => types::F64,
            IrType::Bool => types::I32,
            _ => types::I64, // Default to I64 for other types
        };

        // Coerce both operands to a common type for the operation.
        // IMPORTANT: We NEVER truncate from I64 to I32 because the I64 value might be
        // a pointer loaded from a closure environment. Instead, we always extend to
        // the larger type (I64) and perform the operation at that width.
        //
        // The basic strategy is:
        // - If both operands are integers, use the larger of the two operand types
        // - If operands differ, extend the smaller one to match the larger
        // - Never reduce I64 to I32 (would corrupt pointers)
        // - CRITICAL: When expected_ty is float but operands are integers, use integer coercion
        //   (this happens with generic types that resolve to different types)

        // Determine operation type based on actual operand types
        // When both operands are integers but expected is not, use the larger integer type
        let both_operands_are_int = lhs_ty.is_int() && rhs_ty.is_int();
        let larger_operand_ty = if lhs_ty.bits() >= rhs_ty.bits() { lhs_ty } else { rhs_ty };

        let operation_ty = if both_operands_are_int && !expected_ty.is_int() {
            // Expected type is not an integer but operands are - use the larger operand type
            // This happens with unresolved generics that become Ptr(Void) -> I64
            larger_operand_ty
        } else if larger_operand_ty.is_int() && expected_ty.is_int() && larger_operand_ty.bits() > expected_ty.bits() {
            // Operand type is larger than expected - use larger to prevent truncation
            larger_operand_ty
        } else if expected_ty.is_int() {
            expected_ty
        } else {
            // For non-integer operations (float), use expected type
            expected_ty
        };

        let lhs = if lhs_ty != operation_ty && lhs_ty.is_int() && operation_ty.is_int() {
            // Always extend, never reduce
            if ty.is_signed() {
                builder.ins().sextend(operation_ty, lhs)
            } else {
                builder.ins().uextend(operation_ty, lhs)
            }
        } else {
            lhs
        };

        let rhs = if rhs_ty != operation_ty && rhs_ty.is_int() && operation_ty.is_int() {
            // Always extend, never reduce
            if ty.is_signed() {
                builder.ins().sextend(operation_ty, rhs)
            } else {
                builder.ins().uextend(operation_ty, rhs)
            }
        } else {
            rhs
        };

        let value = match op {
            BinaryOp::Add => builder.ins().iadd(lhs, rhs),
            BinaryOp::Sub => builder.ins().isub(lhs, rhs),
            BinaryOp::Mul => builder.ins().imul(lhs, rhs),
            BinaryOp::Div => {
                if ty.is_float() {
                    builder.ins().fdiv(lhs, rhs)
                } else if ty.is_signed() {
                    builder.ins().sdiv(lhs, rhs)
                } else {
                    builder.ins().udiv(lhs, rhs)
                }
            }
            BinaryOp::Rem => {
                if ty.is_signed() {
                    builder.ins().srem(lhs, rhs)
                } else {
                    builder.ins().urem(lhs, rhs)
                }
            }
            BinaryOp::And => builder.ins().band(lhs, rhs),
            BinaryOp::Or => builder.ins().bor(lhs, rhs),
            BinaryOp::Xor => builder.ins().bxor(lhs, rhs),
            BinaryOp::Shl => builder.ins().ishl(lhs, rhs),
            BinaryOp::Shr => {
                if ty.is_signed() {
                    builder.ins().sshr(lhs, rhs)
                } else {
                    builder.ins().ushr(lhs, rhs)
                }
            }
            BinaryOp::FAdd => builder.ins().fadd(lhs, rhs),
            BinaryOp::FSub => builder.ins().fsub(lhs, rhs),
            BinaryOp::FMul => builder.ins().fmul(lhs, rhs),
            BinaryOp::FDiv => builder.ins().fdiv(lhs, rhs),
            BinaryOp::FRem => builder.ins().fdiv(lhs, rhs), // TODO: Implement proper frem
        };

        Ok(value)
    }

    /// Lower a comparison operation (static version)
    pub(super) fn lower_compare_op_static(
        value_map: &HashMap<IrId, Value>,
        builder: &mut FunctionBuilder,
        op: &CompareOp,
        ty: &IrType,
        left: IrId,
        right: IrId,
    ) -> Result<Value, String> {
        let lhs = *value_map.get(&left).ok_or_else(|| {
            eprintln!("ERROR: Left operand {:?} not found. Available keys: {:?}", left, value_map.keys().collect::<Vec<_>>());
            format!("Left operand {:?} not found in value_map", left)
        })?;
        let rhs = *value_map.get(&right).ok_or_else(|| format!("Right operand {:?} not found in value_map", right))?;

        if ty.is_float() || matches!(op, CompareOp::FEq | CompareOp::FNe | CompareOp::FLt | CompareOp::FLe | CompareOp::FGt | CompareOp::FGe | CompareOp::FOrd | CompareOp::FUno) {
            let cc = match op {
                CompareOp::Eq | CompareOp::FEq => FloatCC::Equal,
                CompareOp::Ne | CompareOp::FNe => FloatCC::NotEqual,
                CompareOp::Lt | CompareOp::FLt => FloatCC::LessThan,
                CompareOp::Le | CompareOp::FLe => FloatCC::LessThanOrEqual,
                CompareOp::Gt | CompareOp::FGt => FloatCC::GreaterThan,
                CompareOp::Ge | CompareOp::FGe => FloatCC::GreaterThanOrEqual,
                CompareOp::FOrd => FloatCC::Ordered,
                CompareOp::FUno => FloatCC::Unordered,
                _ => return Err(format!("Invalid float comparison: {:?}", op)),
            };
            let cmp = builder.ins().fcmp(cc, lhs, rhs);
            // Return the i8 boolean result directly - don't extend to i32
            // Bool is represented as i8 in the type system
            Ok(cmp)
        } else {
            // Get the types of both operands
            let lhs_ty = builder.func.dfg.value_type(lhs);
            let rhs_ty = builder.func.dfg.value_type(rhs);

            // If types don't match, extend the smaller one to match the larger
            let (final_lhs, final_rhs) = if lhs_ty != rhs_ty && lhs_ty.is_int() && rhs_ty.is_int() {
                let lhs_bits = lhs_ty.bits();
                let rhs_bits = rhs_ty.bits();

                if lhs_bits > rhs_bits {
                    // Extend rhs to match lhs
                    let extended_rhs = if ty.is_signed() {
                        builder.ins().sextend(lhs_ty, rhs)
                    } else {
                        builder.ins().uextend(lhs_ty, rhs)
                    };
                    (lhs, extended_rhs)
                } else {
                    // Extend lhs to match rhs
                    let extended_lhs = if ty.is_signed() {
                        builder.ins().sextend(rhs_ty, lhs)
                    } else {
                        builder.ins().uextend(rhs_ty, lhs)
                    };
                    (extended_lhs, rhs)
                }
            } else {
                (lhs, rhs)
            };

            let cc = match op {
                CompareOp::Eq => IntCC::Equal,
                CompareOp::Ne => IntCC::NotEqual,
                CompareOp::Lt => IntCC::SignedLessThan,
                CompareOp::Le => IntCC::SignedLessThanOrEqual,
                CompareOp::Gt => IntCC::SignedGreaterThan,
                CompareOp::Ge => IntCC::SignedGreaterThanOrEqual,
                CompareOp::ULt => IntCC::UnsignedLessThan,
                CompareOp::ULe => IntCC::UnsignedLessThanOrEqual,
                CompareOp::UGt => IntCC::UnsignedGreaterThan,
                CompareOp::UGe => IntCC::UnsignedGreaterThanOrEqual,
                _ => return Err(format!("Invalid int comparison: {:?}", op)),
            };
            let cmp = builder.ins().icmp(cc, final_lhs, final_rhs);
            // Return the i8 boolean result directly - don't extend to i32
            // Bool is represented as i8 in the type system
            Ok(cmp)
        }
    }

    /// Lower a unary operation (static version)
    pub(super) fn lower_unary_op_static(
        value_map: &HashMap<IrId, Value>,
        builder: &mut FunctionBuilder,
        op: &UnaryOp,
        ty: &IrType,
        operand: IrId,
    ) -> Result<Value, String> {
        let val = *value_map.get(&operand).ok_or("Operand not found")?;

        let value = match op {
            UnaryOp::Neg => {
                if ty.is_float() {
                    builder.ins().fneg(val)
                } else {
                    builder.ins().ineg(val)
                }
            }
            UnaryOp::Not => builder.ins().bnot(val),
            UnaryOp::FNeg => builder.ins().fneg(val),
        };

        Ok(value)
    }

    /// Lower a load operation (static version)
    pub(super) fn lower_load_static(
        value_map: &HashMap<IrId, Value>,
        builder: &mut FunctionBuilder,
        ty: &IrType,
        ptr: IrId,
    ) -> Result<Value, String> {
        let ptr_val = *value_map.get(&ptr).ok_or("Pointer not found")?;
        let cranelift_ty = Self::mir_type_to_cranelift_static(ty)?;
        let flags = MemFlags::new().with_aligned().with_notrap();
        let value = builder.ins().load(cranelift_ty, flags, ptr_val, 0);
        Ok(value)
    }

    /// Lower a store operation (static version)
    pub(super) fn lower_store_static(
        value_map: &HashMap<IrId, Value>,
        builder: &mut FunctionBuilder,
        ptr: IrId,
        value: IrId,
    ) -> Result<(), String> {
        let val = *value_map.get(&value).ok_or("Value not found")?;
        let ptr_val = *value_map.get(&ptr).ok_or("Pointer not found")?;
        let flags = MemFlags::new().with_aligned().with_notrap();
        builder.ins().store(flags, val, ptr_val, 0);
        Ok(())
    }

    /// Lower an alloca operation (static version)
    pub(super) fn lower_alloca_static(
        builder: &mut FunctionBuilder,
        ty: &IrType,
        count: Option<u32>,
    ) -> Result<Value, String> {
        let size = type_size(ty)?;
        let alloc_size = if let Some(c) = count {
            size * c
        } else {
            // WORKAROUND: For complex types (Any, Ptr, etc.) that might be dynamic arrays,
            // allocate extra space to avoid stack corruption.
            // Arrays should really be heap-allocated, but for now we allocate enough
            // stack space for reasonable array sizes (up to 16 elements).
            if matches!(ty, IrType::Any | IrType::Ptr(_) | IrType::Ref(_)) {
                size * 16  // Allocate space for up to 16 pointers/elements
            } else {
                size
            }
        };

        let slot_data = StackSlotData::new(StackSlotKind::ExplicitSlot, alloc_size, 8);
        let slot = builder.create_sized_stack_slot(slot_data);
        let addr = builder.ins().stack_addr(types::I64, slot, 0);
        Ok(addr)
    }

}

/// Calculate type size in bytes
fn type_size(ty: &IrType) -> Result<u32, String> {
    Ok(match ty {
        IrType::I8 | IrType::U8 | IrType::Bool => 1,
        IrType::I16 | IrType::U16 => 2,
        IrType::I32 | IrType::U32 | IrType::F32 => 4,
        IrType::I64 | IrType::U64 | IrType::F64 => 8,
        IrType::Ptr(_) | IrType::Ref(_) | IrType::Any | IrType::Function { .. } => 8,
        _ => 8, // Default to pointer size for complex types
    })
}

// Helper trait to check type properties
pub trait TypeProperties {
    fn is_float(&self) -> bool;
    fn is_signed(&self) -> bool;
}

impl TypeProperties for IrType {
    fn is_float(&self) -> bool {
        matches!(self, IrType::F32 | IrType::F64)
    }

    fn is_signed(&self) -> bool {
        matches!(
            self,
            IrType::I8 | IrType::I16 | IrType::I32 | IrType::I64
        )
    }
}
