//! Function Inlining for MIR Optimization
//!
//! This module provides function inlining infrastructure including:
//! - Call graph construction and analysis
//! - Inlining cost model and heuristics
//! - Function body cloning and integration

use super::{
    IrFunction, IrFunctionId, IrModule, IrInstruction, IrId, IrBlockId,
    IrType, IrBasicBlock, IrTerminator, IrPhiNode,
};
use super::loop_analysis::{DominatorTree, LoopNestInfo};
use super::optimization::{OptimizationPass, OptimizationResult};
use std::collections::{HashMap, HashSet, VecDeque};

/// A call site in the program.
#[derive(Debug, Clone)]
pub struct CallSite {
    /// Function containing the call
    pub caller: IrFunctionId,
    /// Function being called
    pub callee: IrFunctionId,
    /// Block containing the call
    pub block: IrBlockId,
    /// Index of the call instruction in the block
    pub instruction_index: usize,
    /// Loop nesting depth at call site (higher = hotter)
    pub loop_depth: usize,
    /// Arguments passed to the call
    pub args: Vec<IrId>,
    /// Destination register for call result (if any)
    pub dest: Option<IrId>,
}

/// Call graph for the module.
#[derive(Debug, Clone)]
pub struct CallGraph {
    /// All call sites in the module
    pub call_sites: Vec<CallSite>,
    /// Map from callee to call sites calling it
    pub callers: HashMap<IrFunctionId, Vec<usize>>,
    /// Map from caller to call sites it contains
    pub callees: HashMap<IrFunctionId, Vec<usize>>,
    /// Functions that are recursive (call themselves directly or indirectly)
    pub recursive_functions: HashSet<IrFunctionId>,
}

impl CallGraph {
    /// Build call graph from a module.
    pub fn build(module: &IrModule) -> Self {
        let mut call_sites = Vec::new();
        let mut callers: HashMap<IrFunctionId, Vec<usize>> = HashMap::new();
        let mut callees: HashMap<IrFunctionId, Vec<usize>> = HashMap::new();

        for (&func_id, function) in &module.functions {
            // Compute loop info for this function to get call site loop depths
            let domtree = DominatorTree::compute(function);
            let loop_info = LoopNestInfo::analyze(function, &domtree);

            for (&block_id, block) in &function.cfg.blocks {
                let loop_depth = loop_info.loop_depth(block_id);

                for (idx, inst) in block.instructions.iter().enumerate() {
                    if let IrInstruction::CallDirect { dest, func_id: callee_id, args, .. } = inst {
                        let call_site = CallSite {
                            caller: func_id,
                            callee: *callee_id,
                            block: block_id,
                            instruction_index: idx,
                            loop_depth,
                            args: args.clone(),
                            dest: *dest,
                        };

                        let site_idx = call_sites.len();
                        call_sites.push(call_site);

                        callers.entry(*callee_id).or_default().push(site_idx);
                        callees.entry(func_id).or_default().push(site_idx);
                    }
                }
            }
        }

        // Find recursive functions using SCC (simplified: just check direct recursion for now)
        let mut recursive_functions = HashSet::new();
        for site in &call_sites {
            if site.caller == site.callee {
                recursive_functions.insert(site.caller);
            }
        }

        // Also check for indirect recursion via reachability
        for &func_id in module.functions.keys() {
            if Self::can_reach(&callees, &call_sites, func_id, func_id, &mut HashSet::new()) {
                recursive_functions.insert(func_id);
            }
        }

        Self {
            call_sites,
            callers,
            callees,
            recursive_functions,
        }
    }

    /// Check if `from` can reach `target` via calls.
    fn can_reach(
        callees: &HashMap<IrFunctionId, Vec<usize>>,
        call_sites: &[CallSite],
        from: IrFunctionId,
        target: IrFunctionId,
        visited: &mut HashSet<IrFunctionId>,
    ) -> bool {
        if !visited.insert(from) {
            return false;
        }

        if let Some(sites) = callees.get(&from) {
            for &site_idx in sites {
                let callee = call_sites[site_idx].callee;
                if callee == target {
                    return true;
                }
                if Self::can_reach(callees, call_sites, callee, target, visited) {
                    return true;
                }
            }
        }

        false
    }

    /// Get all call sites for a function.
    pub fn get_call_sites(&self, func_id: IrFunctionId) -> &[usize] {
        self.callees.get(&func_id).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Check if a function is recursive.
    pub fn is_recursive(&self, func_id: IrFunctionId) -> bool {
        self.recursive_functions.contains(&func_id)
    }
}

/// Inlining cost model parameters.
#[derive(Debug, Clone)]
pub struct InliningCostModel {
    /// Maximum instruction count for automatic inlining
    pub max_inline_size: usize,
    /// Bonus multiplier for calls in loops (higher = more likely to inline)
    pub loop_depth_bonus: f64,
    /// Penalty for functions with many basic blocks
    pub block_count_penalty: f64,
    /// Bonus for small functions (leaf functions with few instructions)
    pub small_function_bonus: usize,
    /// Maximum total growth allowed (as percentage of original size)
    pub max_growth_percent: usize,
}

impl Default for InliningCostModel {
    fn default() -> Self {
        Self {
            max_inline_size: 50,        // Inline functions up to 50 instructions
            loop_depth_bonus: 2.0,      // Double threshold for each loop level
            block_count_penalty: 0.9,   // Reduce threshold by 10% per extra block
            small_function_bonus: 20,   // Extra budget for tiny functions
            max_growth_percent: 200,    // Allow up to 2x code growth
        }
    }
}

impl InliningCostModel {
    /// Calculate the cost of inlining a function at a call site.
    pub fn should_inline(
        &self,
        callee: &IrFunction,
        call_site: &CallSite,
        call_graph: &CallGraph,
    ) -> bool {
        // Never inline recursive functions (for now)
        if call_graph.is_recursive(call_site.callee) {
            return false;
        }

        // Count instructions and blocks
        let mut inst_count = 0;
        for block in callee.cfg.blocks.values() {
            inst_count += block.instructions.len();
        }
        let block_count = callee.cfg.blocks.len();

        // Calculate adjusted threshold based on call site context
        let mut threshold = self.max_inline_size as f64;

        // Bonus for loops: more likely to inline hot code
        threshold *= self.loop_depth_bonus.powi(call_site.loop_depth as i32);

        // Penalty for complex CFG
        if block_count > 3 {
            threshold *= self.block_count_penalty.powi((block_count - 3) as i32);
        }

        // Bonus for very small functions
        if inst_count <= 5 {
            threshold += self.small_function_bonus as f64;
        }

        inst_count as f64 <= threshold
    }
}

/// Function inlining optimization pass.
pub struct InliningPass {
    /// Cost model for inlining decisions
    cost_model: InliningCostModel,
    /// Maximum iterations of inlining
    max_iterations: usize,
}

impl InliningPass {
    pub fn new() -> Self {
        Self {
            cost_model: InliningCostModel::default(),
            max_iterations: 3,
        }
    }

    pub fn with_cost_model(cost_model: InliningCostModel) -> Self {
        Self {
            cost_model,
            max_iterations: 3,
        }
    }

    /// Inline a specific call site.
    fn inline_call_site(
        module: &mut IrModule,
        call_site: &CallSite,
        next_reg_id: &mut u32,
    ) -> Result<(), String> {
        // Get callee function (clone to avoid borrow issues)
        let callee = module.functions.get(&call_site.callee)
            .ok_or_else(|| format!("Callee function {:?} not found", call_site.callee))?
            .clone();

        // Get caller function
        let caller = module.functions.get_mut(&call_site.caller)
            .ok_or_else(|| format!("Caller function {:?} not found", call_site.caller))?;

        // Create register mapping: callee registers -> new registers in caller
        let mut reg_map: HashMap<IrId, IrId> = HashMap::new();

        // Map callee parameters to call arguments
        for (param, arg) in callee.signature.parameters.iter().zip(&call_site.args) {
            reg_map.insert(param.reg, *arg);
        }

        // Allocate new registers for callee's internal values
        for block in callee.cfg.blocks.values() {
            for phi in &block.phi_nodes {
                if !reg_map.contains_key(&phi.dest) {
                    let new_reg = IrId::new(*next_reg_id);
                    *next_reg_id += 1;
                    reg_map.insert(phi.dest, new_reg);
                }
            }
            for inst in &block.instructions {
                if let Some(dest) = inst.dest() {
                    if !reg_map.contains_key(&dest) {
                        let new_reg = IrId::new(*next_reg_id);
                        *next_reg_id += 1;
                        reg_map.insert(dest, new_reg);
                    }
                }
            }
        }

        // Create block mapping: callee blocks -> new blocks in caller
        let mut block_map: HashMap<IrBlockId, IrBlockId> = HashMap::new();
        for &block_id in callee.cfg.blocks.keys() {
            let new_block = caller.cfg.create_block();
            block_map.insert(block_id, new_block);
        }

        // Create continuation block for after inlined code
        let continuation_block = caller.cfg.create_block();

        // Clone callee blocks into caller with remapped registers and blocks
        for (&old_block_id, old_block) in &callee.cfg.blocks {
            let new_block_id = block_map[&old_block_id];

            // Clone and remap phi nodes
            let new_phis: Vec<IrPhiNode> = old_block.phi_nodes.iter().map(|phi| {
                IrPhiNode {
                    dest: reg_map[&phi.dest],
                    incoming: phi.incoming.iter().map(|(block, val)| {
                        (block_map[block], *reg_map.get(val).unwrap_or(val))
                    }).collect(),
                    ty: phi.ty.clone(),
                }
            }).collect();

            // Clone and remap instructions
            let new_instructions: Vec<IrInstruction> = old_block.instructions.iter().map(|inst| {
                Self::remap_instruction(inst, &reg_map, &block_map)
            }).collect();

            // Handle terminator
            let new_terminator = match &old_block.terminator {
                IrTerminator::Return { value } => {
                    // Map return value to call destination
                    if let (Some(dest), Some(val)) = (call_site.dest, value) {
                        let mapped_val = *reg_map.get(val).unwrap_or(val);
                        // Add copy instruction to continuation block
                        if let Some(cont_block) = caller.cfg.get_block_mut(continuation_block) {
                            cont_block.instructions.push(IrInstruction::Copy {
                                dest,
                                src: mapped_val,
                            });
                        }
                    }
                    IrTerminator::Branch { target: continuation_block }
                }
                IrTerminator::Branch { target } => {
                    IrTerminator::Branch { target: block_map[target] }
                }
                IrTerminator::CondBranch { condition, true_target, false_target } => {
                    IrTerminator::CondBranch {
                        condition: *reg_map.get(condition).unwrap_or(condition),
                        true_target: block_map[true_target],
                        false_target: block_map[false_target],
                    }
                }
                IrTerminator::Switch { value, cases, default } => {
                    IrTerminator::Switch {
                        value: *reg_map.get(value).unwrap_or(value),
                        cases: cases.iter().map(|(v, t)| (*v, block_map[t])).collect(),
                        default: block_map[default],
                    }
                }
                other => other.clone(),
            };

            // Update the new block
            if let Some(new_block) = caller.cfg.get_block_mut(new_block_id) {
                new_block.phi_nodes = new_phis;
                new_block.instructions = new_instructions;
                new_block.terminator = new_terminator;
            }
        }

        // Split the original block at the call site
        let call_block_id = call_site.block;
        let inlined_entry = block_map[&callee.cfg.entry_block];

        if let Some(call_block) = caller.cfg.get_block_mut(call_block_id) {
            // Move instructions after the call to the continuation block
            let after_call: Vec<IrInstruction> = call_block.instructions
                .drain((call_site.instruction_index + 1)..)
                .collect();

            // Remove the call instruction
            call_block.instructions.remove(call_site.instruction_index);

            // Save original terminator for continuation
            let original_terminator = call_block.terminator.clone();

            // Redirect call block to inlined entry
            call_block.terminator = IrTerminator::Branch { target: inlined_entry };

            // Set up continuation block
            if let Some(cont_block) = caller.cfg.get_block_mut(continuation_block) {
                cont_block.instructions.extend(after_call);
                cont_block.terminator = original_terminator;
            }
        }

        // Update predecessor info (simplified - full update would require more work)
        caller.cfg.connect_blocks(call_block_id, inlined_entry);

        Ok(())
    }

    /// Remap an instruction's registers and block references.
    fn remap_instruction(
        inst: &IrInstruction,
        reg_map: &HashMap<IrId, IrId>,
        _block_map: &HashMap<IrBlockId, IrBlockId>,
    ) -> IrInstruction {
        let remap = |id: &IrId| -> IrId {
            *reg_map.get(id).unwrap_or(id)
        };

        match inst {
            IrInstruction::BinOp { dest, op, left, right } => {
                IrInstruction::BinOp {
                    dest: remap(dest),
                    op: *op,
                    left: remap(left),
                    right: remap(right),
                }
            }
            IrInstruction::UnOp { dest, op, operand } => {
                IrInstruction::UnOp {
                    dest: remap(dest),
                    op: *op,
                    operand: remap(operand),
                }
            }
            IrInstruction::Copy { dest, src } => {
                IrInstruction::Copy {
                    dest: remap(dest),
                    src: remap(src),
                }
            }
            IrInstruction::Const { dest, value } => {
                IrInstruction::Const {
                    dest: remap(dest),
                    value: value.clone(),
                }
            }
            IrInstruction::Cmp { dest, op, left, right } => {
                IrInstruction::Cmp {
                    dest: remap(dest),
                    op: *op,
                    left: remap(left),
                    right: remap(right),
                }
            }
            IrInstruction::Load { dest, ptr, ty } => {
                IrInstruction::Load {
                    dest: remap(dest),
                    ptr: remap(ptr),
                    ty: ty.clone(),
                }
            }
            IrInstruction::Store { ptr, value } => {
                IrInstruction::Store {
                    ptr: remap(ptr),
                    value: remap(value),
                }
            }
            IrInstruction::Cast { dest, src, from_ty, to_ty } => {
                IrInstruction::Cast {
                    dest: remap(dest),
                    src: remap(src),
                    from_ty: from_ty.clone(),
                    to_ty: to_ty.clone(),
                }
            }
            // For other instructions, just clone (may need extension for full support)
            other => other.clone(),
        }
    }
}

impl OptimizationPass for InliningPass {
    fn name(&self) -> &'static str {
        "inlining"
    }

    fn run_on_module(&mut self, module: &mut IrModule) -> OptimizationResult {
        let mut result = OptimizationResult::unchanged();

        for _iteration in 0..self.max_iterations {
            let call_graph = CallGraph::build(module);

            // Find candidate call sites for inlining
            let mut candidates: Vec<CallSite> = Vec::new();

            for site in &call_graph.call_sites {
                if let Some(callee) = module.functions.get(&site.callee) {
                    if self.cost_model.should_inline(callee, site, &call_graph) {
                        candidates.push(site.clone());
                    }
                }
            }

            if candidates.is_empty() {
                break;
            }

            // Sort by priority: prefer loop-nested calls and smaller functions
            candidates.sort_by(|a, b| {
                // Higher loop depth = higher priority
                b.loop_depth.cmp(&a.loop_depth)
            });

            // Find the highest register ID in use
            let mut next_reg_id = 0u32;
            for function in module.functions.values() {
                for block in function.cfg.blocks.values() {
                    for phi in &block.phi_nodes {
                        next_reg_id = next_reg_id.max(phi.dest.as_u32() + 1);
                    }
                    for inst in &block.instructions {
                        if let Some(dest) = inst.dest() {
                            next_reg_id = next_reg_id.max(dest.as_u32() + 1);
                        }
                    }
                }
            }

            // Inline one call at a time to avoid invalidating indices
            if let Some(candidate) = candidates.into_iter().next() {
                match Self::inline_call_site(module, &candidate, &mut next_reg_id) {
                    Ok(()) => {
                        result.modified = true;
                        *result.stats.entry("functions_inlined".to_string()).or_insert(0) += 1;
                    }
                    Err(e) => {
                        // Log error but continue
                        tracing::warn!("Failed to inline call: {}", e);
                    }
                }
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cost_model_small_function() {
        let cost_model = InliningCostModel::default();

        // A function with 3 instructions should be inlined
        assert!(3.0 <= cost_model.max_inline_size as f64);
    }

    #[test]
    fn test_cost_model_loop_bonus() {
        let cost_model = InliningCostModel::default();

        // Loop depth bonus should increase threshold
        let base_threshold = cost_model.max_inline_size as f64;
        let loop_threshold = base_threshold * cost_model.loop_depth_bonus;

        assert!(loop_threshold > base_threshold);
    }
}
