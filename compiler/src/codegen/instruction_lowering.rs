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
            // Extend i8 boolean result to i32
            Ok(builder.ins().uextend(types::I32, cmp))
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
            // Extend i8 boolean result to i32
            Ok(builder.ins().uextend(types::I32, cmp))
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

        let value = match op {
            BinaryOp::Add => builder.ins().iadd(lhs, rhs),
            BinaryOp::Sub => builder.ins().isub(lhs, rhs),
            BinaryOp::Mul => builder.ins().imul(lhs, rhs),
            BinaryOp::Div => {
                // Debug: Print type information
                eprintln!("DEBUG Division: op={:?}, ty={:?}, is_float={}, is_signed={}",
                    op, ty, ty.is_float(), ty.is_signed());

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
            Ok(builder.ins().uextend(types::I32, cmp))
        } else {
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
            Ok(builder.ins().uextend(types::I32, cmp))
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
            size
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
