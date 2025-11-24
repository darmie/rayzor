# Cranelift Loop Implementation - Quick Reference

**TL;DR:** Exit blocks CAN and SHOULD have parameters to receive final loop values. Use `brif` to pass different arguments to each branch.

---

## The Answer to Your Key Question

**Q: When a loop exits (condition becomes false), how do you pass the final loop variable values to the exit block?**

**A: Pass them as arguments in the `brif` instruction, and the exit block receives them as block parameters.**

```rust
// In loop header:
builder.ins().brif(
    condition,
    loop_body, &[],              // True: body doesn't need arguments
    exit_block, &[sum, i]        // False: pass final values to exit
);

// Exit block declaration:
builder.append_block_param(exit_block, types::I64); // sum parameter
builder.append_block_param(exit_block, types::I64); // i parameter

// In exit block:
builder.switch_to_block(exit_block);
let params = builder.block_params(exit_block);
let final_sum = params[0];
let final_i = params[1];
builder.ins().return_(&[final_sum]);
```

---

## Complete Working Example

```rust
use cranelift::prelude::*;
use cranelift_codegen::ir::condcodes::IntCC;

fn compile_sum_to_n(builder: &mut FunctionBuilder, param_n: Value) -> Result<(), String> {
    // Create all blocks first
    let entry_block = builder.create_block();
    let loop_header = builder.create_block();
    let loop_body = builder.create_block();
    let exit_block = builder.create_block();
    
    // Entry block: initialize variables and jump to loop
    builder.switch_to_block(entry_block);
    let sum_init = builder.ins().iconst(types::I64, 0);  // sum = 0
    let i_init = builder.ins().iconst(types::I64, 1);    // i = 1
    builder.ins().jump(loop_header, &[sum_init, i_init]);
    builder.seal_block(entry_block);
    
    // Loop header: receives sum and i as block parameters (phi nodes)
    builder.append_block_param(loop_header, types::I64); // sum parameter
    builder.append_block_param(loop_header, types::I64); // i parameter
    builder.switch_to_block(loop_header);
    let header_params = builder.block_params(loop_header);
    let sum = header_params[0];
    let i = header_params[1];
    
    // Check condition: i <= n
    let condition = builder.ins().icmp(IntCC::SignedLessThanOrEqual, i, param_n);
    
    // Branch: if true -> body, if false -> exit (passing final sum)
    builder.ins().brif(
        condition,
        loop_body, &[],         // Body doesn't need arguments
        exit_block, &[sum]      // Pass final sum to exit
    );
    
    // Loop body: compute sum+i and i+1, jump back to header
    builder.switch_to_block(loop_body);
    let sum_next = builder.ins().iadd(sum, i);
    let i_next = builder.ins().iadd_imm(i, 1);
    builder.ins().jump(loop_header, &[sum_next, i_next]);
    builder.seal_block(loop_body);
    
    // IMPORTANT: Seal header AFTER the back edge is added
    builder.seal_block(loop_header);
    
    // Exit block: receives final sum as parameter
    builder.append_block_param(exit_block, types::I64); // sum parameter
    builder.switch_to_block(exit_block);
    let exit_params = builder.block_params(exit_block);
    let final_sum = exit_params[0];
    builder.ins().return_(&[final_sum]);
    builder.seal_block(exit_block);
    
    Ok(())
}
```

---

## Your Current Implementation is Correct!

The code in `/Users/amaterasu/Vibranium/rayzor/compiler/src/codegen/cranelift_backend.rs` is already doing the right thing:

### 1. Phi Node Translation (Line 301-337) ✅
```rust
fn translate_phi_node_static(...) {
    // Append a block parameter for each phi node
    let block_param = builder.append_block_param(current_block, cl_type);
    value_map.insert(phi_node.dest, block_param);
}
```

**This is correct!** Phi nodes become block parameters.

### 2. Jump with Arguments (Line 441-454) ✅
```rust
IrTerminator::Branch { target } => {
    let phi_args = Self::collect_phi_args(value_map, function, *target, current_block_id)?;
    builder.ins().jump(cl_block, &phi_args);
}
```

**Perfect!** Passes phi node values when jumping.

### 3. Brif with Separate Arguments (Line 457-476) ✅
```rust
IrTerminator::CondBranch { condition, true_target, false_target } => {
    let true_phi_args = Self::collect_phi_args(..., *true_target, ...)?;
    let false_phi_args = Self::collect_phi_args(..., *false_target, ...)?;
    builder.ins().brif(cond_val, true_block, &true_phi_args, false_block, &false_phi_args);
}
```

**Excellent!** Each branch gets its own arguments.

### 4. Collect Phi Args (Line 264-298) ✅
```rust
fn collect_phi_args(...) -> Result<Vec<Value>, String> {
    let mut phi_args = Vec::new();
    for phi_node in &target.phi_nodes {
        let incoming_value = phi_node.incoming.iter()
            .find(|(block_id, _)| *block_id == from_block)
            .map(|(_, value_id)| value_id)
            .ok_or_else(|| format!("No incoming value..."))?;
        let cl_value = *value_map.get(incoming_value).ok_or_else(...)?;
        phi_args.push(cl_value);
    }
    Ok(phi_args)
}
```

**This is exactly right!** Looks up the correct incoming value for each phi node based on the predecessor block.

---

## The Problem with Your Test

The issue is NOT in your backend code. It's in how the test constructs MIR.

### Your Test (WRONG) ❌

From `/Users/amaterasu/Vibranium/rayzor/compiler/examples/test_cranelift_loop.rs`:

```rust
// Loop body:
body.instructions.push(IrInstruction::Copy {
    dest: sum_reg,   // ❌ Trying to "update" sum
    src: sum_new,
});
body.instructions.push(IrInstruction::Copy {
    dest: i_reg,     // ❌ Trying to "update" i
    src: i_new,
});
```

**Problem:** You're using `Copy` to redefine SSA values, which breaks SSA form. The loop header doesn't have phi nodes, so there's no way for values to merge.

### How to Fix ✅

Add phi nodes to the loop header:

```rust
// Create phi node IDs
let sum_phi = IrId::new(10);
let i_phi = IrId::new(11);

// Add phi nodes to loop header
let header = function.cfg.blocks.get_mut(&loop_header).unwrap();
header.phi_nodes.push(IrPhiNode {
    dest: sum_phi,
    incoming: vec![
        (entry_block, const_0),      // From entry: sum = 0
        (loop_body, sum_new),         // From body: sum = sum + i
    ],
    ty: IrType::I64,
});
header.phi_nodes.push(IrPhiNode {
    dest: i_phi,
    incoming: vec![
        (entry_block, const_1),       // From entry: i = 1
        (loop_body, i_new),           // From body: i = i + 1
    ],
    ty: IrType::I64,
});

// In loop header instructions, USE the phi values
header.instructions.push(IrInstruction::Cmp {
    dest: cond_reg,
    op: CompareOp::Le,
    left: i_phi,      // ✅ Use phi value, not original i_reg
    right: param_n,
});

// In loop body, compute from phi values
let body = function.cfg.blocks.get_mut(&loop_body).unwrap();
body.instructions.push(IrInstruction::BinOp {
    dest: sum_new,
    op: BinaryOp::Add,
    left: sum_phi,    // ✅ Use phi value
    right: i_phi,     // ✅ Use phi value
});
body.instructions.push(IrInstruction::BinOp {
    dest: i_new,
    op: BinaryOp::Add,
    left: i_phi,      // ✅ Use phi value
    right: const_1,
});
// NO Copy instructions! Just branch back
body.terminator = IrTerminator::Branch { target: loop_header };

// Exit block should also have a phi to receive final sum
let exit = function.cfg.blocks.get_mut(&exit_block).unwrap();
exit.phi_nodes.push(IrPhiNode {
    dest: final_sum_reg,
    incoming: vec![
        (loop_header, sum_phi),  // Receive final sum from header
    ],
    ty: IrType::I64,
});
exit.terminator = IrTerminator::Return { value: Some(final_sum_reg) };
```

---

## Key Principles

### 1. Block Parameters = Phi Nodes
```
Traditional:         Cranelift:
phi i = [1, 5]  →   block(i: i64)
```

### 2. Jump Passes Arguments
```rust
builder.ins().jump(target, &[arg1, arg2]);
// Arguments become the block's parameter values
```

### 3. Brif Passes Different Arguments
```rust
builder.ins().brif(cond, 
    true_block, &[true_args],
    false_block, &[false_args]
);
// Each branch can pass different values
```

### 4. Seal After All Predecessors
```rust
// Add ALL branches to loop_header (including back edge)
builder.ins().jump(loop_header, &[...]);  // From entry
builder.ins().jump(loop_header, &[...]);  // From body (back edge)

// THEN seal
builder.seal_block(loop_header);  // Now all predecessors are known
```

### 5. Exit Blocks Can Have Parameters
```rust
// Exit block receives final loop values
builder.append_block_param(exit_block, types::I64);
builder.ins().brif(cond, body, &[], exit, &[final_value]);
```

---

## Common Patterns

### Pattern 1: While Loop (Condition at Top)
```
entry → loop_header → loop_body
           ↓ exit         ↑
        exit_block        └─ back edge
```

### Pattern 2: Do-While Loop (Condition at Bottom)
```
entry → loop_body → loop_body (back edge if condition true)
           ↓ exit
        exit_block
```

### Pattern 3: For Loop
```
entry → init → loop_header → loop_body → increment
                   ↓ exit         ↑           ↓
                exit_block         └───────────┘
```

---

## Debugging Checklist

When your loop doesn't work:

1. **Does loop header have phi nodes?**
   - Each loop-carried variable needs a phi node in the header
   
2. **Do phi nodes have entries for ALL predecessors?**
   - Entry block: initial values
   - Body block: updated values
   
3. **Are you using the phi values in the loop body?**
   - Don't use the original registers; use the phi results
   
4. **Is exit block receiving values?**
   - Either via phi nodes OR via brif arguments
   
5. **Are blocks sealed in the right order?**
   - Seal loop header LAST (after back edge)
   
6. **Do argument counts match parameter counts?**
   - Check each jump/brif has correct number of arguments

---

## Files to Reference

1. **Your Correct Implementation:**
   - `/Users/amaterasu/Vibranium/rayzor/compiler/src/codegen/cranelift_backend.rs`
   - Lines 264-298: `collect_phi_args`
   - Lines 301-337: `translate_phi_node_static`
   - Lines 441-476: jump and brif translation

2. **Research Documents:**
   - `/Users/amaterasu/Vibranium/rayzor/compiler/CRANELIFT_LOOP_RESEARCH.md`
   - `/Users/amaterasu/Vibranium/rayzor/compiler/CRANELIFT_LOOP_DIAGRAMS.md`

3. **MIR Phi Node Definition:**
   - `/Users/amaterasu/Vibranium/rayzor/compiler/src/ir/blocks.rs` (lines 64-73)
   - Shows `IrPhiNode` structure with `dest`, `incoming`, and `ty`

4. **External References:**
   - Cranelift IR docs: https://github.com/bytecodealliance/wasmtime/blob/main/cranelift/docs/ir.md
   - Cranelift JIT demo: https://github.com/bytecodealliance/cranelift-jit-demo
   - Cranelift frontend API: https://docs.rs/cranelift-frontend/

---

## Summary

Your Cranelift backend implementation is **correct**. The issue is in your test's MIR construction:

- ✅ Your backend correctly translates phi nodes to block parameters
- ✅ Your backend correctly collects and passes phi arguments
- ✅ Your backend correctly handles brif with different arguments for each branch
- ❌ Your test doesn't create proper phi nodes in MIR
- ❌ Your test uses Copy instructions incorrectly

**Fix:** Add phi nodes to loop header in MIR, remove Copy instructions, use phi values in loop body.

**Alternative:** Wait for HIR→MIR pipeline to generate proper SSA form automatically.
