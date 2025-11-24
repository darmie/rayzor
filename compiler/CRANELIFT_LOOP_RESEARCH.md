# Cranelift SSA Phi Nodes and Loops - Research Summary

## Executive Summary

Cranelift uses **block parameters** instead of traditional phi nodes. When a loop exits, the exit block receives final values via block parameters, just like any other control flow merge point. The `brif` instruction passes different argument lists for true and false branches.

---

## 1. How Cranelift Represents Phi Nodes Using Block Parameters

### Traditional SSA Phi Nodes
```
loop_header:
    i = phi [0, entry], [i_next, loop_body]
    sum = phi [0, entry], [sum_next, loop_body]
```

### Cranelift Block Parameters
```
block1(i: i64, sum: i64):  // Parameters act as phi nodes
    // Use i and sum directly
```

**Key Insight:** Instead of explicit phi instructions, blocks declare parameters. When branching to a block, you pass arguments that become the parameter values.

**From Documentation:**
> "Cranelift does not have phi instructions but uses BB parameters instead. A BB can be defined with a list of typed parameters."

---

## 2. Loop Exit Blocks Receiving Loop-Carried Values

### The Question
**"When a loop exits (condition becomes false), how do you pass the final loop variable values to the exit block?"**

### The Answer: Exit Blocks Have Block Parameters Too!

Exit blocks receive values the same way loop headers do - via block parameters passed through `jump` or `brif` instructions.

### Concrete Example from Cranelift IR Documentation

```cranelift
function average(i64) -> f64 {
block0(v0: i64):                    // Entry: parameter n
    v3 = iconst.i64 0               // Initial i = 0
    jump block3(v3)                 // Jump to loop with initial value

block3(v4: i64):                    // Loop header: i is block parameter
    v11 = iadd_imm v4, 1            // Compute i_next = i + 1
    v12 = icmp ult v11, v0          // Check if i < n
    brif v12, block3(v11), block4   // If true: loop back with v11
                                    // If false: exit to block4
                                    
block4:                             // Exit block (no parameters needed here)
    v13 = stack_load.f64 ss0        // Load accumulated sum from stack
    v14 = fcvt_from_uint.f64 v0
    v15 = fdiv v13, v14
    return v15
}
```

**Key Observations:**
1. Loop header `block3` has parameter `v4` (the induction variable `i`)
2. Back edge: `brif v12, block3(v11), block4`
   - True branch passes `v11` (next iteration value) to `block3`
   - False branch goes to `block4` with NO arguments
3. Exit block `block4` has no parameters because it uses stack memory for accumulation

---

## 3. Pattern for Passing Values When Branching

### Jump Instruction (Unconditional Branch)
```rust
// Cranelift IR
builder.ins().jump(target_block, &[arg1, arg2, arg3]);
```

**Arguments must match the target block's parameters in count and type.**

### Brif Instruction (Conditional Branch)
```rust
// Cranelift IR
builder.ins().brif(condition, 
    true_block, &[true_arg1, true_arg2],    // Arguments for true branch
    false_block, &[false_arg1, false_arg2]  // Arguments for false branch
);
```

**Each branch can pass DIFFERENT arguments to its target block.**

### Example: While Loop Pattern

```rust
// Pseudo-Haxe
var sum = 0;
var i = 1;
while (i <= n) {
    sum = sum + i;
    i = i + 1;
}
return sum;
```

**Cranelift IR Translation:**

```cranelift
block0(v_n: i64):                      // Entry block
    v0 = iconst.i64 0                  // sum = 0
    v1 = iconst.i64 1                  // i = 1
    jump block1(v1, v0)                // Jump to loop with (i=1, sum=0)

block1(v_i: i64, v_sum: i64):          // Loop header (phi nodes as params)
    v_cond = icmp sle v_i, v_n         // cond = (i <= n)
    brif v_cond, block2, block3(v_sum) // If true: body, if false: exit with sum

block2:                                 // Loop body
    v_sum_next = iadd v_sum, v_i       // sum_next = sum + i
    v_i_next = iadd_imm v_i, 1         // i_next = i + 1
    jump block1(v_i_next, v_sum_next)  // Loop back with updated values

block3(v_result: i64):                  // Exit block with parameter!
    return v_result                     // Return final sum
```

**Critical Insights:**
1. Loop header `block1` has parameters `(v_i, v_sum)` - these ARE the phi nodes
2. Entry jumps to loop: `jump block1(v1, v0)` passes initial values
3. Back edge: `jump block1(v_i_next, v_sum_next)` passes updated values
4. Exit via false branch: `brif v_cond, block2, block3(v_sum)`
   - **Exit block DOES have a parameter** to receive final sum!
   - Alternative: Exit block could have no parameters if loop writes to stack/memory

---

## 4. Two Approaches to SSA in Cranelift

### Approach A: Manual Block Parameters (What You're Doing)
**Pros:**
- Direct control over SSA form
- Matches academic SSA literature
- No hidden magic

**Cons:**
- Must manually create block parameters for every phi node
- Must pass correct arguments on every branch
- Error-prone for complex control flow

**When to Use:** When your IR already has explicit phi nodes (like MIR with IrPhiNode).

### Approach B: Cranelift Variable API (Recommended for Simple Cases)
**Pros:**
- Automatic SSA construction
- Use `def_var` and `use_var` like mutable variables
- Cranelift inserts phi nodes automatically

**Cons:**
- Less explicit control
- Slight overhead for immutable values
- Not suitable if you already have SSA form

**When to Use:** When translating from non-SSA source (AST/bytecode).

---

## 5. The Current Implementation Analysis

### What the Code Does Correctly ✅

From `/Users/amaterasu/Vibranium/rayzor/compiler/src/codegen/cranelift_backend.rs`:

```rust
// Line 331: Append block parameter for phi node
let block_param = builder.append_block_param(current_block, cl_type);
value_map.insert(phi_node.dest, block_param);
```

**Correct!** Phi nodes become block parameters.

```rust
// Line 454: Jump with phi arguments
let phi_args = Self::collect_phi_args(value_map, function, *target, current_block_id)?;
builder.ins().jump(cl_block, &phi_args);
```

**Correct!** Passes arguments when jumping.

```rust
// Line 476: Brif with separate arguments for each branch
let true_phi_args = Self::collect_phi_args(value_map, function, *true_target, current_block_id)?;
let false_phi_args = Self::collect_phi_args(value_map, function, *false_target, current_block_id)?;
builder.ins().brif(cond_val, true_block, &true_phi_args, false_block, &false_phi_args);
```

**Perfect!** Each branch passes its own arguments.

### The Problem with Your Test ❌

From `/Users/amaterasu/Vibranium/rayzor/compiler/examples/test_cranelift_loop.rs`:

```rust
// Loop body: sum = sum + i; i = i + 1
body.instructions.push(IrInstruction::Copy {
    dest: sum_reg,
    src: sum_new,
});
body.instructions.push(IrInstruction::Copy {
    dest: i_reg,
    src: i_new,
});
```

**Problem:** You're using `Copy` instructions to "update" variables in SSA form, which is semantically incorrect. In SSA, each value is assigned exactly once. You can't copy to an existing register; you must create new values and pass them via block parameters.

**What Your MIR Should Look Like:**

```rust
// Loop header needs phi nodes!
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
```

---

## 6. Correct Loop Pattern for Rayzor

### MIR Structure Needed

```rust
// Entry Block (bb0)
entry:
    const_0 = 0
    const_1 = 1
    branch loop_header  // No arguments needed; loop header has phi nodes

// Loop Header (bb1) - Has PHI nodes
loop_header:
    phi sum_phi = [const_0 from entry], [sum_new from loop_body]
    phi i_phi = [const_1 from entry], [i_new from loop_body]
    cond = (i_phi <= n)
    cond_branch cond, loop_body, exit_block

// Loop Body (bb2)
loop_body:
    sum_new = sum_phi + i_phi
    i_new = i_phi + 1
    branch loop_header  // Back edge

// Exit Block (bb3)
exit_block:
    return sum_phi  // Use phi value from loop header
```

### Cranelift Translation

```rust
// 1. Create all blocks
let entry = builder.create_block();
let loop_header = builder.create_block();
let loop_body = builder.create_block();
let exit_block = builder.create_block();

// 2. Entry block
builder.switch_to_block(entry);
let v0 = builder.ins().iconst(types::I64, 0);
let v1 = builder.ins().iconst(types::I64, 1);
builder.ins().jump(loop_header, &[v0, v1]);  // Pass initial values

// 3. Loop header with block parameters (phi nodes)
builder.append_block_param(loop_header, types::I64);  // sum parameter
builder.append_block_param(loop_header, types::I64);  // i parameter
builder.switch_to_block(loop_header);
let params = builder.block_params(loop_header);
let sum_phi = params[0];
let i_phi = params[1];

let cond = builder.ins().icmp(IntCC::SignedLessThanOrEqual, i_phi, v_n);
builder.ins().brif(cond, loop_body, &[], exit_block, &[sum_phi]);
//                              ^^^^                     ^^^^^^^^
//                         Body needs no args      Exit receives final sum

// 4. Loop body
builder.switch_to_block(loop_body);
let sum_new = builder.ins().iadd(sum_phi, i_phi);
let i_new = builder.ins().iadd_imm(i_phi, 1);
builder.ins().jump(loop_header, &[sum_new, i_new]);  // Pass updated values

// 5. Exit block
builder.append_block_param(exit_block, types::I64);  // Receive final sum
builder.switch_to_block(exit_block);
let exit_params = builder.block_params(exit_block);
let final_sum = exit_params[0];
builder.ins().return_(&[final_sum]);

// 6. Seal blocks after all predecessors are known
builder.seal_block(entry);
builder.seal_block(loop_body);
builder.seal_block(loop_header);  // Seal after back edge added
builder.seal_block(exit_block);
```

---

## 7. Answer to Your Key Question

> **"When a loop exits (condition becomes false), how do you pass the final loop variable values to the exit block? Should the exit block have block parameters?"**

### Answer: YES, the exit block CAN have block parameters!

**Two valid approaches:**

### Option 1: Exit Block with Parameters (Recommended)
```rust
builder.ins().brif(cond, 
    loop_body, &[],                // True: go to body (no args needed)
    exit_block, &[sum_phi, i_phi]  // False: pass final values to exit
);
```

**Advantages:**
- Clean SSA form
- Values flow through control edges
- No memory operations needed

### Option 2: Exit Block without Parameters (Alternative)
```rust
// Exit block has no parameters
builder.ins().brif(cond, loop_body, &[], exit_block, &[]);

// In exit block:
builder.switch_to_block(exit_block);
// Use sum_phi directly (still in scope if you saved it)
builder.ins().return_(&[sum_phi]);
```

**Note:** This only works if the value is still accessible (e.g., from a dominating block or memory).

---

## 8. Common Patterns from Wasmtime

From Wasmtime's WebAssembly translation:

### Pattern 1: Loop with Stack Accumulation
```
block3(v4: i64):              // Loop header: iteration variable only
    v11 = iadd_imm v4, 1
    v12 = icmp ult v11, v0
    brif v12, block3(v11), block4
    
block4:                        // Exit: no parameters
    v13 = stack_load.f64 ss0   // Load accumulated value from stack
    return v13
```

### Pattern 2: Loop with Value Parameters
```
block4(v13: i64, v17: i64, v18: i64):  // Multiple phi values
    v15 = icmp ne v13, v14
    brif v15, block5, block6(v17)      // Pass value to exit block
    
block6(v22: i64):                      // Exit block receives value
    return v22
```

---

## 9. Implementation Recommendations for Rayzor

### Short-term Fix (Use Your Existing MIR with Phi Nodes)

Your `collect_phi_args` and `translate_phi_node_static` methods are **correct**. The issue is in your test's MIR construction.

**Fix the test:**
1. Add explicit phi nodes to loop header in MIR
2. Remove Copy instructions (they're not needed with phi nodes)
3. Ensure phi nodes have incoming edges from all predecessors

### Medium-term (Improve HIR→MIR Pipeline)

Make sure your HIR→MIR lowering creates proper phi nodes for:
- Loop-carried variables
- Variables defined in if/else branches
- Variables escaping block scope

### Long-term Alternative (Variable API)

Consider adding a parallel path that uses Cranelift's Variable API for simple cases:

```rust
let sum_var = Variable::new(0);
let i_var = Variable::new(1);

builder.declare_var(sum_var, types::I64);
builder.declare_var(i_var, types::I64);

builder.def_var(sum_var, v0);  // sum = 0
builder.def_var(i_var, v1);    // i = 1

// In loop:
let sum = builder.use_var(sum_var);
let i = builder.use_var(i_var);
// ... compute ...
builder.def_var(sum_var, sum_new);
builder.def_var(i_var, i_new);
```

**Pros:** Simpler for code generation, automatic phi insertion  
**Cons:** Redundant if you already have phi nodes in MIR

---

## 10. Critical Cranelift Concepts

### Block Sealing
**Must seal blocks after ALL predecessors are processed, including back edges!**

```rust
// For loops: seal header AFTER adding back edge
builder.ins().jump(loop_header, &[...]); // Back edge
builder.seal_block(loop_header);          // NOW you can seal
```

### Block Parameter Order
**Arguments must match parameter order exactly:**

```rust
// Declaration order matters:
builder.append_block_param(block, types::I64); // Parameter 0
builder.append_block_param(block, types::I32); // Parameter 1

// Must jump with matching types:
builder.ins().jump(block, &[i64_value, i32_value]); // ✅
builder.ins().jump(block, &[i32_value, i64_value]); // ❌ Wrong order!
```

### Dominance
**Block parameters dominate the entire block:**

```rust
block1(v_param: i64):
    // v_param is available everywhere in block1
    v2 = iadd v_param, v_param  // ✅ Valid
```

---

## 11. Summary Checklist

For implementing loops in Cranelift:

- [x] ✅ Loop header block has parameters for loop-carried values
- [x] ✅ Entry block jumps to loop header with initial values
- [x] ✅ Loop body computes new values for next iteration
- [x] ✅ Back edge jumps to loop header with updated values
- [x] ✅ Exit condition uses `brif` with DIFFERENT arguments for each branch
- [x] ✅ Exit block CAN have parameters to receive final values
- [x] ✅ All blocks sealed after predecessors are known (including back edges)
- [x] ✅ Block parameter types match jump argument types

---

## 12. References

1. **Cranelift IR Documentation**
   - https://github.com/bytecodealliance/wasmtime/blob/main/cranelift/docs/ir.md
   - Block parameters, control flow, SSA form

2. **Cranelift Frontend API**
   - https://docs.rs/cranelift-frontend/
   - Variable API, FunctionBuilder, SSA construction

3. **Cranelift JIT Demo**
   - https://github.com/bytecodealliance/cranelift-jit-demo
   - Complete toy language compiler with loops

4. **Wasmtime Code Translator**
   - https://github.com/bytecodealliance/wasmtime/blob/main/cranelift/wasm/src/code_translator.rs
   - Real-world WebAssembly→Cranelift translation

5. **Your Implementation**
   - `/Users/amaterasu/Vibranium/rayzor/compiler/src/codegen/cranelift_backend.rs`
   - Already has correct phi node → block parameter translation!
   - Lines 301-337: `translate_phi_node_static`
   - Lines 441-476: `collect_phi_args` for jump/brif

---

## Appendix: Minimal Working Example

```rust
// Function: sum_to_n(n: i64) -> i64
//   sum = 0; i = 1
//   while (i <= n) { sum += i; i++ }
//   return sum

let mut backend = CraneliftBackend::new()?;
let func_id = IrFunctionId(0);

let mut function = IrFunction::new(...);

// Create blocks
let entry = function.cfg.entry_block;
let loop_header = function.cfg.create_block();
let loop_body = function.cfg.create_block();
let exit_block = function.cfg.create_block();

// Define phi nodes for loop header
let sum_phi = IrId::new(10);
let i_phi = IrId::new(11);

function.cfg.blocks.get_mut(&loop_header).unwrap().phi_nodes = vec![
    IrPhiNode {
        dest: sum_phi,
        incoming: vec![
            (entry, const_0_id),         // From entry: 0
            (loop_body, sum_new_id),     // From body: sum + i
        ],
        ty: IrType::I64,
    },
    IrPhiNode {
        dest: i_phi,
        incoming: vec![
            (entry, const_1_id),         // From entry: 1
            (loop_body, i_new_id),       // From body: i + 1
        ],
        ty: IrType::I64,
    },
];

// Entry: initialize and jump to loop
function.cfg.blocks.get_mut(&entry).unwrap().instructions = vec![
    IrInstruction::Const { dest: const_0_id, value: IrValue::I64(0) },
    IrInstruction::Const { dest: const_1_id, value: IrValue::I64(1) },
];
function.cfg.blocks.get_mut(&entry).unwrap().terminator = 
    IrTerminator::Branch { target: loop_header };

// Loop header: check condition
function.cfg.blocks.get_mut(&loop_header).unwrap().instructions = vec![
    IrInstruction::Cmp {
        dest: cond_id,
        op: CompareOp::Le,
        left: i_phi,
        right: param_n_id,
    },
];
function.cfg.blocks.get_mut(&loop_header).unwrap().terminator = 
    IrTerminator::CondBranch {
        condition: cond_id,
        true_target: loop_body,
        false_target: exit_block,
    };

// Loop body: compute and loop back
function.cfg.blocks.get_mut(&loop_body).unwrap().instructions = vec![
    IrInstruction::BinOp {
        dest: sum_new_id,
        op: BinaryOp::Add,
        left: sum_phi,
        right: i_phi,
    },
    IrInstruction::BinOp {
        dest: i_new_id,
        op: BinaryOp::Add,
        left: i_phi,
        right: const_1_id,
    },
];
function.cfg.blocks.get_mut(&loop_body).unwrap().terminator = 
    IrTerminator::Branch { target: loop_header };

// Exit: return sum
// Option 1: Add phi node to exit block to receive final sum
function.cfg.blocks.get_mut(&exit_block).unwrap().phi_nodes = vec![
    IrPhiNode {
        dest: final_sum_id,
        incoming: vec![
            (loop_header, sum_phi),  // Final value from loop
        ],
        ty: IrType::I64,
    },
];
function.cfg.blocks.get_mut(&exit_block).unwrap().terminator = 
    IrTerminator::Return { value: Some(final_sum_id) };

// Compile
backend.compile_module(&module)?;
```

**This will correctly generate Cranelift IR with block parameters!**
