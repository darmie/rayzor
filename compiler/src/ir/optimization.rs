//! HIR Optimization Passes
//!
//! This module implements various optimization passes for the HIR.
//! Optimizations are organized into passes that can be run independently
//! and in different orders based on optimization level.

use super::{
    IrModule, IrFunction, IrBasicBlock, IrInstruction, IrTerminator,
    IrType, IrId, IrBlockId, IrFunctionId, IrValue, BinaryOp, CompareOp,
};
use std::collections::{HashMap, HashSet};

/// Optimization pass trait
pub trait OptimizationPass {
    /// Get the name of this pass
    fn name(&self) -> &'static str;
    
    /// Run the pass on a module
    fn run_on_module(&mut self, module: &mut IrModule) -> OptimizationResult;
    
    /// Run the pass on a function (default implementation does nothing)
    fn run_on_function(&mut self, _function: &mut IrFunction) -> OptimizationResult {
        OptimizationResult::unchanged()
    }
}

/// Result of an optimization pass
#[derive(Debug, Clone)]
pub struct OptimizationResult {
    /// Whether the IR was modified
    pub modified: bool,
    
    /// Number of instructions eliminated
    pub instructions_eliminated: usize,
    
    /// Number of blocks eliminated
    pub blocks_eliminated: usize,
    
    /// Other statistics
    pub stats: HashMap<String, usize>,
}

impl OptimizationResult {
    /// Create a result indicating no changes
    pub fn unchanged() -> Self {
        Self {
            modified: false,
            instructions_eliminated: 0,
            blocks_eliminated: 0,
            stats: HashMap::new(),
        }
    }
    
    /// Create a result indicating changes
    pub fn changed() -> Self {
        Self {
            modified: true,
            instructions_eliminated: 0,
            blocks_eliminated: 0,
            stats: HashMap::new(),
        }
    }
    
    /// Combine results
    pub fn combine(mut self, other: OptimizationResult) -> Self {
        self.modified |= other.modified;
        self.instructions_eliminated += other.instructions_eliminated;
        self.blocks_eliminated += other.blocks_eliminated;
        
        for (key, value) in other.stats {
            *self.stats.entry(key).or_insert(0) += value;
        }
        
        self
    }
}

/// Optimization pass manager
pub struct PassManager {
    passes: Vec<Box<dyn OptimizationPass>>,
}

impl PassManager {
    /// Create a new pass manager
    pub fn new() -> Self {
        Self {
            passes: Vec::new(),
        }
    }
    
    /// Add a pass to the manager
    pub fn add_pass<P: OptimizationPass + 'static>(&mut self, pass: P) {
        self.passes.push(Box::new(pass));
    }
    
    /// Build a default optimization pipeline
    pub fn default_pipeline() -> Self {
        let mut manager = Self::new();
        
        // Dead code elimination
        manager.add_pass(DeadCodeEliminationPass::new());
        
        // Constant folding
        manager.add_pass(ConstantFoldingPass::new());
        
        // Copy propagation
        manager.add_pass(CopyPropagationPass::new());
        
        // Unreachable block elimination
        manager.add_pass(UnreachableBlockEliminationPass::new());
        
        // Simplify control flow
        manager.add_pass(ControlFlowSimplificationPass::new());
        
        manager
    }
    
    /// Run all passes on a module
    pub fn run(&mut self, module: &mut IrModule) -> OptimizationResult {
        let mut total_result = OptimizationResult::unchanged();
        
        loop {
            let mut changed = false;
            
            for pass in &mut self.passes {
                let result = pass.run_on_module(module);
                if result.modified {
                    changed = true;
                }
                total_result = total_result.combine(result);
            }
            
            if !changed {
                break;
            }
        }
        
        total_result
    }
}

/// Dead code elimination pass
pub struct DeadCodeEliminationPass {
    // Configuration options can go here
}

impl DeadCodeEliminationPass {
    pub fn new() -> Self {
        Self {}
    }
    
    /// Find all used registers in a function
    fn find_used_registers(&self, function: &IrFunction) -> HashSet<IrId> {
        let mut used = HashSet::new();
        
        for block in function.cfg.blocks.values() {
            // Mark phi node uses
            for phi in &block.phi_nodes {
                for &(_, value) in &phi.incoming {
                    used.insert(value);
                }
            }
            
            // Mark instruction uses
            for inst in &block.instructions {
                used.extend(inst.uses());
            }
            
            // Mark terminator uses
            used.extend(terminator_uses(&block.terminator));
        }
        
        used
    }
    
    /// Remove dead instructions from a function
    fn eliminate_dead_instructions(&self, function: &mut IrFunction) -> usize {
        let used = self.find_used_registers(function);
        let mut eliminated = 0;
        
        for block in function.cfg.blocks.values_mut() {
            // Remove dead phi nodes
            block.phi_nodes.retain(|phi| used.contains(&phi.dest));
            
            // Remove dead instructions
            let original_len = block.instructions.len();
            block.instructions.retain(|inst| {
                if let Some(dest) = inst.dest() {
                    used.contains(&dest) || inst.has_side_effects()
                } else {
                    true // Instructions without destinations are kept
                }
            });
            eliminated += original_len - block.instructions.len();
        }
        
        eliminated
    }
}

impl OptimizationPass for DeadCodeEliminationPass {
    fn name(&self) -> &'static str {
        "dead-code-elimination"
    }
    
    fn run_on_module(&mut self, module: &mut IrModule) -> OptimizationResult {
        let mut result = OptimizationResult::unchanged();
        
        for function in module.functions.values_mut() {
            let eliminated = self.eliminate_dead_instructions(function);
            if eliminated > 0 {
                result.modified = true;
                result.instructions_eliminated += eliminated;
            }
        }
        
        result
    }
}

/// Constant folding pass
pub struct ConstantFoldingPass;

impl ConstantFoldingPass {
    pub fn new() -> Self {
        Self
    }
    
    /// Try to fold a binary operation
    fn fold_binary_op(&self, op: BinaryOp, left: &IrValue, right: &IrValue) -> Option<IrValue> {
        use BinaryOp::*;
        use IrValue::*;
        
        match (op, left, right) {
            // Integer arithmetic
            (Add, I32(a), I32(b)) => Some(I32(a.wrapping_add(*b))),
            (Sub, I32(a), I32(b)) => Some(I32(a.wrapping_sub(*b))),
            (Mul, I32(a), I32(b)) => Some(I32(a.wrapping_mul(*b))),
            (Div, I32(a), I32(b)) if *b != 0 => Some(I32(a / b)),
            (Rem, I32(a), I32(b)) if *b != 0 => Some(I32(a % b)),
            
            // Floating point arithmetic
            (FAdd, F64(a), F64(b)) => Some(F64(a + b)),
            (FSub, F64(a), F64(b)) => Some(F64(a - b)),
            (FMul, F64(a), F64(b)) => Some(F64(a * b)),
            (FDiv, F64(a), F64(b)) if *b != 0.0 => Some(F64(a / b)),
            
            // Bitwise operations
            (And, I32(a), I32(b)) => Some(I32(a & b)),
            (Or, I32(a), I32(b)) => Some(I32(a | b)),
            (Xor, I32(a), I32(b)) => Some(I32(a ^ b)),
            (Shl, I32(a), I32(b)) if *b >= 0 && *b < 32 => Some(I32(a << b)),
            (Shr, I32(a), I32(b)) if *b >= 0 && *b < 32 => Some(I32(a >> b)),
            
            _ => None,
        }
    }
    
    /// Try to fold a comparison
    fn fold_comparison(&self, op: CompareOp, left: &IrValue, right: &IrValue) -> Option<IrValue> {
        use CompareOp::*;
        use IrValue::*;
        
        match (op, left, right) {
            // Integer comparisons
            (Eq, I32(a), I32(b)) => Some(Bool(a == b)),
            (Ne, I32(a), I32(b)) => Some(Bool(a != b)),
            (Lt, I32(a), I32(b)) => Some(Bool(a < b)),
            (Le, I32(a), I32(b)) => Some(Bool(a <= b)),
            (Gt, I32(a), I32(b)) => Some(Bool(a > b)),
            (Ge, I32(a), I32(b)) => Some(Bool(a >= b)),
            
            // Floating point comparisons
            (FEq, F64(a), F64(b)) => Some(Bool((a - b).abs() < f64::EPSILON)),
            (FNe, F64(a), F64(b)) => Some(Bool((a - b).abs() >= f64::EPSILON)),
            (FLt, F64(a), F64(b)) => Some(Bool(a < b)),
            (FLe, F64(a), F64(b)) => Some(Bool(a <= b)),
            (FGt, F64(a), F64(b)) => Some(Bool(a > b)),
            (FGe, F64(a), F64(b)) => Some(Bool(a >= b)),
            
            // Boolean comparisons
            (Eq, Bool(a), Bool(b)) => Some(Bool(a == b)),
            (Ne, Bool(a), Bool(b)) => Some(Bool(a != b)),
            
            _ => None,
        }
    }
}

impl OptimizationPass for ConstantFoldingPass {
    fn name(&self) -> &'static str {
        "constant-folding"
    }
    
    fn run_on_module(&mut self, module: &mut IrModule) -> OptimizationResult {
        let mut result = OptimizationResult::unchanged();
        
        // Build constant value map
        let mut constants: HashMap<IrId, IrValue> = HashMap::new();
        
        for function in module.functions.values_mut() {
            constants.clear();
            
            // First pass: collect constants
            for block in function.cfg.blocks.values() {
                for inst in &block.instructions {
                    if let IrInstruction::Const { dest, value } = inst {
                        constants.insert(*dest, value.clone());
                    }
                }
            }
            
            // Second pass: fold operations
            for block in function.cfg.blocks.values_mut() {
                for inst in &mut block.instructions {
                    match inst {
                        IrInstruction::BinOp { dest, op, left, right } => {
                            let dest_reg = *dest;
                            let op_val = *op;
                            let left_reg = *left;
                            let right_reg = *right;
                            if let (Some(left_val), Some(right_val)) = 
                                (constants.get(&left_reg), constants.get(&right_reg)) {
                                if let Some(folded) = self.fold_binary_op(op_val, left_val, right_val) {
                                    // Replace with constant
                                    *inst = IrInstruction::Const { dest: dest_reg, value: folded.clone() };
                                    constants.insert(dest_reg, folded);
                                    result.modified = true;
                                }
                            }
                        }
                        IrInstruction::Cmp { dest, op, left, right } => {
                            let dest_reg = *dest;
                            let op_val = *op;
                            let left_reg = *left;
                            let right_reg = *right;
                            if let (Some(left_val), Some(right_val)) = 
                                (constants.get(&left_reg), constants.get(&right_reg)) {
                                if let Some(folded) = self.fold_comparison(op_val, left_val, right_val) {
                                    // Replace with constant
                                    *inst = IrInstruction::Const { dest: dest_reg, value: folded.clone() };
                                    constants.insert(dest_reg, folded);
                                    result.modified = true;
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        
        result
    }
}

/// Copy propagation pass
pub struct CopyPropagationPass;

impl CopyPropagationPass {
    pub fn new() -> Self {
        Self
    }
}

impl OptimizationPass for CopyPropagationPass {
    fn name(&self) -> &'static str {
        "copy-propagation"
    }
    
    fn run_on_module(&mut self, module: &mut IrModule) -> OptimizationResult {
        let mut result = OptimizationResult::unchanged();
        
        for function in module.functions.values_mut() {
            let mut copies: HashMap<IrId, IrId> = HashMap::new();
            
            // Find copy instructions
            for block in function.cfg.blocks.values() {
                for inst in &block.instructions {
                    if let IrInstruction::Copy { dest, src } = inst {
                        copies.insert(*dest, *src);
                    }
                }
            }
            
            if !copies.is_empty() {
                // Replace uses with original sources
                for block in function.cfg.blocks.values_mut() {
                    for inst in &mut block.instructions {
                        inst.replace_uses(&copies);
                    }
                    
                    // Replace terminator uses
                    replace_terminator_uses(&mut block.terminator, &copies);
                }
                
                result.modified = true;
            }
        }
        
        result
    }
}

/// Unreachable block elimination pass
pub struct UnreachableBlockEliminationPass;

impl UnreachableBlockEliminationPass {
    pub fn new() -> Self {
        Self
    }
    
    /// Find reachable blocks from entry
    fn find_reachable(&self, function: &IrFunction) -> HashSet<IrBlockId> {
        let mut reachable = HashSet::new();
        let mut worklist = vec![function.entry_block()];
        
        while let Some(block_id) = worklist.pop() {
            if reachable.insert(block_id) {
                if let Some(block) = function.cfg.get_block(block_id) {
                    worklist.extend(block.successors());
                }
            }
        }
        
        reachable
    }
}

impl OptimizationPass for UnreachableBlockEliminationPass {
    fn name(&self) -> &'static str {
        "unreachable-block-elimination"
    }
    
    fn run_on_module(&mut self, module: &mut IrModule) -> OptimizationResult {
        let mut result = OptimizationResult::unchanged();
        
        for function in module.functions.values_mut() {
            let reachable = self.find_reachable(function);
            let original_count = function.cfg.blocks.len();
            
            // Remove unreachable blocks
            function.cfg.blocks.retain(|&id, _| reachable.contains(&id));
            
            let eliminated = original_count - function.cfg.blocks.len();
            if eliminated > 0 {
                result.modified = true;
                result.blocks_eliminated += eliminated;
            }
        }
        
        result
    }
}

/// Control flow simplification pass
pub struct ControlFlowSimplificationPass;

impl ControlFlowSimplificationPass {
    pub fn new() -> Self {
        Self
    }
    
    /// Simplify branches with constant conditions
    fn simplify_conditional_branches(&self, function: &mut IrFunction) -> bool {
        let mut modified = false;
        
        // Collect constant values
        let mut constants: HashMap<IrId, IrValue> = HashMap::new();
        for block in function.cfg.blocks.values() {
            for inst in &block.instructions {
                if let IrInstruction::Const { dest, value } = inst {
                    constants.insert(*dest, value.clone());
                }
            }
        }
        
        // Simplify conditional branches
        for block in function.cfg.blocks.values_mut() {
            if let IrTerminator::CondBranch { condition, true_target, false_target } = &block.terminator {
                if let Some(IrValue::Bool(cond_val)) = constants.get(condition) {
                    // Replace with unconditional branch
                    let target = if *cond_val { *true_target } else { *false_target };
                    block.terminator = IrTerminator::Branch { target };
                    modified = true;
                }
            }
        }
        
        modified
    }
}

impl OptimizationPass for ControlFlowSimplificationPass {
    fn name(&self) -> &'static str {
        "control-flow-simplification"
    }
    
    fn run_on_module(&mut self, module: &mut IrModule) -> OptimizationResult {
        let mut result = OptimizationResult::unchanged();
        
        for function in module.functions.values_mut() {
            if self.simplify_conditional_branches(function) {
                result.modified = true;
            }
        }
        
        result
    }
}

// Helper functions

/// Get registers used by a terminator
fn terminator_uses(term: &IrTerminator) -> Vec<IrId> {
    match term {
        IrTerminator::CondBranch { condition, .. } => vec![*condition],
        IrTerminator::Switch { value, .. } => vec![*value],
        IrTerminator::Return { value: Some(val) } => vec![*val],
        IrTerminator::NoReturn { call } => vec![*call],
        _ => Vec::new(),
    }
}

/// Replace register uses in a terminator
fn replace_terminator_uses(term: &mut IrTerminator, replacements: &HashMap<IrId, IrId>) {
    match term {
        IrTerminator::CondBranch { condition, .. } => {
            if let Some(&new_reg) = replacements.get(condition) {
                *condition = new_reg;
            }
        }
        IrTerminator::Switch { value, .. } => {
            if let Some(&new_reg) = replacements.get(value) {
                *value = new_reg;
            }
        }
        IrTerminator::Return { value: Some(val) } => {
            if let Some(&new_reg) = replacements.get(val) {
                *val = new_reg;
            }
        }
        IrTerminator::NoReturn { call } => {
            if let Some(&new_reg) = replacements.get(call) {
                *call = new_reg;
            }
        }
        _ => {}
    }
}

// Extension trait for instruction manipulation
trait InstructionExt {
    fn uses(&self) -> Vec<IrId>;
    fn dest(&self) -> Option<IrId>;
    fn has_side_effects(&self) -> bool;
    fn replace_uses(&mut self, replacements: &HashMap<IrId, IrId>);
}

impl InstructionExt for IrInstruction {
    fn uses(&self) -> Vec<IrId> {
        match self {
            IrInstruction::Copy { src, .. } => vec![*src],
            IrInstruction::Load { ptr, .. } => vec![*ptr],
            IrInstruction::Store { ptr, value } => vec![*ptr, *value],
            IrInstruction::BinOp { left, right, .. } => vec![*left, *right],
            IrInstruction::UnOp { operand, .. } => vec![*operand],
            IrInstruction::Cmp { left, right, .. } => vec![*left, *right],
            IrInstruction::Call { func, args, .. } => {
                let mut uses = vec![*func];
                uses.extend(args.iter().copied());
                uses
            }
            IrInstruction::Cast { src, .. } => vec![*src],
            IrInstruction::Select { condition, true_val, false_val, .. } => {
                vec![*condition, *true_val, *false_val]
            }
            _ => Vec::new(),
        }
    }
    
    fn dest(&self) -> Option<IrId> {
        match self {
            IrInstruction::Const { dest, .. } |
            IrInstruction::Copy { dest, .. } |
            IrInstruction::Load { dest, .. } |
            IrInstruction::BinOp { dest, .. } |
            IrInstruction::UnOp { dest, .. } |
            IrInstruction::Cmp { dest, .. } |
            IrInstruction::Call { dest: Some(dest), .. } |
            IrInstruction::Cast { dest, .. } |
            IrInstruction::Select { dest, .. } => Some(*dest),
            _ => None,
        }
    }
    
    fn has_side_effects(&self) -> bool {
        match self {
            IrInstruction::Store { .. } |
            IrInstruction::Call { .. } |
            IrInstruction::Throw { .. } => true,
            _ => false,
        }
    }
    
    fn replace_uses(&mut self, replacements: &HashMap<IrId, IrId>) {
        match self {
            IrInstruction::Copy { src, .. } => {
                if let Some(&new_reg) = replacements.get(src) {
                    *src = new_reg;
                }
            }
            IrInstruction::Load { ptr, .. } => {
                if let Some(&new_reg) = replacements.get(ptr) {
                    *ptr = new_reg;
                }
            }
            IrInstruction::Store { ptr, value } => {
                if let Some(&new_reg) = replacements.get(ptr) {
                    *ptr = new_reg;
                }
                if let Some(&new_reg) = replacements.get(value) {
                    *value = new_reg;
                }
            }
            IrInstruction::BinOp { left, right, .. } => {
                if let Some(&new_reg) = replacements.get(left) {
                    *left = new_reg;
                }
                if let Some(&new_reg) = replacements.get(right) {
                    *right = new_reg;
                }
            }
            // TODO: Add more cases as needed
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::builder::*;
    use crate::tast::SymbolId;
    
    #[test]
    fn test_constant_folding() {
        let mut builder = IrBuilder::new("test".to_string(), "test.hx".to_string());
        
        let sig = FunctionSignatureBuilder::new().returns(IrType::I32).build();
        builder.start_function(SymbolId::from_raw(1), "test".to_string(), sig);
        
        // Build: 2 + 3
        let two = builder.build_int(2, IrType::I32).unwrap();
        let three = builder.build_int(3, IrType::I32).unwrap();
        let result = builder.build_add(two, three, false).unwrap();
        builder.build_return(Some(result));
        
        builder.finish_function();
        
        // Run constant folding
        let mut pass = ConstantFoldingPass::new();
        let opt_result = pass.run_on_module(&mut builder.module);
        
        assert!(opt_result.modified);
    }
    
    #[test]
    fn test_dead_code_elimination() {
        let mut builder = IrBuilder::new("test".to_string(), "test.hx".to_string());
        
        let sig = FunctionSignatureBuilder::new().returns(IrType::I32).build();
        builder.start_function(SymbolId::from_raw(1), "test".to_string(), sig);
        
        // Create dead code
        let _dead = builder.build_int(42, IrType::I32).unwrap(); // Not used
        
        let live = builder.build_int(10, IrType::I32).unwrap();
        builder.build_return(Some(live));
        
        builder.finish_function();
        
        // Run DCE
        let mut pass = DeadCodeEliminationPass::new();
        let opt_result = pass.run_on_module(&mut builder.module);
        
        assert!(opt_result.modified);
        assert!(opt_result.instructions_eliminated > 0);
    }
}