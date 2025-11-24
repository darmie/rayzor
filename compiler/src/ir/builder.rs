//! HIR Builder
//!
//! This module provides a builder interface for constructing HIR in a convenient way.
//! The builder maintains context and provides helper methods for common patterns.

use super::{
    IrModule, IrFunction, IrFunctionId, IrBasicBlock, IrBlockId,
    IrInstruction, IrTerminator, IrPhiNode, IrId, IrType, IrValue,
    IrSourceLocation, BinaryOp, UnaryOp, CompareOp,
    IrFunctionSignature, IrParameter, CallingConvention,
};
use crate::tast::SymbolId;
use std::collections::HashMap;

/// HIR builder for constructing IR modules
pub struct IrBuilder {
    /// The module being built
    pub module: IrModule,
    
    /// Current function being built
    current_function: Option<IrFunctionId>,
    
    /// Current basic block being built
    current_block: Option<IrBlockId>,
    
    /// Source location context
    current_source_location: IrSourceLocation,
}

impl IrBuilder {
    /// Create a new IR builder
    pub fn new(module_name: String, source_file: String) -> Self {
        Self {
            module: IrModule::new(module_name, source_file),
            current_function: None,
            current_block: None,
            current_source_location: IrSourceLocation::unknown(),
        }
    }
    
    /// Set the current source location for debugging
    pub fn set_source_location(&mut self, loc: IrSourceLocation) {
        self.current_source_location = loc;
    }
    
    // === Module Building ===
    
    /// Start building a new function
    pub fn start_function(
        &mut self,
        symbol_id: SymbolId,
        name: String,
        signature: IrFunctionSignature,
    ) -> IrFunctionId {
        let id = self.module.alloc_function_id();
        let function = IrFunction::new(id, symbol_id, name, signature);
        self.current_function = Some(id);
        self.current_block = Some(function.entry_block());
        self.module.add_function(function);
        id
    }
    
    /// Finish building the current function
    pub fn finish_function(&mut self) {
        self.current_function = None;
        self.current_block = None;
    }
    
    /// Get the current function
    pub fn current_function(&self) -> Option<&IrFunction> {
        self.current_function
            .and_then(|id| self.module.functions.get(&id))
    }
    
    /// Get the current function mutably
    pub fn current_function_mut(&mut self) -> Option<&mut IrFunction> {
        self.current_function
            .and_then(move |id| self.module.functions.get_mut(&id))
    }
    
    // === Block Building ===
    
    /// Create a new basic block in the current function
    pub fn create_block(&mut self) -> Option<IrBlockId> {
        self.current_function_mut().map(|f| f.cfg.create_block())
    }
    
    /// Create a new basic block with a label
    pub fn create_block_with_label(&mut self, label: String) -> Option<IrBlockId> {
        let block_id = self.create_block()?;
        self.current_function_mut()
            .and_then(|f| f.cfg.get_block_mut(block_id))
            .map(|b| b.label = Some(label));
        Some(block_id)
    }
    
    /// Switch to building in a different block
    pub fn switch_to_block(&mut self, block: IrBlockId) {
        self.current_block = Some(block);
    }
    
    /// Get the current block
    pub fn current_block(&self) -> Option<IrBlockId> {
        self.current_block
    }
    
    // === Register Management ===
    
    /// Allocate a new register in the current function
    pub fn alloc_reg(&mut self) -> Option<IrId> {
        self.current_function_mut().map(|f| f.alloc_reg())
    }
    
    /// Declare a local variable
    pub fn declare_local(&mut self, name: String, ty: IrType) -> Option<IrId> {
        self.current_function_mut().map(|f| f.declare_local(name, ty))
    }
    
    // === Instruction Building ===
    
    /// Add an instruction to the current block
    fn add_instruction(&mut self, inst: IrInstruction) -> Option<()> {
        let block_id = self.current_block?;
        self.current_function_mut()
            .and_then(|f| f.cfg.get_block_mut(block_id))
            .map(|b| b.add_instruction(inst))
    }
    
    /// Build a constant instruction
    pub fn build_const(&mut self, value: IrValue) -> Option<IrId> {
        let dest = self.alloc_reg()?;
        self.add_instruction(IrInstruction::Const { dest, value })?;
        Some(dest)
    }
    
    /// Build a copy instruction
    pub fn build_copy(&mut self, src: IrId) -> Option<IrId> {
        let dest = self.alloc_reg()?;
        self.add_instruction(IrInstruction::Copy { dest, src })?;
        Some(dest)
    }
    
    /// Build a load instruction
    pub fn build_load(&mut self, ptr: IrId, ty: IrType) -> Option<IrId> {
        let dest = self.alloc_reg()?;
        self.add_instruction(IrInstruction::Load { dest, ptr, ty })?;
        Some(dest)
    }
    
    /// Build a store instruction
    pub fn build_store(&mut self, ptr: IrId, value: IrId) -> Option<()> {
        self.add_instruction(IrInstruction::Store { ptr, value })
    }
    
    /// Build a binary operation
    pub fn build_binop(&mut self, op: BinaryOp, left: IrId, right: IrId) -> Option<IrId> {
        let dest = self.alloc_reg()?;
        self.add_instruction(IrInstruction::BinOp { dest, op, left, right })?;
        Some(dest)
    }
    
    /// Build a unary operation
    pub fn build_unop(&mut self, op: UnaryOp, operand: IrId) -> Option<IrId> {
        let dest = self.alloc_reg()?;
        self.add_instruction(IrInstruction::UnOp { dest, op, operand })?;
        Some(dest)
    }
    
    /// Build a comparison operation
    pub fn build_cmp(&mut self, op: CompareOp, left: IrId, right: IrId) -> Option<IrId> {
        let dest = self.alloc_reg()?;
        self.add_instruction(IrInstruction::Cmp { dest, op, left, right })?;
        Some(dest)
    }
    
    /// Build a function call
    pub fn build_call(
        &mut self,
        func: IrId,
        args: Vec<IrId>,
        _ty: IrType,
    ) -> Option<IrId> {
        let dest = self.alloc_reg()?;
        self.add_instruction(IrInstruction::Call { dest: Some(dest), func, args })?;
        Some(dest)
    }
    
    /// Build a cast instruction
    pub fn build_cast(
        &mut self,
        src: IrId,
        from_ty: IrType,
        to_ty: IrType,
    ) -> Option<IrId> {
        let dest = self.alloc_reg()?;
        self.add_instruction(IrInstruction::Cast { dest, src, from_ty, to_ty })?;
        Some(dest)
    }
    
    /// Build an alloc instruction
    pub fn build_alloc(&mut self, ty: IrType, count: Option<IrId>) -> Option<IrId> {
        let dest = self.alloc_reg()?;
        self.add_instruction(IrInstruction::Alloc { dest, ty, count })?;
        Some(dest)
    }
    
    /// Build a GEP (get element pointer) instruction
    pub fn build_gep(
        &mut self,
        ptr: IrId,
        indices: Vec<IrId>,
        ty: IrType,
    ) -> Option<IrId> {
        let dest = self.alloc_reg()?;
        self.add_instruction(IrInstruction::GetElementPtr { dest, ptr, indices, ty })?;
        Some(dest)
    }
    
    /// Build a select (ternary) instruction
    pub fn build_select(
        &mut self,
        condition: IrId,
        true_val: IrId,
        false_val: IrId,
    ) -> Option<IrId> {
        let dest = self.alloc_reg()?;
        self.add_instruction(IrInstruction::Select {
            dest,
            condition,
            true_val,
            false_val,
        })?;
        Some(dest)
    }
    
    /// Build an extract value instruction for accessing aggregate elements
    pub fn build_extract_value(
        &mut self,
        aggregate: IrId,
        indices: Vec<u32>,
    ) -> Option<IrId> {
        let dest = self.alloc_reg()?;
        self.add_instruction(IrInstruction::ExtractValue {
            dest,
            aggregate,
            indices,
        })?;
        Some(dest)
    }
    
    // === Terminator Building ===
    
    /// Set the terminator for the current block
    fn set_terminator(&mut self, term: IrTerminator) -> Option<()> {
        let block_id = self.current_block?;
        let func = self.current_function_mut()?;
        
        // First, set the terminator
        func.cfg.get_block_mut(block_id)?.set_terminator(term.clone());
        
        // Then, update predecessor information based on the terminator
        match &term {
            IrTerminator::Branch { target } => {
                func.cfg.connect_blocks(block_id, *target);
            }
            IrTerminator::CondBranch { true_target, false_target, .. } => {
                func.cfg.connect_blocks(block_id, *true_target);
                func.cfg.connect_blocks(block_id, *false_target);
            }
            IrTerminator::Switch { cases, default, .. } => {
                for (_, target) in cases {
                    func.cfg.connect_blocks(block_id, *target);
                }
                func.cfg.connect_blocks(block_id, *default);
            }
            _ => {}
        }
        
        Some(())
    }
    
    /// Build an unconditional branch
    pub fn build_branch(&mut self, target: IrBlockId) -> Option<()> {
        self.set_terminator(IrTerminator::Branch { target })
    }
    
    /// Build a conditional branch
    pub fn build_cond_branch(
        &mut self,
        condition: IrId,
        true_target: IrBlockId,
        false_target: IrBlockId,
    ) -> Option<()> {
        self.set_terminator(IrTerminator::CondBranch {
            condition,
            true_target,
            false_target,
        })
    }
    
    /// Build a switch statement
    pub fn build_switch(
        &mut self,
        value: IrId,
        cases: Vec<(i64, IrBlockId)>,
        default: IrBlockId,
    ) -> Option<()> {
        self.set_terminator(IrTerminator::Switch { value, cases, default })
    }
    
    /// Build a return instruction
    pub fn build_return(&mut self, value: Option<IrId>) -> Option<()> {
        self.set_terminator(IrTerminator::Return { value })
    }
    
    /// Build an unreachable terminator
    pub fn build_unreachable(&mut self) -> Option<()> {
        self.set_terminator(IrTerminator::Unreachable)
    }
    
    // === Phi Node Building ===
    
    /// Add a phi node to a block
    pub fn build_phi(&mut self, block: IrBlockId, ty: IrType) -> Option<IrId> {
        let dest = self.alloc_reg()?;
        let phi = IrPhiNode {
            dest,
            incoming: Vec::new(),
            ty,
        };
        
        self.current_function_mut()
            .and_then(|f| f.cfg.get_block_mut(block))
            .map(|b| b.add_phi(phi))?;
            
        Some(dest)
    }
    
    /// Add an incoming value to a phi node
    pub fn add_phi_incoming(
        &mut self,
        block: IrBlockId,
        phi_dest: IrId,
        from_block: IrBlockId,
        value: IrId,
    ) -> Option<()> {
        self.current_function_mut()
            .and_then(|f| f.cfg.get_block_mut(block))
            .and_then(|b| b.phi_nodes.iter_mut().find(|p| p.dest == phi_dest))
            .map(|phi| phi.incoming.push((from_block, value)))
    }
    
    // === Convenience Methods ===
    
    /// Build an integer constant
    pub fn build_int(&mut self, value: i64, ty: IrType) -> Option<IrId> {
        let ir_value = match ty {
            IrType::I8 => IrValue::I8(value as i8),
            IrType::I16 => IrValue::I16(value as i16),
            IrType::I32 => IrValue::I32(value as i32),
            IrType::I64 => IrValue::I64(value),
            IrType::U8 => IrValue::U8(value as u8),
            IrType::U16 => IrValue::U16(value as u16),
            IrType::U32 => IrValue::U32(value as u32),
            IrType::U64 => IrValue::U64(value as u64),
            _ => return None,
        };
        self.build_const(ir_value)
    }
    
    /// Build a boolean constant
    pub fn build_bool(&mut self, value: bool) -> Option<IrId> {
        self.build_const(IrValue::Bool(value))
    }
    
    /// Build a string constant
    pub fn build_string(&mut self, value: String) -> Option<IrId> {
        // Add to string pool
        let _string_id = self.module.string_pool.add(value.clone());
        self.build_const(IrValue::String(value))
    }
    
    /// Build a null pointer constant
    pub fn build_null(&mut self) -> Option<IrId> {
        self.build_const(IrValue::Null)
    }
    
    /// Build addition
    pub fn build_add(&mut self, left: IrId, right: IrId, is_float: bool) -> Option<IrId> {
        let op = if is_float { BinaryOp::FAdd } else { BinaryOp::Add };
        self.build_binop(op, left, right)
    }
    
    /// Build subtraction
    pub fn build_sub(&mut self, left: IrId, right: IrId, is_float: bool) -> Option<IrId> {
        let op = if is_float { BinaryOp::FSub } else { BinaryOp::Sub };
        self.build_binop(op, left, right)
    }
    
    /// Build multiplication
    pub fn build_mul(&mut self, left: IrId, right: IrId, is_float: bool) -> Option<IrId> {
        let op = if is_float { BinaryOp::FMul } else { BinaryOp::Mul };
        self.build_binop(op, left, right)
    }
    
    /// Build division
    pub fn build_div(&mut self, left: IrId, right: IrId, is_float: bool) -> Option<IrId> {
        let op = if is_float { BinaryOp::FDiv } else { BinaryOp::Div };
        self.build_binop(op, left, right)
    }
}

/// Function builder helper for building function signatures
pub struct FunctionSignatureBuilder {
    parameters: Vec<IrParameter>,
    return_type: IrType,
    calling_convention: CallingConvention,
    can_throw: bool,
}

impl FunctionSignatureBuilder {
    pub fn new() -> Self {
        Self {
            parameters: Vec::new(),
            return_type: IrType::Void,
            calling_convention: CallingConvention::Haxe,
            can_throw: false,
        }
    }
    
    pub fn param(mut self, name: String, ty: IrType) -> Self {
        self.parameters.push(IrParameter {
            name,
            ty,
            reg: IrId::new(0), // Will be assigned later
            by_ref: false,
        });
        self
    }
    
    pub fn param_by_ref(mut self, name: String, ty: IrType) -> Self {
        self.parameters.push(IrParameter {
            name,
            ty,
            reg: IrId::new(0), // Will be assigned later
            by_ref: true,
        });
        self
    }
    
    pub fn returns(mut self, ty: IrType) -> Self {
        self.return_type = ty;
        self
    }
    
    pub fn calling_convention(mut self, cc: CallingConvention) -> Self {
        self.calling_convention = cc;
        self
    }
    
    pub fn can_throw(mut self, throws: bool) -> Self {
        self.can_throw = throws;
        self
    }
    
    pub fn build(self) -> IrFunctionSignature {
        IrFunctionSignature {
            parameters: self.parameters,
            return_type: self.return_type,
            calling_convention: self.calling_convention,
            can_throw: self.can_throw,
            type_params: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_basic_function_building() {
        let mut builder = IrBuilder::new("test".to_string(), "test.hx".to_string());
        
        // Build a simple add function
        let sig = FunctionSignatureBuilder::new()
            .param("a".to_string(), IrType::I32)
            .param("b".to_string(), IrType::I32)
            .returns(IrType::I32)
            .build();
            
        let func_id = builder.start_function(SymbolId::from_raw(1), "add".to_string(), sig);
        
        // Get parameter registers
        let a = builder.current_function().unwrap().get_param_reg(0).unwrap();
        let b = builder.current_function().unwrap().get_param_reg(1).unwrap();
        
        // Build add instruction
        let result = builder.build_add(a, b, false).unwrap();
        
        // Return the result
        builder.build_return(Some(result)).unwrap();
        
        builder.finish_function();
        
        // Verify the function
        let func = &builder.module.functions[&func_id];
        assert_eq!(func.name, "add");
        assert_eq!(func.signature.parameters.len(), 2);
        assert_eq!(func.signature.return_type, IrType::I32);
        
        let entry_block = func.cfg.get_block(func.entry_block()).unwrap();
        assert_eq!(entry_block.instructions.len(), 1);
        assert!(matches!(entry_block.terminator, IrTerminator::Return { .. }));
    }
    
    #[test]
    fn test_control_flow_building() {
        let mut builder = IrBuilder::new("test".to_string(), "test.hx".to_string());
        
        let sig = FunctionSignatureBuilder::new()
            .param("x".to_string(), IrType::I32)
            .returns(IrType::I32)
            .build();
            
        builder.start_function(SymbolId::from_raw(1), "abs".to_string(), sig);
        
        let x = builder.current_function().unwrap().get_param_reg(0).unwrap();
        
        // Create blocks
        let negative_block = builder.create_block_with_label("negative".to_string()).unwrap();
        let positive_block = builder.create_block_with_label("positive".to_string()).unwrap();
        let merge_block = builder.create_block_with_label("merge".to_string()).unwrap();
        
        // Build comparison
        let zero = builder.build_int(0, IrType::I32).unwrap();
        let is_negative = builder.build_cmp(CompareOp::Lt, x, zero).unwrap();
        
        // Branch
        builder.build_cond_branch(is_negative, negative_block, positive_block).unwrap();
        
        // Negative block
        builder.switch_to_block(negative_block);
        let neg_x = builder.build_unop(UnaryOp::Neg, x).unwrap();
        builder.build_branch(merge_block).unwrap();
        
        // Positive block
        builder.switch_to_block(positive_block);
        builder.build_branch(merge_block).unwrap();
        
        // Merge block with phi
        builder.switch_to_block(merge_block);
        let phi = builder.build_phi(merge_block, IrType::I32).unwrap();
        builder.add_phi_incoming(merge_block, phi, negative_block, neg_x).unwrap();
        builder.add_phi_incoming(merge_block, phi, positive_block, x).unwrap();
        
        builder.build_return(Some(phi)).unwrap();
        
        builder.finish_function();
    }
}