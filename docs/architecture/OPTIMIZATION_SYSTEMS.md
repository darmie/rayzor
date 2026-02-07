# MIR Optimization Systems

Rayzor applies optimization passes to MIR (Mid-level IR) before handing it to
the Cranelift or LLVM backend. Passes are composed into pipelines via the
`PassManager` and selected by optimization level (`-O0` through `-O3`).

## Optimization Levels

### O0 — Minimal

```
InsertFree          (correctness — frees non-escaping allocations)
Inlining            (forced: max_inline_size=15, Haxe `inline` + small constructors)
DCE
SRA
CopyProp
DCE                 (cleanup)
```

Despite being "O0", forced inlining and SRA are required for correctness:
inlining exposes constructor Alloc+GEP patterns that SRA replaces with scalar
registers, preventing OOM from per-iteration heap allocations.

### O1 — Fast

```
InsertFree
Inlining            (standard cost model)
DCE
ConstantFolding
CopyProp
UnreachableBlockElim
```

Low-overhead optimizations for scripts and one-shot programs. No SRA, no loop
optimizations.

### O2 — Standard (default)

```
InsertFree
Inlining
DCE
SRA
ConstantFolding
CopyProp
GlobalLoadCaching
BoundsCheckElim
CSE
LICM
ControlFlowSimplify
UnreachableBlockElim
DCE                 (cleanup)
```

The default level. Adds SRA (memory), bounds check elimination (array loops),
global load caching, CSE, and LICM. Good balance of compile time and runtime
performance.

### O3 — Aggressive

```
InsertFree
Inlining
GlobalLoadCaching
DCE
SRA
ConstantFolding
CopyProp
BoundsCheckElim
GVN
CSE
LICM
LoopVectorization
TailCallOpt
ControlFlowSimplify
UnreachableBlockElim
DCE                 (cleanup)
```

Maximum runtime performance. Adds GVN (cross-block redundancy elimination),
loop vectorization, and tail call optimization.

## Pass Descriptions

### Correctness Passes

#### InsertFree

**File**: `compiler/src/ir/insert_free.rs`

Inserts `Free` instructions for heap allocations that do not escape the
function. Runs at **all** optimization levels.

Algorithm:
1. Find all `Alloc` and `malloc` call results
2. Build derived pointer set (GEP, Cast, BitCast, Copy chains)
3. Check escape conditions: return value, function argument, Store value,
   struct field, global, memcpy, phi node
4. Insert `Free` before each return for non-escaping allocations

### Inlining

**File**: `compiler/src/ir/inlining.rs`

Clones callee function bodies into call sites, eliminating call overhead and
exposing optimization opportunities for subsequent passes.

Cost model:
- `max_inline_size` controls the instruction-count threshold (15 at O0, higher
  at O2/O3)
- Functions marked `InlineHint::Always` (Haxe `inline` keyword) are always
  inlined
- Call sites weighted by loop depth — hotter calls get higher priority
- Recursive functions are never inlined

Pipeline position: runs **before** SRA so that inlined constructor bodies
expose Alloc+GEP patterns for scalar replacement.

### Scalar Replacement of Aggregates (SRA)

**File**: `compiler/src/ir/scalar_replacement.rs`

Replaces non-escaping struct and array allocations with individual scalar
registers, eliminating heap allocation entirely.

Two modes:
- **Regular SRA**: function-local allocations accessed via GEP+Load/Store
- **Phi-SRA**: allocations flowing through phi nodes in loops (cross-block)

Algorithm:
1. Identify candidates: `Alloc` instructions with no field escapes
2. Map GEP results to field indices
3. Create one scalar register per field (deterministic order via `BTreeMap`)
4. Replace GEP/Load with scalar register loads; Store with register writes
5. Remove now-unused Alloc/Free instructions

Disable with `RAYZOR_NO_SRA=1`. Disable phi-SRA only with `RAYZOR_NO_PHI_SRA=1`.

### Dead Code Elimination (DCE)

**File**: `compiler/src/ir/optimization.rs`

Removes unused instructions and phi nodes that do not contribute to function
output. Marks all registers used by terminators, instructions, and phi nodes;
removes instructions without destinations unless they have side effects.

### Constant Folding

**File**: `compiler/src/ir/optimization.rs`

Evaluates binary operations and comparisons with constant operands at
compile time. Supported operations: integer arithmetic (Add, Sub, Mul, Div,
Rem), floating-point (FAdd, FSub, FMul, FDiv), bitwise (And, Or, Xor, Shl,
Shr).

### Copy Propagation

**File**: `compiler/src/ir/optimization.rs`

Eliminates `Copy` instructions by replacing all uses of the copy destination
with the original source register. Cleanup pass that enables other
optimizations.

### Global Load Caching

**File**: `compiler/src/ir/optimization.rs`

Caches repeated loads of the same global variable within a function. Identifies
globals that are never stored to, builds a dominator tree, and replaces
subsequent loads with the first load's result when the first load dominates all
use sites.

Eliminates expensive `rayzor_global_load` (HashMap lookup) calls in hot code.

### Bounds Check Elimination (BCE)

**File**: `compiler/src/ir/bounds_check_elimination.rs`

Eliminates redundant array bounds checks in for-in loops by replacing
`haxe_array_get_ptr` calls with inline pointer arithmetic.

Detection: identifies header bounds check (`idx < arr.len`) and replaces the
array element access with `Load arr.ptr` + `Mul idx * elem_size` + `Add`.

Handles both:
1. **Stack-slot pattern** (pre-optimization): index loaded from `Alloc(I64)`
2. **Phi pattern** (post-optimization): index is a phi node in the loop header

Pipeline position: after GlobalLoadCaching, before CSE/LICM (so LICM can
hoist the invariant `arr.ptr` and `arr.elem_size` loads).

### Common Subexpression Elimination (CSE)

**File**: `compiler/src/ir/optimization.rs`

Eliminates redundant computations within a single basic block using value
numbering. Builds expression keys for instructions (commutative operations
normalized to canonical order) and replaces duplicates with the first result.

### Global Value Numbering (GVN)

**File**: `compiler/src/ir/optimization.rs`

More powerful than local CSE. Finds redundancies across multiple basic blocks
using dominance information. Processes blocks in dominator-tree preorder,
maintains a value number table, and replaces redundant expressions. Uses
transitive closure of the replacement map to handle chains (e.g., L1 -> L2 -> L3).

Available at O3 only.

### Loop Invariant Code Motion (LICM)

**File**: `compiler/src/ir/optimization.rs`

Hoists loop-invariant computations out of loops and sinks allocations out of
loop bodies.

Algorithm:
1. Identify loop structure using `DominatorTree` and `LoopNestInfo`
2. Iteratively find instructions whose operands are all defined outside the
   loop or already proven invariant
3. Verify hoisting is safe (instruction dominates all loop exit blocks)
4. Create preheader block if needed; hoist instructions there
5. Special handling for `Alloc`: escape analysis determines if allocation can
   be hoisted; non-escaping `Free` is sunk to loop exit

### Control Flow Simplification

**File**: `compiler/src/ir/optimization.rs`

Simplifies conditional branches with constant conditions into unconditional
branches. Detects when a condition register holds a constant `Bool` value and
replaces `CondBranch` with `Branch` to the appropriate target.

### Unreachable Block Elimination

**File**: `compiler/src/ir/optimization.rs`

Removes basic blocks unreachable from the entry block. Builds a reachability
set via worklist starting from entry, then removes blocks not in the set.

### Tail Call Optimization

**File**: `compiler/src/ir/optimization.rs`

Identifies and marks tail calls. A call is in tail position when it is the last
instruction before a return and its result is the return value (or both are
void). Tracks self-recursive tail calls separately for potential
recursion-to-loop conversion.

Available at O3 only.

### Loop Vectorization

**File**: `compiler/src/ir/vectorization.rs`

Auto-vectorizes scalar loops to SIMD operations. Targets 128-bit (SSE/NEON)
and 256-bit (AVX) vector widths. Supported vector types: V4F32, V2F64, V4I32,
V8I16, V16I8. Operations: element-wise binary, broadcast, extract, insert,
horizontal reduction, masked ops, gather/scatter.

Available at O3 only. Runs after LICM (which prepares loops by hoisting
invariants).

### Tree-Shaking

**File**: `compiler/src/ir/tree_shake.rs`

Removes unreachable functions, extern declarations, and globals from `.rzb`
bundles. Not part of the regular `PassManager` pipeline — invoked separately by
`rayzor bundle --strip`.

Algorithm: worklist walks the call graph from the entry function, following
`CallDirect`, `FunctionRef`, `MakeClosure`, `LoadGlobal`, and `StoreGlobal`
references. Retains only reachable definitions.

## PassManager

**File**: `compiler/src/ir/optimization.rs`

```rust
pub trait OptimizationPass {
    fn name(&self) -> &'static str;
    fn run_on_module(&mut self, module: &mut IrModule) -> OptimizationResult;
}

pub struct PassManager {
    passes: Vec<Box<dyn OptimizationPass>>,
}
```

The `PassManager` runs all passes in sequence. After each full pipeline
iteration it checks whether any "transformative" pass modified the IR. Cleanup
passes (DCE, CopyProp, UnreachableBlockElim) do not count as transformative.
If only cleanup passes made changes, the pipeline converges and stops. Maximum
5 iterations.

Key methods:
- `PassManager::for_level(level)` — builds the pipeline for O0-O3
- `PassManager::run(module)` — executes the pipeline with iteration logic

### Pipeline Ordering Constraints

Some passes depend on the results of others:

| Dependency | Reason |
| ---------- | ------ |
| Inlining before SRA | SRA needs inlined constructor bodies to see Alloc+GEP |
| GlobalLoadCaching before BCE | BCE benefits from cached array length loads |
| BCE/CSE before LICM | LICM hoists invariant loads exposed by earlier passes |
| LICM before Vectorization | LICM prepares loops by hoisting invariant code |

## Environment Variables

| Variable | Effect |
| -------- | ------ |
| `RAYZOR_NO_FMA=1` | Disable FMA fusion in Cranelift/LLVM instruction lowering |
| `RAYZOR_NO_SRA=1` | Disable all SRA passes |
| `RAYZOR_NO_PHI_SRA=1` | Disable phi-aware SRA only (regular SRA still runs) |
| `RAYZOR_RAW_MIR=1` | Skip all optimization passes in `rayzor dump` |
| `RAYZOR_PASS_DEBUG=1` | Run passes one-at-a-time with per-pass change reporting |
| `RAYZOR_DUMP_LLVM_IR=1` | Print LLVM IR before/after optimization (LLVM backend) |
| `RAYZOR_LLVM_OPT=<0-3>` | Override LLVM optimization level |

## Summary

| Pass | O0 | O1 | O2 | O3 | Category |
| ---- | -- | -- | -- | -- | -------- |
| InsertFree | x | x | x | x | Correctness |
| Inlining | x | x | x | x | Inlining |
| DCE | x | x | x | x | Dead code |
| SRA | x | | x | x | Memory |
| CopyProp | x | x | x | x | Cleanup |
| ConstantFolding | | x | x | x | Folding |
| UnreachableBlockElim | | x | x | x | Dead code |
| GlobalLoadCaching | | | x | x | Memory |
| BCE | | | x | x | Array |
| CSE | | | x | x | Redundancy |
| LICM | | | x | x | Loop |
| ControlFlowSimplify | | | x | x | Control flow |
| GVN | | | | x | Redundancy |
| LoopVectorization | | | | x | Loop |
| TailCallOpt | | | | x | Control flow |
| Tree-Shake | bundle --strip | | | | Bundling |
