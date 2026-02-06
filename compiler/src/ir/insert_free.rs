//! Insert Free Pass — adds Free instructions for non-escaping heap allocations.
//!
//! This pass runs at the MIR level after HIR→MIR lowering to ensure all heap
//! allocations that don't escape the function are properly freed. It handles
//! cases the HIR-level drop analysis misses, such as factory function return
//! values (e.g., `createComplex()` which returns `new Complex()`).
//!
//! ## Algorithm
//!
//! For each function:
//! 1. Find all `Alloc` instructions
//! 2. Track derived pointers (GEP, Cast, Copy of alloc result)
//! 3. Check escape conditions:
//!    - Pointer returned from function → escapes
//!    - Pointer passed as argument to a function call → escapes
//!    - Pointer stored as a value (not as a store target) → escapes
//!    - Pointer used in phi node → complex control flow, skip (V1)
//! 4. For non-escaping allocations that have no existing Free, insert Free
//!    before each return instruction in the function

use super::blocks::{IrBlockId, IrTerminator};
use super::instructions::IrInstruction;
use super::optimization::{OptimizationPass, OptimizationResult};
use super::{IrFunction, IrId, IrModule};
use std::collections::{HashMap, HashSet};

pub struct InsertFreePass;

impl InsertFreePass {
    pub fn new() -> Self {
        InsertFreePass
    }
}

impl OptimizationPass for InsertFreePass {
    fn name(&self) -> &'static str {
        "InsertFree"
    }

    fn run_on_module(&mut self, module: &mut IrModule) -> OptimizationResult {
        let mut total_inserted = 0;

        let func_ids: Vec<_> = module.functions.keys().cloned().collect();
        for func_id in func_ids {
            if let Some(function) = module.functions.get_mut(&func_id) {
                total_inserted += insert_free_for_function(function);
            }
        }

        if total_inserted > 0 {
            OptimizationResult {
                modified: true,
                instructions_eliminated: 0,
                stats: {
                    let mut s = HashMap::new();
                    s.insert("free_instructions_inserted".to_string(), total_inserted);
                    s
                },
                blocks_eliminated: 0,
            }
        } else {
            OptimizationResult::unchanged()
        }
    }
}

/// Insert Free instructions for non-escaping Alloc in a single function.
/// Returns the number of Free instructions inserted.
fn insert_free_for_function(function: &mut IrFunction) -> usize {
    if function.cfg.blocks.is_empty() {
        return 0;
    }

    // Step 1: Find all Alloc instructions and their destinations
    let mut alloc_ids: Vec<IrId> = Vec::new();
    for block in function.cfg.blocks.values() {
        for inst in &block.instructions {
            if let IrInstruction::Alloc { dest, .. } = inst {
                alloc_ids.push(*dest);
            }
        }
    }

    if alloc_ids.is_empty() {
        return 0;
    }

    // Step 2: For each Alloc, build the set of derived pointers and check escape
    let mut allocs_needing_free: Vec<IrId> = Vec::new();

    for &alloc_id in &alloc_ids {
        // Build derived pointer set (transitive closure of GEP, Cast, Copy)
        let derived = build_derived_set(alloc_id, function);

        // Check if already has a Free
        let has_free = function.cfg.blocks.values().any(|block| {
            block.instructions.iter().any(|inst| {
                if let IrInstruction::Free { ptr } = inst {
                    derived.contains(ptr) || *ptr == alloc_id
                } else {
                    false
                }
            })
        });

        if has_free {
            continue; // Already freed, skip
        }

        // Check escape conditions
        if !pointer_escapes(alloc_id, &derived, function) {
            allocs_needing_free.push(alloc_id);
        }
    }

    if allocs_needing_free.is_empty() {
        return 0;
    }

    // Step 3: Find all blocks that end with a return
    let return_blocks: Vec<IrBlockId> = function
        .cfg
        .blocks
        .iter()
        .filter(|(_, block)| matches!(block.terminator, IrTerminator::Return { .. }))
        .map(|(id, _)| *id)
        .collect();

    // Pre-compute derived sets for each alloc (before mutating blocks)
    let derived_sets: HashMap<IrId, HashSet<IrId>> = allocs_needing_free
        .iter()
        .map(|&id| (id, build_derived_set(id, function)))
        .collect();

    // Step 4: Insert Free before each return for each non-escaping alloc
    let mut inserted = 0;
    for block_id in &return_blocks {
        if let Some(block) = function.cfg.blocks.get_mut(block_id) {
            // Check if the return value is one of our allocs
            let return_value = if let IrTerminator::Return { value } = &block.terminator {
                *value
            } else {
                None
            };

            for &alloc_id in &allocs_needing_free {
                let derived = &derived_sets[&alloc_id];
                // Don't free if this alloc is being returned
                if let Some(ret_val) = return_value {
                    if ret_val == alloc_id || derived.contains(&ret_val) {
                        continue;
                    }
                }

                // Insert Free at the end of the block (before terminator)
                block.instructions.push(IrInstruction::Free { ptr: alloc_id });
                inserted += 1;
            }
        }
    }

    inserted
}

/// Build the set of all IrIds derived from an allocation pointer.
/// Includes the alloc_id itself plus any GEP, Cast, or Copy that uses it.
fn build_derived_set(alloc_id: IrId, function: &IrFunction) -> HashSet<IrId> {
    let mut derived = HashSet::new();
    derived.insert(alloc_id);

    // Fixed-point iteration to find all derived pointers
    let mut changed = true;
    while changed {
        changed = false;
        for block in function.cfg.blocks.values() {
            for inst in &block.instructions {
                match inst {
                    IrInstruction::GetElementPtr { dest, ptr, .. } => {
                        if derived.contains(ptr) && derived.insert(*dest) {
                            changed = true;
                        }
                    }
                    IrInstruction::Cast { dest, src, .. } => {
                        if derived.contains(src) && derived.insert(*dest) {
                            changed = true;
                        }
                    }
                    IrInstruction::Copy { dest, src } => {
                        if derived.contains(src) && derived.insert(*dest) {
                            changed = true;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    derived
}

/// Check if a pointer (or any of its derived pointers) escapes the function.
fn pointer_escapes(alloc_id: IrId, derived: &HashSet<IrId>, function: &IrFunction) -> bool {
    for block in function.cfg.blocks.values() {
        // Check instructions
        for inst in &block.instructions {
            match inst {
                // Pointer passed as function argument → escapes
                IrInstruction::CallDirect { args, .. } => {
                    for arg in args {
                        if *arg == alloc_id || derived.contains(arg) {
                            return true;
                        }
                    }
                }
                IrInstruction::CallIndirect { args, func_ptr, .. } => {
                    if *func_ptr == alloc_id || derived.contains(func_ptr) {
                        return true;
                    }
                    for arg in args {
                        if *arg == alloc_id || derived.contains(arg) {
                            return true;
                        }
                    }
                }

                // Pointer stored as a VALUE to memory → escapes
                // Store { ptr, value } — if value is our alloc pointer, it escapes
                IrInstruction::Store { value, .. } => {
                    if *value == alloc_id || derived.contains(value) {
                        return true;
                    }
                }

                _ => {}
            }
        }

        // Check phi nodes — if alloc pointer flows through phi, complex control flow
        for phi in &block.phi_nodes {
            for (_, val) in &phi.incoming {
                if *val == alloc_id || derived.contains(val) {
                    return true;
                }
            }
        }

        // Check terminator — if pointer is returned, it escapes
        if let IrTerminator::Return { value: Some(val) } = &block.terminator {
            if *val == alloc_id || derived.contains(val) {
                return true;
            }
        }
    }

    false
}
