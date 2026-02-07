# Compiler Analysis Phases

Rayzor compiles Haxe source code through a multi-stage pipeline. Each stage
produces a distinct intermediate representation and runs dedicated analyses
before handing off to the next stage.

```
Source (.hx)
    │
    ▼
┌──────────┐
│  Parse   │  → AST (HaxeFile)
└──────────┘
    │
    ▼
┌──────────────────┐
│  AST Lowering &  │  → TAST (TypedExpression, TypedStatement)
│  Type Checking   │
└──────────────────┘
    │
    ▼
┌──────────────────┐
│  Semantic Graph  │  → CFG, DFG, Call Graph, Ownership Graph
│  Construction    │
└──────────────────┘
    │
    ▼
┌──────────────┐
│  TAST → HIR  │  → HirModule (desugared, monomorphized)
└──────────────┘
    │
    ▼
┌──────────────┐
│  HIR → MIR   │  → IrModule (SSA form, basic blocks)
└──────────────┘
    │
    ▼
┌──────────────────┐
│  Optimization    │  → IrModule (optimized)
│  Passes (O0-O3)  │
└──────────────────┘
    │
    ▼
┌──────────────────┐
│  Code Generation │  → Machine code (Cranelift / LLVM / Interpreter)
└──────────────────┘
```

## 1. Parsing

**Input**: Haxe source text
**Output**: AST (`HaxeFile`)

The parser crate produces an untyped abstract syntax tree. Parse errors are
recovered where possible so that downstream phases can report multiple errors
in a single run.

## 2. AST Lowering and Type Checking

**Input**: AST
**Output**: TAST (Typed Abstract Syntax Tree)

This is the largest phase in the compiler. It converts the untyped AST into a
fully typed, symbol-resolved representation.

### Key files

| File | Role |
| ---- | ---- |
| `tast/ast_lowering.rs` | Symbol resolution, scope building, implicit `this` |
| `tast/type_checker.rs` | Type inference and validation |
| `tast/constraint_solver.rs` | Generic constraint solving |
| `tast/type_resolution.rs` | Type lookup and compatibility |
| `tast/control_flow_analysis.rs` | Variable state tracking across branches |
| `tast/null_safety_analysis.rs` | Null pointer detection |
| `tast/scopes.rs` | Hierarchical scope tree |
| `tast/symbols.rs` | Symbol table and symbol kinds |
| `tast/namespace.rs` | Module and package path resolution |
| `tast/core.rs` | TypeTable, TypeKind definitions |
| `tast/class_builder.rs` | Class/interface metadata construction |

### What happens

1. **Symbol resolution** — identifiers are resolved to `SymbolId` values via
   the scope tree. Each symbol carries its kind (Variable, Function, Class,
   Enum, TypeAlias, etc.) and type information.

2. **Scope building** — a hierarchical `ScopeTree` is constructed. Nested
   scopes for blocks, loops, and classes allow shadowing and lexical lookup.

3. **Type inference** — unannotated expressions get inferred types. The
   constraint solver handles generics by collecting type constraints and
   solving them.

4. **Implicit `this` insertion** — field assignments in class
   methods/constructors (e.g., `i = value` where `i` is a field) are
   rewritten to `this.i = value` with a `FieldAccess` node.

5. **Flow-sensitive analysis** — `TypeFlowGuard` tracks type narrowing
   through control flow (e.g., after a `null` check, the variable is
   non-nullable). `ControlFlowAnalysis` tracks variable initialization and
   definite assignment.

6. **Null safety** — `NullSafetyAnalysis` flags potential null dereferences
   and validates nullable type usage.

### Key data structures

- **SymbolTable**: global registry mapping `SymbolId` → `Symbol`
- **ScopeTree**: hierarchical scope chain for lexical lookup
- **TypeTable**: arena-allocated type registry with interning
- **StringInterner**: deduplicated string storage (`InternedString(u32)`)

### Error reporting

Type errors are collected (not fatal) so the compiler can report all issues at
once. Error kinds include: `TypeMismatch`, `UndefinedSymbol`, `UndefinedType`,
`InvalidTypeArguments`, `ConstraintViolation`, `CircularDependency`,
`AccessViolation`, `InferenceFailed`, `InterfaceNotImplemented`.

## 3. Semantic Graph Construction

**Input**: TAST
**Output**: Control Flow Graph, Data Flow Graph, Call Graph, Ownership Graph

These graphs provide the foundation for safety analyses (lifetimes, ownership,
escape detection).

### Key files

| File | Role |
| ---- | ---- |
| `semantic_graph/builder.rs` | Constructs all graphs from TAST |
| `semantic_graph/cfg.rs` | Control Flow Graph |
| `semantic_graph/dfg.rs` | Data Flow Graph |
| `semantic_graph/dfg_builder.rs` | DFG construction with SSA conversion |
| `semantic_graph/call_graph.rs` | Inter-procedural call graph |
| `semantic_graph/dominance.rs` | Dominator tree analysis |
| `semantic_graph/ownership_graph.rs` | Ownership and borrow tracking |
| `semantic_graph/validation.rs` | Graph consistency checks |

### Graphs produced

```rust
pub struct SemanticGraphs {
    pub control_flow: IdMap<SymbolId, ControlFlowGraph>,
    pub data_flow: IdMap<SymbolId, DataFlowGraph>,
    pub call_graph: CallGraph,
    pub ownership_graph: OwnershipGraph,
}
```

- **CFG** — basic blocks and edges for each function. Used for reachability,
  dominance, and loop detection.
- **DFG** — data dependencies, use-def chains, liveness information. Built in
  SSA form via dominance frontier phi placement.
- **Call Graph** — function call relationships. Used for recursion detection
  and inter-procedural analysis.
- **Ownership Graph** — tracks object ownership through moves and borrows.

### Dominance analysis

`semantic_graph/dominance.rs` computes dominator trees and dominance frontiers.
These are used for:
- SSA phi node placement in the DFG builder
- Loop identification (back edges)
- Code motion safety checks in LICM

## 4. Semantic Analysis Passes

These run over the semantic graphs to validate safety properties.

### Key files

| File | Role |
| ---- | ---- |
| `semantic_graph/analysis/analysis_engine.rs` | Orchestrates all analyses |
| `semantic_graph/analysis/lifetime_analyzer.rs` | Lifetime assignment and validation |
| `semantic_graph/analysis/lifetime_solver.rs` | Lifetime constraint solving |
| `semantic_graph/analysis/ownership_analyzer.rs` | Move/borrow validation |
| `semantic_graph/analysis/escape_analyzer.rs` | Heap allocation escape detection |
| `semantic_graph/analysis/deadcode_analyzer.rs` | Unreachable code detection |

### Lifetime analysis

Assigns lifetimes to all values and enforces outlives constraints. Detects
use-after-free and borrow violations at compile time.

### Ownership analysis

Tracks value ownership through function calls. Validates move semantics and
detects multiple mutable borrows.

### Escape analysis

Determines whether heap allocations escape their defining function. Results
feed into the MIR-level `InsertFree` pass — non-escaping allocations get
automatic `Free` instructions.

### Dead code analysis

Identifies unreachable code paths and unused variables/functions. Feeds into
tree-shaking for `.rzb` bundles.

## 5. TAST to HIR Lowering

**Input**: TAST
**Output**: HIR (`HirModule`, `HirFunction`, `HirExpression`)

**Key file**: `ir/tast_to_hir.rs`

This phase desugars high-level constructs into simpler operations:

- **For-in loops** → iterator call sequences
- **Pattern matching** → nested conditionals
- **Range expressions** → array allocations
- **Constant evaluation** — static `inline var` initializers with arithmetic
  (including bit shifts, bitwise ops) are evaluated at compile time
- **Type instantiation** — monomorphizes generic functions

## 6. HIR to MIR Lowering

**Input**: HIR
**Output**: MIR (`IrModule`, `IrFunction`, `IrBasicBlock`, `IrInstruction`)

**Key file**: `ir/hir_to_mir.rs` (the largest file in the compiler)

This phase produces the SSA-form MIR that all optimization passes and backends
consume.

### What happens

1. **Basic block construction** — code is partitioned into basic blocks with
   single-entry, single-exit control flow.

2. **SSA conversion** — variables become SSA registers. Phi nodes are inserted
   at control flow merge points.

3. **Instruction selection** — HIR expressions are lowered to primitive MIR
   instructions: `Add`, `Sub`, `Mul`, `Load`, `Store`, `GEP`, `Call`, etc.

4. **Call site resolution** — function calls are resolved first by SymbolId,
   then by qualified name (handles cases where call-site and definition have
   different SymbolIds).

5. **Memory layout** — struct field accesses become `GEP` (Get Element
   Pointer) instructions with computed byte offsets.

6. **Type coercion** — `Cast` instructions are inserted where Haxe allows
   implicit conversions (e.g., `Int` → `Float` at call sites).

### MIR structure

```
IrModule
├── functions: BTreeMap<IrFunctionId, IrFunction>
├── globals: HashMap<IrGlobalId, IrGlobal>
├── types: HashMap<IrTypeDefId, IrTypeDef>
└── extern_functions: BTreeMap<IrFunctionId, IrExternFunction>

IrFunction
├── signature: IrFunctionSignature (params, return type)
├── cfg: IrControlFlowGraph
│   ├── entry_block: IrBlockId
│   └── blocks: BTreeMap<IrBlockId, IrBasicBlock>
├── locals: HashMap<IrId, IrLocal>
└── register_types: HashMap<IrId, IrType>

IrBasicBlock
├── phi_nodes: Vec<IrPhiNode>
├── instructions: Vec<IrInstruction>
├── terminator: IrTerminator
└── predecessors: Vec<IrBlockId>
```

Registers are displayed as `$0`, `$1`, etc. Blocks as `bb0`, `bb1`.
Functions as `fn0`, `fn1`. See [DEBUGGING_MIR.md](DEBUGGING_MIR.md) for the
full textual format.

## 7. MIR Optimization

**Input**: MIR
**Output**: Optimized MIR

Optimization passes transform the MIR in place. The `PassManager` composes
passes into level-specific pipelines (O0 through O3). See
[OPTIMIZATION_SYSTEMS.md](OPTIMIZATION_SYSTEMS.md) for the full pass catalog.

### MIR-level analyses used by passes

| Analysis | Used by | File |
| -------- | ------- | ---- |
| Dominator tree | LICM, GVN, GlobalLoadCaching | `ir/optimization.rs` |
| Loop nest info | LICM, BCE, Vectorization | `ir/loop_analysis.rs` |
| Escape analysis | InsertFree, LICM alloc hoisting | `ir/insert_free.rs` |
| Derived pointer set | InsertFree | `ir/insert_free.rs` |
| Use-def chains | DCE, CopyProp, CSE | `ir/optimization.rs` |

### Free insertion

`InsertFree` deserves special mention because it runs at **all** optimization
levels as a correctness pass. It uses escape analysis to identify heap
allocations that do not escape, then inserts `Free` instructions before
returns. The derived pointer set must track `GEP`, `Cast`, `BitCast`, and
`Copy` chains to correctly detect escapes through function arguments.

## 8. Code Generation

**Input**: Optimized MIR
**Output**: Executable machine code

### Backends

| Backend | File | Tier | Use case |
| ------- | ---- | ---- | -------- |
| MIR Interpreter | `codegen/mir_interpreter.rs` | 0 | Startup, cold code |
| Cranelift JIT | `codegen/cranelift_backend.rs` | 1-2 | Warm/hot code |
| LLVM JIT | `codegen/llvm_jit_backend.rs` | 3 | Peak performance |
| LLVM AOT | `codegen/llvm_aot_backend.rs` | — | Ahead-of-time compilation |

### Tiered execution

The `TieredBackend` (`codegen/tiered_backend.rs`) orchestrates tier promotion:

1. **Tier 0** — MIR interpreter. All functions start here.
2. **Tier 1** — Cranelift baseline (fast compile, basic optimization).
   Promoted after ~10 calls.
3. **Tier 2** — Cranelift optimized (speed + alias analysis). Promoted after
   ~100 calls.
4. **Tier 3** — LLVM (aggressive optimization). Promoted after ~1000 calls.

Hot functions are compiled in a background thread while the interpreter
continues executing.

### Instruction lowering

`codegen/instruction_lowering.rs` handles MIR → Cranelift IR translation,
including:
- **FMA fusion** — `fmul` + `fadd`/`fsub` in the same Cranelift block are
  fused into `fma` instructions (disabled with `RAYZOR_NO_FMA=1`)
- **Intrinsification** — runtime calls like `haxe_array_get_ptr` are replaced
  with inline pointer arithmetic when bounds checks have been eliminated

## 9. Dependency Analysis

**Key file**: `dependency_graph.rs`

For multi-file compilation, the dependency graph detects circular dependencies
between types and modules. It determines compilation order and reports cycles
as errors.

## 10. Validation

**Key file**: `ir/validation.rs`

MIR validation checks SSA invariants:
- Every use has a preceding definition
- Phi node sources match block predecessors
- Terminator targets reference existing blocks
- Type consistency across instructions

Run automatically in debug builds and when `RAYZOR_PASS_DEBUG=1` is set.

## Pipeline Orchestration

**Key file**: `compilation.rs`

`CompilationUnit` is the top-level driver that ties all phases together:

```rust
let mut unit = CompilationUnit::new();
unit.load_stdlib();
unit.add_file("Main.hx");
unit.lower_to_tast()?;          // phases 1-4
let modules = unit.get_mir_modules()?;  // phases 5-6
// phases 7-8 happen in PassManager + backend
```

Multi-file projects add all files before calling `lower_to_tast()`. Stdlib
modules are loaded once and shared across compilations. External plugins
(rpkg packages) register their method mappings and runtime symbols before
compilation begins.
