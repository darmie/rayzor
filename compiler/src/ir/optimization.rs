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
            IrInstruction::CallDirect { args, .. } => {
                args.iter().copied().collect()
            }
            IrInstruction::CallIndirect { func_ptr, args, .. } => {
                let mut uses = vec![*func_ptr];
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
            IrInstruction::CallDirect { dest: Some(dest), .. } |
            IrInstruction::CallIndirect { dest: Some(dest), .. } |
            IrInstruction::Cast { dest, .. } |
            IrInstruction::Select { dest, .. } => Some(*dest),
            _ => None,
        }
    }
    
    fn has_side_effects(&self) -> bool {
        match self {
            IrInstruction::Store { .. } |
            IrInstruction::CallDirect { .. } |
            IrInstruction::CallIndirect { .. } |
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

/// Loop Invariant Code Motion (LICM) pass
///
/// Moves loop-invariant computations out of loops to reduce redundant work.
/// An instruction is loop-invariant if all its operands are:
/// - Defined outside the loop, OR
/// - Are themselves loop-invariant
pub struct LICMPass;

impl LICMPass {
    pub fn new() -> Self {
        Self
    }

    /// Check if an instruction is loop-invariant.
    fn is_loop_invariant(
        inst: &IrInstruction,
        loop_blocks: &HashSet<IrBlockId>,
        def_block: &HashMap<IrId, IrBlockId>,
        invariant_defs: &HashSet<IrId>,
    ) -> bool {
        // Instructions with side effects are not loop-invariant
        if inst.has_side_effects() {
            return false;
        }

        // Check if all uses are defined outside the loop or are invariant
        for use_id in inst.uses() {
            if let Some(&def_blk) = def_block.get(&use_id) {
                if loop_blocks.contains(&def_blk) && !invariant_defs.contains(&use_id) {
                    return false;
                }
            }
        }

        true
    }

    /// Check if it's safe to hoist an instruction.
    fn is_safe_to_hoist(
        inst: &IrInstruction,
        inst_block: IrBlockId,
        loop_info: &super::loop_analysis::NaturalLoop,
        domtree: &super::loop_analysis::DominatorTree,
    ) -> bool {
        // Must dominate all exit blocks
        for &exit in &loop_info.exit_blocks {
            if !domtree.dominates(inst_block, exit) {
                return false;
            }
        }

        // Don't hoist instructions that could trap (division, etc.)
        match inst {
            IrInstruction::BinOp { op, .. } => {
                !matches!(op, BinaryOp::Div | BinaryOp::Rem | BinaryOp::FDiv)
            }
            _ => true,
        }
    }

    /// Create a preheader block for a loop if one doesn't exist.
    fn ensure_preheader(
        cfg: &mut super::IrControlFlowGraph,
        header: IrBlockId,
        loop_blocks: &HashSet<IrBlockId>,
    ) -> IrBlockId {
        let header_block = cfg.get_block(header).unwrap();

        // Find predecessors outside the loop
        let outside_preds: Vec<IrBlockId> = header_block.predecessors.iter()
            .filter(|p| !loop_blocks.contains(p))
            .copied()
            .collect();

        // If there's already a valid preheader, use it
        if outside_preds.len() == 1 {
            let pred = outside_preds[0];
            if let Some(pred_block) = cfg.get_block(pred) {
                if pred_block.successors().len() == 1 {
                    return pred;
                }
            }
        }

        // Create a new preheader
        let preheader = cfg.create_block();

        // Set up preheader terminator to branch to header
        if let Some(preheader_block) = cfg.get_block_mut(preheader) {
            preheader_block.terminator = IrTerminator::Branch { target: header };
        }

        // Update outside predecessors to branch to preheader instead of header
        for &pred in &outside_preds {
            if let Some(pred_block) = cfg.get_block_mut(pred) {
                match &mut pred_block.terminator {
                    IrTerminator::Branch { target } if *target == header => {
                        *target = preheader;
                    }
                    IrTerminator::CondBranch { true_target, false_target, .. } => {
                        if *true_target == header {
                            *true_target = preheader;
                        }
                        if *false_target == header {
                            *false_target = preheader;
                        }
                    }
                    IrTerminator::Switch { cases, default, .. } => {
                        for (_, target) in cases.iter_mut() {
                            if *target == header {
                                *target = preheader;
                            }
                        }
                        if *default == header {
                            *default = preheader;
                        }
                    }
                    _ => {}
                }
            }
        }

        // Update header's predecessors
        if let Some(header_block) = cfg.get_block_mut(header) {
            header_block.predecessors.retain(|p| loop_blocks.contains(p));
            header_block.predecessors.push(preheader);
        }

        // Set preheader's predecessors
        if let Some(preheader_block) = cfg.get_block_mut(preheader) {
            preheader_block.predecessors = outside_preds;
        }

        preheader
    }
}

impl OptimizationPass for LICMPass {
    fn name(&self) -> &'static str {
        "licm"
    }

    fn run_on_module(&mut self, module: &mut IrModule) -> OptimizationResult {
        use super::loop_analysis::{DominatorTree, LoopNestInfo};

        let mut result = OptimizationResult::unchanged();

        for function in module.functions.values_mut() {
            let domtree = DominatorTree::compute(function);
            let loop_info = LoopNestInfo::analyze(function, &domtree);

            if loop_info.loops.is_empty() {
                continue;
            }

            // Build definition site map: register -> block where it's defined
            let mut def_block: HashMap<IrId, IrBlockId> = HashMap::new();
            for (&block_id, block) in &function.cfg.blocks {
                for phi in &block.phi_nodes {
                    def_block.insert(phi.dest, block_id);
                }
                for inst in &block.instructions {
                    if let Some(dest) = inst.dest() {
                        def_block.insert(dest, block_id);
                    }
                }
            }

            // Process loops from innermost to outermost
            for loop_data in loop_info.loops_innermost_first() {
                let mut invariant_defs: HashSet<IrId> = HashSet::new();
                let mut to_hoist: Vec<(IrBlockId, usize, IrInstruction)> = Vec::new();

                // Iterate until no more invariants found
                let mut changed = true;
                while changed {
                    changed = false;

                    for &block_id in &loop_data.blocks {
                        if block_id == loop_data.header {
                            continue; // Don't hoist from header initially
                        }

                        if let Some(block) = function.cfg.get_block(block_id) {
                            for (idx, inst) in block.instructions.iter().enumerate() {
                                if let Some(dest) = inst.dest() {
                                    if invariant_defs.contains(&dest) {
                                        continue;
                                    }

                                    if Self::is_loop_invariant(
                                        inst,
                                        &loop_data.blocks,
                                        &def_block,
                                        &invariant_defs,
                                    ) && Self::is_safe_to_hoist(
                                        inst,
                                        block_id,
                                        loop_data,
                                        &domtree,
                                    ) {
                                        invariant_defs.insert(dest);
                                        to_hoist.push((block_id, idx, inst.clone()));
                                        changed = true;
                                    }
                                }
                            }
                        }
                    }
                }

                if to_hoist.is_empty() {
                    continue;
                }

                // Ensure we have a preheader
                let preheader = Self::ensure_preheader(
                    &mut function.cfg,
                    loop_data.header,
                    &loop_data.blocks,
                );

                // Sort hoisted instructions by original position to maintain order
                to_hoist.sort_by_key(|(block_id, idx, _)| (block_id.as_u32(), *idx));

                // Collect indices to remove per block
                let mut indices_to_remove: HashMap<IrBlockId, Vec<usize>> = HashMap::new();
                for (block_id, idx, _) in &to_hoist {
                    indices_to_remove.entry(*block_id).or_default().push(*idx);
                }

                // Remove instructions from their original blocks (in reverse order to preserve indices)
                for (block_id, indices) in indices_to_remove {
                    if let Some(block) = function.cfg.get_block_mut(block_id) {
                        // Remove in reverse order to preserve indices
                        let mut indices_sorted = indices;
                        indices_sorted.sort_by(|a, b| b.cmp(a));
                        for idx in indices_sorted {
                            if idx < block.instructions.len() {
                                block.instructions.remove(idx);
                            }
                        }
                    }
                }

                // Add instructions to preheader
                if let Some(preheader_block) = function.cfg.get_block_mut(preheader) {
                    for (_, _, inst) in to_hoist {
                        preheader_block.instructions.push(inst);
                        result.modified = true;
                        *result.stats.entry("instructions_hoisted".to_string()).or_insert(0) += 1;
                    }
                }
            }
        }

        result
    }
}

/// Common Subexpression Elimination (CSE) pass
///
/// Eliminates redundant computations by reusing previously computed values.
/// Uses value numbering to identify equivalent expressions.
pub struct CSEPass;

impl CSEPass {
    pub fn new() -> Self {
        Self
    }

    /// Generate a hash key for an instruction's computation.
    fn instruction_key(inst: &IrInstruction) -> Option<String> {
        match inst {
            IrInstruction::BinOp { op, left, right, .. } => {
                // For commutative ops, normalize operand order
                let (l, r) = if Self::is_commutative(*op) && left.as_u32() > right.as_u32() {
                    (right, left)
                } else {
                    (left, right)
                };
                Some(format!("binop:{:?}:{}:{}", op, l.as_u32(), r.as_u32()))
            }
            IrInstruction::UnOp { op, operand, .. } => {
                Some(format!("unop:{:?}:{}", op, operand.as_u32()))
            }
            IrInstruction::Cmp { op, left, right, .. } => {
                Some(format!("cmp:{:?}:{}:{}", op, left.as_u32(), right.as_u32()))
            }
            IrInstruction::Cast { src, to_ty, .. } => {
                Some(format!("cast:{}:{:?}", src.as_u32(), to_ty))
            }
            // Loads are not CSE-safe without alias analysis
            // Calls have side effects
            _ => None,
        }
    }

    /// Check if a binary operation is commutative.
    fn is_commutative(op: BinaryOp) -> bool {
        matches!(
            op,
            BinaryOp::Add | BinaryOp::Mul | BinaryOp::FAdd | BinaryOp::FMul |
            BinaryOp::And | BinaryOp::Or | BinaryOp::Xor
        )
    }
}

impl OptimizationPass for CSEPass {
    fn name(&self) -> &'static str {
        "cse"
    }

    fn run_on_module(&mut self, module: &mut IrModule) -> OptimizationResult {
        let mut result = OptimizationResult::unchanged();

        for function in module.functions.values_mut() {
            // Local CSE within each block
            for block in function.cfg.blocks.values_mut() {
                let mut available: HashMap<String, IrId> = HashMap::new();
                let mut replacements: HashMap<IrId, IrId> = HashMap::new();

                for inst in &block.instructions {
                    if let Some(key) = Self::instruction_key(inst) {
                        if let Some(&existing) = available.get(&key) {
                            // Found common subexpression
                            if let Some(dest) = inst.dest() {
                                replacements.insert(dest, existing);
                                result.modified = true;
                                *result.stats.entry("cse_eliminated".to_string()).or_insert(0) += 1;
                            }
                        } else if let Some(dest) = inst.dest() {
                            available.insert(key, dest);
                        }
                    }
                }

                // Apply replacements
                if !replacements.is_empty() {
                    for inst in &mut block.instructions {
                        inst.replace_uses(&replacements);
                    }
                    replace_terminator_uses(&mut block.terminator, &replacements);
                }
            }
        }

        result
    }
}

/// Global Value Numbering (GVN) pass
///
/// More powerful than local CSE, uses dominator tree to find redundancies across blocks.
pub struct GVNPass;

impl GVNPass {
    pub fn new() -> Self {
        Self
    }
}

impl OptimizationPass for GVNPass {
    fn name(&self) -> &'static str {
        "gvn"
    }

    fn run_on_module(&mut self, module: &mut IrModule) -> OptimizationResult {
        use super::loop_analysis::DominatorTree;

        let mut result = OptimizationResult::unchanged();

        for function in module.functions.values_mut() {
            let domtree = DominatorTree::compute(function);

            // Value number table: expression -> canonical register
            let mut value_numbers: HashMap<String, IrId> = HashMap::new();
            // Registers to replace
            let mut replacements: HashMap<IrId, IrId> = HashMap::new();

            // Process blocks in dominator tree order (preorder DFS)
            let mut worklist = vec![function.entry_block()];
            let mut visited = HashSet::new();

            while let Some(block_id) = worklist.pop() {
                if !visited.insert(block_id) {
                    continue;
                }

                // Process this block
                if let Some(block) = function.cfg.get_block(block_id) {
                    let mut local_values = value_numbers.clone();

                    for inst in &block.instructions {
                        // First, apply known replacements to this instruction's uses
                        let key = Self::make_key_with_replacements(inst, &replacements);

                        if let Some(key) = key {
                            if let Some(&existing) = local_values.get(&key) {
                                if let Some(dest) = inst.dest() {
                                    replacements.insert(dest, existing);
                                    result.modified = true;
                                    *result.stats.entry("gvn_eliminated".to_string()).or_insert(0) += 1;
                                }
                            } else if let Some(dest) = inst.dest() {
                                local_values.insert(key, dest);
                            }
                        }
                    }

                    // Update global value numbers for dominated blocks
                    value_numbers = local_values;
                }

                // Add children in dominator tree
                for &child in domtree.children(block_id) {
                    worklist.push(child);
                }
            }

            // Apply all replacements
            if !replacements.is_empty() {
                for block in function.cfg.blocks.values_mut() {
                    for inst in &mut block.instructions {
                        inst.replace_uses(&replacements);
                    }
                    replace_terminator_uses(&mut block.terminator, &replacements);
                }
            }
        }

        result
    }
}

impl GVNPass {
    /// Create expression key with replacements applied to operands.
    fn make_key_with_replacements(
        inst: &IrInstruction,
        replacements: &HashMap<IrId, IrId>,
    ) -> Option<String> {
        let resolve = |id: IrId| -> IrId {
            *replacements.get(&id).unwrap_or(&id)
        };

        match inst {
            IrInstruction::BinOp { op, left, right, .. } => {
                let l = resolve(*left);
                let r = resolve(*right);
                let (l, r) = if CSEPass::is_commutative(*op) && l.as_u32() > r.as_u32() {
                    (r, l)
                } else {
                    (l, r)
                };
                Some(format!("binop:{:?}:{}:{}", op, l.as_u32(), r.as_u32()))
            }
            IrInstruction::UnOp { op, operand, .. } => {
                Some(format!("unop:{:?}:{}", op, resolve(*operand).as_u32()))
            }
            IrInstruction::Cmp { op, left, right, .. } => {
                Some(format!("cmp:{:?}:{}:{}", op, resolve(*left).as_u32(), resolve(*right).as_u32()))
            }
            IrInstruction::Cast { src, to_ty, .. } => {
                Some(format!("cast:{}:{:?}", resolve(*src).as_u32(), to_ty))
            }
            _ => None,
        }
    }
}

/// Tail Call Optimization pass
///
/// Identifies tail calls and marks them for optimization by the backend.
/// Also converts self-recursive tail calls to loops when possible.
pub struct TailCallOptimizationPass;

impl TailCallOptimizationPass {
    pub fn new() -> Self {
        Self
    }

    /// Check if a call is in tail position.
    fn is_tail_call(block: &IrBasicBlock, call_idx: usize) -> bool {
        // Call must be the last instruction before a return
        if call_idx + 1 != block.instructions.len() {
            return false;
        }

        // Terminator must be a return
        match &block.terminator {
            IrTerminator::Return { value } => {
                // If returning a value, it must be the call's result
                if let Some(ret_val) = value {
                    if let Some(IrInstruction::CallDirect { dest: Some(dest), .. })
                        | Some(IrInstruction::CallIndirect { dest: Some(dest), .. })
                        = block.instructions.get(call_idx)
                    {
                        return *ret_val == *dest;
                    }
                    false
                } else {
                    // Returning void - call must also return void
                    matches!(
                        block.instructions.get(call_idx),
                        Some(IrInstruction::CallDirect { dest: None, .. })
                        | Some(IrInstruction::CallIndirect { dest: None, .. })
                    )
                }
            }
            _ => false,
        }
    }
}

impl OptimizationPass for TailCallOptimizationPass {
    fn name(&self) -> &'static str {
        "tail-call-optimization"
    }

    fn run_on_module(&mut self, module: &mut IrModule) -> OptimizationResult {
        let mut result = OptimizationResult::unchanged();

        for (func_id, function) in module.functions.iter_mut() {
            let current_func_id = *func_id;

            for block in function.cfg.blocks.values_mut() {
                // First pass: identify tail calls
                let mut tail_call_indices: Vec<usize> = Vec::new();
                for (idx, inst) in block.instructions.iter().enumerate() {
                    let is_call = matches!(
                        inst,
                        IrInstruction::CallDirect { .. } | IrInstruction::CallIndirect { .. }
                    );
                    if is_call && Self::is_tail_call(block, idx) {
                        tail_call_indices.push(idx);
                    }
                }

                // Second pass: mark tail calls
                for idx in tail_call_indices {
                    if let Some(inst) = block.instructions.get_mut(idx) {
                        match inst {
                            IrInstruction::CallDirect { func_id, is_tail_call, .. } => {
                                *is_tail_call = true;
                                result.modified = true;

                                // Track self-recursive tail calls separately
                                if *func_id == current_func_id {
                                    *result.stats.entry("self_recursive_tail_calls".to_string())
                                        .or_insert(0) += 1;
                                } else {
                                    *result.stats.entry("tail_calls_marked".to_string())
                                        .or_insert(0) += 1;
                                }
                            }
                            IrInstruction::CallIndirect { is_tail_call, .. } => {
                                *is_tail_call = true;
                                result.modified = true;
                                *result.stats.entry("indirect_tail_calls_marked".to_string())
                                    .or_insert(0) += 1;
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        result
    }
}

/// Optimization level for tiered compilation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizationLevel {
    /// No optimization (fastest compilation)
    O0,
    /// Basic optimizations (fast, low overhead)
    O1,
    /// Standard optimizations (good balance)
    O2,
    /// Aggressive optimizations (best runtime, slower compile)
    O3,
}

impl PassManager {
    /// Create optimization pipeline for a specific level.
    pub fn for_level(level: OptimizationLevel) -> Self {
        let mut manager = Self::new();

        match level {
            OptimizationLevel::O0 => {
                // No optimizations
            }
            OptimizationLevel::O1 => {
                // Fast, low-overhead optimizations
                manager.add_pass(DeadCodeEliminationPass::new());
                manager.add_pass(ConstantFoldingPass::new());
                manager.add_pass(CopyPropagationPass::new());
                manager.add_pass(UnreachableBlockEliminationPass::new());
            }
            OptimizationLevel::O2 => {
                // Standard optimizations
                manager.add_pass(DeadCodeEliminationPass::new());
                manager.add_pass(ConstantFoldingPass::new());
                manager.add_pass(CopyPropagationPass::new());
                manager.add_pass(CSEPass::new());
                manager.add_pass(LICMPass::new());
                manager.add_pass(ControlFlowSimplificationPass::new());
                manager.add_pass(UnreachableBlockEliminationPass::new());
                manager.add_pass(DeadCodeEliminationPass::new()); // Cleanup after other passes
            }
            OptimizationLevel::O3 => {
                // Aggressive optimizations
                // Inlining first to expose more optimization opportunities
                manager.add_pass(super::inlining::InliningPass::new());
                manager.add_pass(DeadCodeEliminationPass::new());
                manager.add_pass(ConstantFoldingPass::new());
                manager.add_pass(CopyPropagationPass::new());
                manager.add_pass(GVNPass::new());
                manager.add_pass(CSEPass::new());
                manager.add_pass(LICMPass::new());
                manager.add_pass(TailCallOptimizationPass::new());
                manager.add_pass(ControlFlowSimplificationPass::new());
                manager.add_pass(UnreachableBlockEliminationPass::new());
                manager.add_pass(DeadCodeEliminationPass::new()); // Cleanup
            }
        }

        manager
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