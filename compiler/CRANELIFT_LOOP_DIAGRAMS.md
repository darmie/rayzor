# Cranelift Loop Pattern: Visual Diagrams

## 1. Traditional SSA Phi Nodes vs Cranelift Block Parameters

### Traditional SSA IR (LLVM-style):
```
    Entry Block               Loop Header Block           Loop Body Block
┌─────────────────┐       ┌──────────────────────┐    ┌────────────────┐
│ sum_0 = 0       │       │ sum = phi(sum_0,     │    │ temp = sum + i │
│ i_0 = 1         │───────▶   sum_next)          │◀───│ sum_next = temp│
│                 │       │ i = phi(i_0, i_next) │    │ i_next = i + 1 │
│ goto loop_header│       │ if (i <= n)          │    │ goto loop_hdr  │
└─────────────────┘       │   goto body          │    └────────────────┘
                          │ else                 │
                          │   goto exit          │
                          └──────────┬───────────┘
                                     │
                                     ▼
                               ┌──────────┐
                               │ Exit     │
                               │ return   │
                               └──────────┘
```

### Cranelift IR (Block Parameters):
```
    Entry Block               Loop Header Block           Loop Body Block
┌─────────────────┐       ┌──────────────────────┐    ┌────────────────┐
│ v0 = 0          │       │ block1(sum:i64,      │    │ temp = sum + i │
│ v1 = 1          │───┬───▶  i:i64):             │◀─┬─│ sum_next=temp  │
│                 │   │   │ cond = (i <= n)      │  │ │ i_next = i + 1 │
│ jump block1(    │   │   │ brif cond,           │  │ │ jump block1(   │
│   v0, v1)       │   │   │   block2, block3(sum)│  │ │   sum_next,    │
└─────────────────┘   │   └──────────┬───────────┘  │ │   i_next)      │
                      │              │              │ └────────────────┘
  initial values ─────┘              │              └─── updated values
  passed as args                     │ false
                                     │ (pass sum to exit!)
                                     ▼
                               ┌───────────────┐
                               │ block3(result:│
                               │   i64):       │
                               │ return result │
                               └───────────────┘
```

**Key Difference:** Block parameters REPLACE phi nodes. Values flow via jump/brif arguments.

---

## 2. The Critical brif Pattern for Loop Exits

```
Loop Header: block1(sum: i64, i: i64)
┌────────────────────────────────────────────┐
│  cond = icmp sle i, n                      │
│                                            │
│  brif cond,                                │
│      block2,          // true: go to body │
│      block3(sum)      // false: exit with │
│                       //        final sum  │
└─────┬───────────────────────┬──────────────┘
      │ true                  │ false
      │ NO ARGUMENTS          │ PASS ARGUMENTS
      │ (body doesn't         │ (exit receives
      │  need them)           │  final values)
      ▼                       ▼
  ┌─────────┐          ┌──────────────┐
  │ block2: │          │ block3(sum): │
  │ Loop    │          │ return sum   │
  │ Body    │          └──────────────┘
  └─────────┘
```

**CRITICAL:** The `brif` instruction passes DIFFERENT arguments to each branch!

---

## 3. Complete While Loop Data Flow

```
Haxe Source:
  function sumToN(n: Int): Int {
    var sum = 0;
    var i = 1;
    while (i <= n) {
      sum = sum + i;
      i = i + 1;
    }
    return sum;
  }

Cranelift IR Data Flow:

  Entry (block0):
    n = param[0]
    ┌───────┐
    │ v0=0  │ sum initial value
    │ v1=1  │ i initial value
    └───┬───┘
        │
        │ jump block1(v0, v1)
        │
        ▼
  Loop Header (block1):
    ┌──────────────────────────────┐
    │ block1(sum: i64, i: i64)     │◀─────────┐
    │   cond = icmp sle i, n       │          │
    │   brif cond, block2, block3  │          │
    └─────┬────────────────────┬───┘          │
          │                    │              │
   true   │ (no args)   false  │ (with sum)   │
          ▼                    ▼              │
    ┌──────────┐         ┌────────────┐       │
    │ block2:  │         │ block3(r): │       │
    │ temp =   │         │ return r   │       │
    │  sum + i │         └────────────┘       │
    │ sum_next │                              │
    │  = temp  │                              │
    │ i_next = │                              │
    │  i + 1   │                              │
    │ jump ────┼──────────────────────────────┘
    │ block1(  │    jump with updated values
    │ sum_next,│    (back edge)
    │ i_next)  │
    └──────────┘

Data Flow per Iteration:
  Iter 0: block1(sum=0, i=1)   → i≤n? yes → block2 → block1(sum=1, i=2)
  Iter 1: block1(sum=1, i=2)   → i≤n? yes → block2 → block1(sum=3, i=3)
  ...
  Iter n: block1(sum=X, i=n+1) → i≤n? no  → block3(sum=X) → return X
```

---

## 4. Why Exit Block Needs Parameters (Option 1)

### Problem: How does exit block access final `sum`?

```
❌ WRONG: No way for exit to access sum
  block1(sum: i64, i: i64):
    brif cond, block2, block3
  
  block3:  // ⚠️ How do we get sum here?
    return ???  // sum is not in scope!

✅ CORRECT: Pass sum as argument
  block1(sum: i64, i: i64):
    brif cond, block2, block3(sum)
           │           │    └──── argument
           │           └────────── target
           └────────────────────── condition
  
  block3(result: i64):  // ✅ Receives sum via parameter
    return result
```

**Dominance Rule:** Block parameters dominate their entire block.  
Once you enter `block3`, you can't access `sum` from `block1` unless:
  1. It's passed as a block parameter (✅ clean)
  2. It's stored in memory/stack (⚠️ works but less efficient)
  3. The block dominates exit (❌ doesn't work for loops)

---

## 5. Alternative: Exit Block Without Parameters (Option 2)

Some Cranelift code uses memory for accumulation:

```
  Entry:
    ss0 = stack_slot 8  // Allocate stack for sum
    store ss0, 0        // sum = 0
    v1 = 1              // i = 1
    jump block1(v1)

  Loop Header (block1):
    block1(i: i64):                  // Only i as parameter
      temp_sum = load ss0             // Load sum from stack
      new_sum = iadd temp_sum, i      // sum + i
      store ss0, new_sum              // Store back
      i_next = iadd_imm i, 1
      cond = icmp sle i_next, n
      brif cond, block1(i_next), block2

  Exit (block2):
    final_sum = load ss0              // Load final sum
    return final_sum
```

**Trade-offs:**
- ✅ Exit block has no parameters
- ❌ More memory operations (loads/stores)
- ❌ Less optimization opportunities
- ❌ Not pure SSA form

**Recommendation:** Use block parameters for pure SSA unless you have a reason to use stack.

---

## 6. Comparison: 3-Block vs 4-Block Loop Structures

### 4-Block Structure (Recommended for Complex Loops)
```
  entry → loop_header → loop_body → back to loop_header
             ↓ exit
          exit_block
```

**Advantages:**
- Separation of condition check and body
- Easier to optimize
- Matches typical CFG structure

### 3-Block Structure (Possible for Simple Loops)
```
  entry → loop_body (with condition at end) → back edge
             ↓ exit
          exit_block
```

**When to Use:**
- Do-while loops (condition at end)
- Very simple loops
- When body must execute at least once

---

## 7. Sealing Order for Loops (IMPORTANT!)

```
Step 1: Create all blocks
  ┌─────┐  ┌──────┐  ┌──────┐  ┌──────┐
  │entry│  │header│  │ body │  │ exit │
  └─────┘  └──────┘  └──────┘  └──────┘

Step 2: Add entry block code
  ┌─────┐
  │entry│──┐
  └─────┘  │ jump
           ▼
         ┌──────┐
         │header│
         └──────┘

Step 3: Seal entry (all predecessors known)
  [✓] seal entry

Step 4: Add header code
         ┌──────┐
         │header│──┬─→ body
         └──────┘  │
            │      │
            └─→ exit

Step 5: Add body code (with back edge!)
         ┌──────┐
      ┌──│header│◀─┐
      │  └──────┘  │
      ▼            │
   ┌──────┐        │
   │ body │────────┘ back edge!
   └──────┘

Step 6: Seal body and exit (predecessors known)
  [✓] seal body
  [✓] seal exit

Step 7: Seal header LAST (after back edge!)
  [✓] seal header  ← MUST be last for loop!

❌ WRONG: Sealing header before adding back edge
  This breaks SSA construction for phi nodes!

✅ CORRECT: Seal header after ALL edges (including back edge)
```

**Rule:** Seal a block ONLY after ALL predecessors have branched to it.

---

## 8. Full Example: sum_to_n MIR with Phi Nodes

```rust
// MIR CFG Structure:

Block 0 (Entry):
  Instructions:
    v0 = const 0                    // sum initial
    v1 = const 1                    // i initial
  Terminator:
    branch block1                   // → loop header
  Phi nodes: []

Block 1 (Loop Header):
  Phi nodes:
    v_sum = phi[(0,v0), (2,v4)]    // sum: 0 from entry, v4 from body
    v_i = phi[(0,v1), (2,v5)]      // i: 1 from entry, v5 from body
  Instructions:
    v2 = cmp_le v_i, param_n        // i <= n?
  Terminator:
    cond_branch v2, block2, block3  // → body or exit

Block 2 (Loop Body):
  Instructions:
    v4 = add v_sum, v_i             // sum = sum + i
    v5 = add v_i, v1                // i = i + 1
  Terminator:
    branch block1                   // → back to header
  Phi nodes: []

Block 3 (Exit):
  Phi nodes:
    v_result = phi[(1,v_sum)]       // final sum from header
  Terminator:
    return v_result
```

**Key Points:**
1. Phi nodes appear at MERGE POINTS (block1, block3)
2. Block1 merges: entry (initial) + body (updated)
3. Block3 receives final value from block1
4. No Copy instructions needed!

---

## 9. Common Mistakes and Solutions

### Mistake 1: Using Copy in Loop Body ❌
```rust
body.instructions.push(IrInstruction::Copy {
    dest: sum_reg,    // ❌ Redefining sum (breaks SSA!)
    src: sum_new,
});
```

**Solution:** Use phi nodes in loop header ✅
```rust
loop_header.phi_nodes.push(IrPhiNode {
    dest: sum_reg,
    incoming: [(entry, v0), (body, sum_new)],
    ty: IrType::I64,
});
```

### Mistake 2: Sealing Header Before Back Edge ❌
```rust
builder.seal_block(loop_header);  // ❌ Too early!
builder.ins().jump(loop_header, &[...]); // Back edge added after sealing
```

**Solution:** Seal after ALL edges ✅
```rust
builder.ins().jump(loop_header, &[...]); // Add back edge first
builder.seal_block(loop_header);          // ✅ Now seal
```

### Mistake 3: Exit Block Can't Access Values ❌
```rust
builder.ins().brif(cond, body, &[], exit, &[]);  // ❌ No args to exit
// In exit block:
builder.ins().return_(&[sum]);  // ❌ sum not in scope!
```

**Solution:** Pass values to exit block ✅
```rust
builder.ins().brif(cond, body, &[], exit, &[sum]);  // ✅ Pass sum
// In exit block (with parameter):
let params = builder.block_params(exit);
let final_sum = params[0];
builder.ins().return_(&[final_sum]);  // ✅ Use parameter
```

---

## 10. Quick Reference: Jump vs Brif

### jump (Unconditional)
```rust
builder.ins().jump(target_block, &[arg1, arg2, ...]);
//                  └──target─┘   └─────arguments────┘
```

- Always goes to target_block
- Passes arguments to target's block parameters
- Arguments must match parameter types and count

### brif (Conditional - Binary)
```rust
builder.ins().brif(condition,
    true_block, &[true_arg1, true_arg2],
    false_block, &[false_arg1, false_arg2]
);
//  └─condition─┘  └─true_target─┘  └─false_target─┘
```

- Goes to true_block if condition is non-zero
- Goes to false_block if condition is zero
- Each target gets DIFFERENT arguments
- Arguments must match respective parameter types

### br_table (Switch/Jump Table)
```rust
builder.ins().br_table(index, default_block);
// Set jump table entries with builder.ins().jump_table_entry(...)
```

- For switch statements
- Less common, see Cranelift docs for details

---

## Summary Diagram: The Complete Picture

```
┌─────────────────────────────────────────────────────────────┐
│                    CRANELIFT LOOP PATTERN                    │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  Entry Block:                                               │
│    Initialize variables                                     │
│    jump loop_header(initial_values)  ← Pass initial values │
│                                                              │
│  Loop Header Block(phi_params):      ← Block parameters!   │
│    Compute condition                                        │
│    brif condition,                                          │
│      loop_body, &[],                 ← Body: no args       │
│      exit_block, &[final_values]     ← Exit: pass finals!  │
│                                                              │
│  Loop Body Block:                                           │
│    Compute updated values                                   │
│    jump loop_header(updated_values)  ← Pass updated vals   │
│                                                              │
│  Exit Block(result_params):          ← Receive finals!     │
│    Use result_params                                        │
│    return                                                   │
│                                                              │
│  Key Principles:                                            │
│  1. Block parameters = Phi nodes                            │
│  2. Jump/brif pass arguments to parameters                  │
│  3. Each branch can pass different arguments                │
│  4. Seal blocks AFTER all predecessors (including loops!)   │
│  5. Exit blocks CAN have parameters for clean SSA           │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

