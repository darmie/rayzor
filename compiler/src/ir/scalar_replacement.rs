//! Scalar Replacement of Aggregates (SRA) Pass
//!
//! Replaces heap/stack allocations that are only accessed via GEP+Load/Store
//! with individual scalar registers (one per field). This eliminates allocation
//! overhead for small structs like `Complex { re: f64, im: f64 }` that don't
//! escape the current scope.
//!
//! The pass runs after inlining so that constructor bodies and field accesses
//! are visible in the same function. Supports multi-block patterns where the
//! alloc, stores, loads, and free may be in different basic blocks.

use super::optimization::{OptimizationPass, OptimizationResult};
use super::{IrBlockId, IrFunction, IrFunctionId, IrId, IrInstruction, IrModule, IrType, IrValue};
use std::collections::{HashMap, HashSet};

pub struct ScalarReplacementPass;

impl ScalarReplacementPass {
    pub fn new() -> Self {
        Self
    }
}

/// A candidate allocation that can be scalar-replaced (function-wide).
struct SraCandidate {
    alloc_dest: IrId,
    /// (block_id, instruction_index) of the Alloc
    alloc_location: (IrBlockId, usize),
    /// Maps GEP dest IrId → field index (across all blocks)
    gep_map: HashMap<IrId, usize>,
    /// All tracked pointer IrIds (alloc dest + GEP dests + copies/casts)
    tracked: HashSet<IrId>,
    /// (block_id, instruction_index) of Free instructions to remove
    free_locations: Vec<(IrBlockId, usize)>,
    /// Number of fields
    num_fields: usize,
    /// Types of loads per field index
    field_types: HashMap<usize, IrType>,
    /// All GEP instruction locations to remove: (block_id, inst_idx)
    gep_locations: Vec<(IrBlockId, usize)>,
    /// All Copy/Cast locations that propagate tracked ptrs: (block_id, inst_idx)
    copy_locations: Vec<(IrBlockId, usize)>,
}

impl OptimizationPass for ScalarReplacementPass {
    fn name(&self) -> &'static str {
        "scalar_replacement"
    }

    fn run_on_module(&mut self, module: &mut IrModule) -> OptimizationResult {
        let mut result = OptimizationResult::unchanged();

        // Identify malloc and free function IDs
        let mut malloc_ids = HashSet::new();
        let mut free_ids = HashSet::new();
        for (&fid, func) in &module.functions {
            if func.name == "malloc" {
                malloc_ids.insert(fid);
            } else if func.name == "free" {
                free_ids.insert(fid);
            }
        }
        for (&fid, func) in &module.extern_functions {
            if func.name == "malloc" {
                malloc_ids.insert(fid);
            } else if func.name == "free" {
                free_ids.insert(fid);
            }
        }

        for function in module.functions.values_mut() {
            let r = run_sra_on_function(function, &malloc_ids, &free_ids);
            if r.modified {
                result.modified = true;
                result.instructions_eliminated += r.instructions_eliminated;
                for (k, v) in &r.stats {
                    *result.stats.entry(k.clone()).or_insert(0) += v;
                }
            }
        }

        result
    }
}

fn run_sra_on_function(function: &mut IrFunction, malloc_ids: &HashSet<IrFunctionId>, free_ids: &HashSet<IrFunctionId>) -> OptimizationResult {
    let mut result = OptimizationResult::unchanged();

    let constants = build_constant_map(&function.cfg);
    let candidates = find_candidates_in_function(&function.cfg, &constants, malloc_ids, free_ids);

    for candidate in &candidates {
        let eliminated = apply_sra(function, candidate);
        if eliminated > 0 {
            result.modified = true;
            result.instructions_eliminated += eliminated;
            *result.stats.entry("allocs_replaced".to_string()).or_insert(0) += 1;
        }
    }

    result
}

/// Build a map of IrId → constant value from all Const instructions.
fn build_constant_map(
    cfg: &super::blocks::IrControlFlowGraph,
) -> HashMap<IrId, i64> {
    let mut constants = HashMap::new();
    for block in cfg.blocks.values() {
        for inst in &block.instructions {
            if let IrInstruction::Const { dest, value } = inst {
                let int_val = match value {
                    IrValue::I32(v) => Some(*v as i64),
                    IrValue::I64(v) => Some(*v),
                    IrValue::U32(v) => Some(*v as i64),
                    IrValue::U64(v) => Some(*v as i64),
                    _ => None,
                };
                if let Some(v) = int_val {
                    constants.insert(*dest, v);
                }
            }
        }
    }
    constants
}

/// Find all SRA candidates across the entire function.
fn find_candidates_in_function(
    cfg: &super::blocks::IrControlFlowGraph,
    constants: &HashMap<IrId, i64>,
    malloc_ids: &HashSet<IrFunctionId>,
    free_ids: &HashSet<IrFunctionId>,
) -> Vec<SraCandidate> {
    let mut candidates = Vec::new();

    for (&block_id, block) in &cfg.blocks {
        for (idx, inst) in block.instructions.iter().enumerate() {
            let alloc_dest = match inst {
                // Stack alloc
                IrInstruction::Alloc { dest, count: None, .. } => *dest,
                // Heap alloc via malloc
                IrInstruction::CallDirect { dest: Some(dest), func_id, args, .. }
                    if malloc_ids.contains(func_id) && args.len() == 1 => *dest,
                _ => continue,
            };

            if let Some(candidate) = try_build_candidate_function_wide(
                alloc_dest,
                (block_id, idx),
                cfg,
                constants,
                free_ids,
            ) {
                candidates.push(candidate);
            }
        }
    }

    candidates
}

/// Try to build an SRA candidate by scanning ALL blocks for uses.
fn try_build_candidate_function_wide(
    alloc_dest: IrId,
    alloc_location: (IrBlockId, usize),
    cfg: &super::blocks::IrControlFlowGraph,
    constants: &HashMap<IrId, i64>,
    free_ids: &HashSet<IrFunctionId>,
) -> Option<SraCandidate> {
    // Phase 1: Build tracked pointer set across ALL blocks
    let mut tracked = HashSet::new();
    tracked.insert(alloc_dest);

    let mut gep_map: HashMap<IrId, usize> = HashMap::new();

    // Iterate until fixpoint — find all GEPs, copies, casts derived from alloc
    let mut changed = true;
    while changed {
        changed = false;
        for block in cfg.blocks.values() {
            for inst in &block.instructions {
                match inst {
                    IrInstruction::GetElementPtr {
                        dest, ptr, indices, ..
                    } if tracked.contains(ptr) && !tracked.contains(dest) => {
                        if let Some(field_idx) = resolve_gep_field_index(indices, constants) {
                            tracked.insert(*dest);
                            gep_map.insert(*dest, field_idx);
                            changed = true;
                        } else {
                            return None; // Non-constant GEP index
                        }
                    }
                    IrInstruction::Copy { dest, src }
                        if tracked.contains(src) && !tracked.contains(dest) =>
                    {
                        tracked.insert(*dest);
                        if let Some(&field_idx) = gep_map.get(src) {
                            gep_map.insert(*dest, field_idx);
                        }
                        changed = true;
                    }
                    IrInstruction::Cast { dest, src, .. }
                    | IrInstruction::BitCast { dest, src, .. }
                        if tracked.contains(src) && !tracked.contains(dest) =>
                    {
                        tracked.insert(*dest);
                        if let Some(&field_idx) = gep_map.get(src) {
                            gep_map.insert(*dest, field_idx);
                        }
                        changed = true;
                    }
                    _ => {}
                }
            }
        }
    }

    // Phase 2: Check escape conditions across ALL blocks
    let mut free_locations = Vec::new();
    let mut field_types: HashMap<usize, IrType> = HashMap::new();
    let mut gep_locations = Vec::new();
    let mut copy_locations = Vec::new();

    for (&block_id, block) in &cfg.blocks {
        // Check phi nodes — allow phis where ALL incoming values involving
        // tracked pointers are tracked (the phi just aliases the same alloc).
        // The phi dest becomes tracked too.
        // Reject if only SOME incoming values are tracked (merging different allocs).
        for phi in &block.phi_nodes {
            let any_tracked_incoming = phi.incoming.iter().any(|(_, v)| tracked.contains(v));
            if any_tracked_incoming || tracked.contains(&phi.dest) {
                // For now, reject phis involving tracked pointers.
                // Multi-block SRA through phis requires inserting scalar phis
                // for each field, which is a future extension.
                return None;
            }
        }

        for (inst_idx, inst) in block.instructions.iter().enumerate() {
            match inst {
                IrInstruction::Store { ptr, value } => {
                    if tracked.contains(ptr) {
                        // Store to a tracked GEP field — OK only if value is NOT tracked
                        if tracked.contains(value) {
                            return None; // Tracked pointer used as stored value — escapes
                        }
                    } else if tracked.contains(value) {
                        return None; // Pointer escapes via store
                    }
                }

                IrInstruction::Load { ptr, ty, .. } => {
                    if tracked.contains(ptr) {
                        if let Some(&field_idx) = gep_map.get(ptr) {
                            field_types.insert(field_idx, ty.clone());
                        }
                    }
                }

                IrInstruction::Free { ptr } => {
                    if tracked.contains(ptr) {
                        free_locations.push((block_id, inst_idx));
                    }
                }

                // CallDirect to free with a tracked pointer
                IrInstruction::CallDirect { func_id, args, .. }
                    if free_ids.contains(func_id)
                        && args.len() == 1
                        && tracked.contains(&args[0]) =>
                {
                    free_locations.push((block_id, inst_idx));
                }

                IrInstruction::GetElementPtr { ptr, dest, .. } if tracked.contains(ptr) => {
                    gep_locations.push((block_id, inst_idx));
                    // Already handled in tracking phase
                    let _ = dest;
                }

                IrInstruction::Copy { src, .. } if tracked.contains(src) => {
                    copy_locations.push((block_id, inst_idx));
                }
                IrInstruction::Cast { src, .. } if tracked.contains(src) => {
                    copy_locations.push((block_id, inst_idx));
                }
                IrInstruction::BitCast { src, .. } if tracked.contains(src) => {
                    copy_locations.push((block_id, inst_idx));
                }

                IrInstruction::Alloc { dest, .. } if *dest == alloc_dest => {}
                // The malloc call that IS this allocation
                IrInstruction::CallDirect { dest: Some(d), .. } if *d == alloc_dest => {}
                // A free call on a tracked pointer (already recorded above)
                IrInstruction::CallDirect { func_id, args, .. }
                    if free_ids.contains(func_id)
                        && args.len() == 1
                        && tracked.contains(&args[0]) => {}
                IrInstruction::Const { .. } => {}

                _ => {
                    if uses_any_tracked(inst, &tracked) {
                        return None;
                    }
                }
            }
        }

        // Check terminator
        if terminator_uses_tracked(&block.terminator, &tracked) {
            return None;
        }
    }

    if gep_map.is_empty() {
        return None;
    }

    let num_fields = gep_map.values().copied().max().unwrap_or(0) + 1;

    Some(SraCandidate {
        alloc_dest,
        alloc_location,
        gep_map,
        tracked,
        free_locations,
        num_fields,
        field_types,
        gep_locations,
        copy_locations,
    })
}

/// Resolve GEP indices to a single field index.
fn resolve_gep_field_index(
    indices: &[IrId],
    constants: &HashMap<IrId, i64>,
) -> Option<usize> {
    match indices.len() {
        1 => {
            let idx = constants.get(&indices[0])?;
            if *idx < 0 { return None; }
            Some(*idx as usize)
        }
        2 => {
            let base = constants.get(&indices[0])?;
            if *base != 0 { return None; }
            let field = constants.get(&indices[1])?;
            if *field < 0 { return None; }
            Some(*field as usize)
        }
        _ => None,
    }
}

fn uses_any_tracked(inst: &IrInstruction, tracked: &HashSet<IrId>) -> bool {
    match inst {
        IrInstruction::CallDirect { args, .. } => args.iter().any(|a| tracked.contains(a)),
        IrInstruction::CallIndirect { args, func_ptr, .. } => {
            tracked.contains(func_ptr) || args.iter().any(|a| tracked.contains(a))
        }
        IrInstruction::Return { value: Some(v) } => tracked.contains(v),
        IrInstruction::MemCopy { dest, src, .. } => {
            tracked.contains(dest) || tracked.contains(src)
        }
        IrInstruction::StoreGlobal { value, .. } => tracked.contains(value),
        IrInstruction::MakeClosure { captured_values, .. } => {
            captured_values.iter().any(|v| tracked.contains(v))
        }
        IrInstruction::Throw { exception } => tracked.contains(exception),
        IrInstruction::BinOp { left, right, .. } => {
            tracked.contains(left) || tracked.contains(right)
        }
        IrInstruction::Select { condition, true_val, false_val, .. } => {
            tracked.contains(condition) || tracked.contains(true_val) || tracked.contains(false_val)
        }
        IrInstruction::Phi { incoming, .. } => {
            incoming.iter().any(|(v, _)| tracked.contains(v))
        }
        _ => false,
    }
}

fn terminator_uses_tracked(
    terminator: &super::IrTerminator,
    tracked: &HashSet<IrId>,
) -> bool {
    match terminator {
        super::IrTerminator::Return { value: Some(v) } => tracked.contains(v),
        super::IrTerminator::CondBranch { condition, .. } => tracked.contains(condition),
        super::IrTerminator::Switch { value, .. } => tracked.contains(value),
        _ => false,
    }
}

/// Apply SRA rewrite for a multi-block candidate.
///
/// Strategy: process blocks in order. For each block, track field values
/// using a per-block map. When we encounter a Store to a field GEP, record
/// the value. When we encounter a Load from a field GEP, replace with a
/// Copy from the last stored value for that field.
///
/// This works for the post-inlining pattern where stores happen before loads
/// in a linear block sequence (alloc block → constructor block → use block).
fn apply_sra(
    function: &mut IrFunction,
    candidate: &SraCandidate,
) -> usize {
    // Allocate initial Undef registers for each field
    let mut field_regs: Vec<IrId> = Vec::with_capacity(candidate.num_fields);
    for _ in 0..candidate.num_fields {
        let id = IrId::new(function.next_reg_id);
        function.next_reg_id += 1;
        field_regs.push(id);
    }

    // Global field value tracker — initialized to the Undef registers
    let mut field_current: Vec<IrId> = field_regs.clone();
    let mut eliminated = 0;

    // Collect all locations to remove, grouped by block
    let mut to_remove: HashMap<IrBlockId, HashSet<usize>> = HashMap::new();

    // Mark alloc for removal
    to_remove
        .entry(candidate.alloc_location.0)
        .or_default()
        .insert(candidate.alloc_location.1);

    // Mark frees for removal
    for &(block_id, inst_idx) in &candidate.free_locations {
        to_remove.entry(block_id).or_default().insert(inst_idx);
    }

    // Don't remove GEPs eagerly — they may be referenced by non-SRA'd code.
    // DCE will clean them up if they become dead after the rewrite.

    // Don't remove copy/cast of tracked ptrs — they may be used as values
    // elsewhere. DCE will clean them up if they become dead.

    // Build a simple block ordering: BFS from entry to process stores before loads
    let block_order = bfs_block_order(&function.cfg);

    // First pass: find all stores to determine field values per block
    // We need to process in order so loads see the right field values.
    // Build a map of (block_id, inst_idx) → replacement instruction
    let mut replacements: HashMap<(IrBlockId, usize), IrInstruction> = HashMap::new();

    for &block_id in &block_order {
        let block = match function.cfg.blocks.get(&block_id) {
            Some(b) => b,
            None => continue,
        };

        for (inst_idx, inst) in block.instructions.iter().enumerate() {
            match inst {
                // Replace Store via GEP → Copy to field register
                IrInstruction::Store { ptr, value } if candidate.gep_map.contains_key(ptr) => {
                    let field_idx = candidate.gep_map[ptr];
                    if field_idx < candidate.num_fields {
                        let new_reg = IrId::new(function.next_reg_id);
                        function.next_reg_id += 1;
                        replacements.insert(
                            (block_id, inst_idx),
                            IrInstruction::Copy {
                                dest: new_reg,
                                src: *value,
                            },
                        );
                        field_current[field_idx] = new_reg;
                    }
                    to_remove.entry(block_id).or_default().insert(inst_idx);
                }

                // Replace Load via GEP → Copy from current field register
                IrInstruction::Load { dest, ptr, .. }
                    if candidate.gep_map.contains_key(ptr) =>
                {
                    let field_idx = candidate.gep_map[ptr];
                    if field_idx < candidate.num_fields {
                        replacements.insert(
                            (block_id, inst_idx),
                            IrInstruction::Copy {
                                dest: *dest,
                                src: field_current[field_idx],
                            },
                        );
                    }
                    to_remove.entry(block_id).or_default().insert(inst_idx);
                }

                _ => {}
            }
        }
    }

    // Second pass: rewrite each block
    for &block_id in &block_order {
        let block_removes = to_remove.get(&block_id);
        let block = match function.cfg.blocks.get_mut(&block_id) {
            Some(b) => b,
            None => continue,
        };

        let has_removes = block_removes.map_or(false, |s| !s.is_empty());
        if !has_removes {
            continue;
        }

        let block_removes = block_removes.unwrap();
        let old_instructions = std::mem::take(&mut block.instructions);
        let mut new_instructions = Vec::with_capacity(old_instructions.len());

        for (idx, inst) in old_instructions.into_iter().enumerate() {
            // At alloc position, insert Undef for each field
            if (block_id, idx) == candidate.alloc_location {
                for (field_idx, reg) in field_regs.iter().enumerate() {
                    let ty = candidate
                        .field_types
                        .get(&field_idx)
                        .cloned()
                        .unwrap_or(IrType::I64);
                    new_instructions.push(IrInstruction::Undef { dest: *reg, ty });
                }
                eliminated += 1;
                continue;
            }

            if block_removes.contains(&idx) {
                if let Some(replacement) = replacements.remove(&(block_id, idx)) {
                    new_instructions.push(replacement);
                }
                // else: just remove (GEP, Free, Copy of ptr, etc.)
                eliminated += 1;
                continue;
            }

            new_instructions.push(inst);
        }

        block.instructions = new_instructions;
    }

    eliminated
}

/// BFS block ordering from entry block.
fn bfs_block_order(cfg: &super::blocks::IrControlFlowGraph) -> Vec<IrBlockId> {
    let mut order = Vec::new();
    let mut visited = HashSet::new();
    let mut queue = std::collections::VecDeque::new();

    queue.push_back(cfg.entry_block);
    visited.insert(cfg.entry_block);

    while let Some(block_id) = queue.pop_front() {
        order.push(block_id);

        if let Some(block) = cfg.blocks.get(&block_id) {
            for succ in block.successors() {
                if visited.insert(succ) {
                    queue.push_back(succ);
                }
            }
        }
    }

    // Include any unreachable blocks not visited by BFS
    for &block_id in cfg.blocks.keys() {
        if visited.insert(block_id) {
            order.push(block_id);
        }
    }

    order
}
