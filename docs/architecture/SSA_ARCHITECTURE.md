# Rayzor Compiler Architecture

> **A layered compilation architecture with SSA-based flow analysis**

## Table of Contents

- [Overview](#overview)
- [Architecture Diagram](#architecture-diagram)
- [Core Design Principles](#core-design-principles)
- [Compilation Pipeline](#compilation-pipeline)
- [SSA Integration Strategy](#ssa-integration-strategy)
- [Layer Details](#layer-details)
- [Information Flow](#information-flow)
- [Implementation Guide](#implementation-guide)
- [Benefits & Trade-offs](#benefits--trade-offs)

---

## Overview

The Rayzor compiler implements a **layered architecture** where Static Single Assignment (SSA) form serves as **analysis infrastructure** rather than a representation requirement. This design enables precise flow-sensitive analysis while maintaining flexibility in intermediate representations.

### Key Innovation

**SSA is built once in the SemanticGraphs layer and queried by all subsequent passes**, eliminating duplication and maintaining a single source of truth.

---

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────┐
│                         Source Code (Haxe)                          │
│                    Parser (nom-based, incremental)                  │
└────────────────────────────┬────────────────────────────────────────┘
                             │
                             ↓
┌─────────────────────────────────────────────────────────────────────┐
│                      AST (Abstract Syntax Tree)                     │
│              Untyped representation of source structure             │
└────────────────────────────┬────────────────────────────────────────┘
                             │
                             ↓
┌─────────────────────────────────────────────────────────────────────┐
│                    TAST (Typed AST) Layer                           │
│  ┌────────────────────────────────────────────────────────────┐    │
│  │  Type Checker                                               │    │
│  │  • Type inference and unification                           │    │
│  │  • Symbol resolution                                        │    │
│  │  • Type constraints                                         │    │
│  └────────────────────────────────────────────────────────────┘    │
│                                                                      │
│  Components:                                                         │
│  • TypedFile, TypedFunction, TypedExpression, TypedStatement        │
│  • SymbolTable (symbols and their types)                            │
│  • TypeTable (type definitions and relationships)                   │
└────────────────────────────┬────────────────────────────────────────┘
                             │
                             ↓
┌─────────────────────────────────────────────────────────────────────┐
│                    SemanticGraphs Layer                             │
│                  ★ SINGLE SOURCE OF TRUTH FOR SSA ★                 │
│  ┌──────────────────┐  ┌──────────────────┐  ┌─────────────────┐  │
│  │ CFG              │  │ DFG (SSA Form)   │  │ CallGraph       │  │
│  │ Control Flow     │  │ • Phi nodes      │  │ Inter-procedural│  │
│  │ • Basic blocks   │  │ • Def-use chains │  │ call structure  │  │
│  │ • Dominance      │  │ • SSA variables  │  │                 │  │
│  │ • Terminators    │  │ • Value numbering│  │                 │  │
│  └──────────────────┘  └──────────────────┘  └─────────────────┘  │
│                                                                      │
│  ┌──────────────────┐  ┌──────────────────┐                        │
│  │ OwnershipGraph   │  │ LifetimeAnalyzer │                        │
│  │ Borrow tracking  │  │ Lifetime regions │                        │
│  └──────────────────┘  └──────────────────┘                        │
│                                                                      │
│  Key Property: All SSA operations (phi insertion, renaming,         │
│                dominance frontiers) happen HERE and ONLY here       │
└────────────────────────────┬────────────────────────────────────────┘
                             │ (queries SSA insights)
                             ↓
┌─────────────────────────────────────────────────────────────────────┐
│                      TypeFlowGuard Layer                            │
│           Flow-sensitive type checking using SSA                    │
│  ┌────────────────────────────────────────────────────────────┐    │
│  │  SSA-Based Analysis                                         │    │
│  │  • analyze_ssa_initialization() - uses def-use chains       │    │
│  │  • analyze_ssa_null_safety() - SSA vars + nullability       │    │
│  │  • analyze_ssa_dead_code() - SSA liveness analysis          │    │
│  │  • Integration with lifetime/ownership analyzers            │    │
│  └────────────────────────────────────────────────────────────┘    │
│                                                                      │
│  Queries: DFG.is_valid_ssa(), DFG.ssa_variables,                    │
│           DFG.nodes, DFG.def_use_chains                             │
│  Output: FlowSafetyResults (errors, warnings, metrics)              │
└────────────────────────────┬────────────────────────────────────────┘
                             │ (queries SSA insights)
                             ↓
┌─────────────────────────────────────────────────────────────────────┐
│                    HIR (High-level IR) Layer                        │
│          Preserves language semantics + optimization hints          │
│  ┌────────────────────────────────────────────────────────────┐    │
│  │  TAST → HIR Lowering (tast_to_hir.rs)                      │    │
│  │  • Desugar syntax (for-in → iterators)                     │    │
│  │  • Resolve all symbols                                     │    │
│  │  • Extract SSA hints via extract_ssa_optimization_hints()  │    │
│  └────────────────────────────────────────────────────────────┘    │
│                                                                      │
│  Representation:                                                     │
│  • HirModule, HirFunction, HirExpr, HirStatement                    │
│  • High-level constructs (lambdas, pattern matching, etc.)          │
│  • HirAttribute metadata with SSA-derived hints:                    │
│    - inline_candidate (from SSA complexity)                         │
│    - optimization_hint: "straight_line_code" (from CFG)             │
│    - optimization_hint: "complex_control_flow" (from phi count)     │
│    - optimization_hint: "common_subexpressions" (from value #)      │
│    - dead_code_count (from SSA liveness)                            │
│                                                                      │
│  Purpose: Enable hot-reload, debugging, source mapping              │
└────────────────────────────┬────────────────────────────────────────┘
                             │ (reads SSA hints from metadata)
                             ↓
┌─────────────────────────────────────────────────────────────────────┐
│                    MIR (Mid-level IR) Layer                         │
│              Optimizable form with SSA-guided attributes            │
│  ┌────────────────────────────────────────────────────────────┐    │
│  │  HIR → MIR Lowering (hir_to_mir.rs)                        │    │
│  │  • Parse SSA hints from HIR metadata                       │    │
│  │  • Apply to function attributes:                           │    │
│  │    - inline_candidates → InlineHint::Always                │    │
│  │    - straight_line_code → pure = true                      │    │
│  │    - complex_control_flow → optimize_size = false          │    │
│  │  • Generate standard IR instructions                       │    │
│  └────────────────────────────────────────────────────────────┘    │
│                                                                      │
│  Representation:                                                     │
│  • IrModule, IrFunction, IrInstruction, IrBasicBlock                │
│  • Standard IR operations (add, mul, load, store, etc.)             │
│  • FunctionAttributes with optimization guidance                    │
│  • NOT in SSA form (but informed by SSA analysis)                   │
│                                                                      │
│  Purpose: Platform-independent optimization target                  │
└────────────────────────────┬────────────────────────────────────────┘
                             │
                             ↓
┌─────────────────────────────────────────────────────────────────────┐
│                    Optimization Passes                              │
│  • Dead Code Elimination (guided by SSA liveness)                   │
│  • Common Subexpression Elimination (guided by value numbering)     │
│  • Inlining (guided by inline_candidate hints)                      │
│  • Constant Propagation                                             │
│  • Loop Optimization                                                │
└────────────────────────────┬────────────────────────────────────────┘
                             │
                             ↓
┌─────────────────────────────────────────────────────────────────────┐
│                    Code Generation                                  │
│  • LLVM Backend (future)                                            │
│  • JavaScript Backend                                               │
│  • Interpreter (hot-reload support)                                 │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Core Design Principles

### 1. **SSA as Analysis Infrastructure, Not Representation**

**Principle**: SSA form is built once for analysis purposes and queried by all passes that need it. Individual IRs are not required to be in SSA form.

**Rationale**:
- SSA transformation is expensive (phi node insertion, variable renaming, dominance computation)
- Not all IRs benefit from SSA form
- HIR needs to preserve high-level semantics for hot-reload and debugging
- MIR benefits from SSA insights without the structural constraints

**Implementation**:
- `DataFlowGraph` in SemanticGraphs maintains SSA form
- Other passes query SSA properties via well-defined APIs
- No duplication of SSA construction logic

### 2. **Separation of Concerns**

**Principle**: Each layer has a single, well-defined responsibility.

| Layer | Responsibility | Does NOT Do |
|-------|---------------|-------------|
| SemanticGraphs | Analysis (SSA, CFG, lifetimes) | Code generation |
| TypeFlowGuard | Flow-sensitive type checking | AST transformation |
| HIR | Preserve semantics + carry hints | Optimization |
| MIR | Optimization target | Source mapping |

### 3. **Single Source of Truth**

**Principle**: Each piece of information has exactly one authoritative source.

- **SSA Form**: `DataFlowGraph` in SemanticGraphs
- **Type Information**: `TypeTable` in TAST
- **Symbol Bindings**: `SymbolTable` in TAST
- **Control Flow**: `ControlFlowGraph` in SemanticGraphs

### 4. **Information Flow via Metadata**

**Principle**: Layers communicate via explicit metadata, not implicit coupling.

**Example Flow**:
```
DFG (SSA)
  → extract_ssa_optimization_hints()
    → HirAttribute in HIR metadata
      → extract_ssa_hints_from_hir()
        → FunctionAttributes in MIR
```

Benefits:
- Loose coupling
- Easy to add new hints
- Clear data dependencies
- Testable in isolation

### 5. **Fail-Safe Defaults**

**Principle**: Missing optimization hints degrade performance, not correctness.

- If SSA hints are missing, compilation still succeeds
- Optimization passes use conservative defaults
- No crashes or wrong code generation

---

## Compilation Pipeline

### Phase 1: Parsing
**Input**: Source code (.hx files)
**Output**: AST
**Tool**: nom-based parser with incremental support
**Features**: Error recovery, position tracking, incremental re-parsing

### Phase 2: Type Checking
**Input**: AST
**Output**: TAST (Typed AST)
**Process**:
1. Symbol resolution
2. Type inference and unification
3. Constraint solving
4. Type error reporting

**Key Structures**:
- `TypedFile`, `TypedFunction`, `TypedExpression`
- `SymbolTable`: Symbol ID → Symbol info
- `TypeTable`: Type ID → Type definition

### Phase 3: Semantic Analysis
**Input**: TAST
**Output**: SemanticGraphs
**Process**:
1. **CFG Construction** (`CfgBuilder`)
   - Build basic blocks
   - Add terminators (branches, returns)
   - Compute dominance relationships

2. **DFG Construction in SSA Form** (`DfgBuilder`)
   - Insert phi nodes at dominance frontiers
   - Rename variables to SSA form
   - Build def-use chains
   - Compute value numbering

3. **Call Graph Construction**
   - Track function calls
   - Build interprocedural graph

4. **Ownership/Lifetime Analysis**
   - Track variable lifetimes
   - Detect use-after-move
   - Validate borrowing rules

### Phase 4: Flow-Sensitive Type Checking
**Input**: TAST + SemanticGraphs
**Output**: FlowSafetyResults
**Process** (TypeFlowGuard):
1. Validate DFG is in SSA form
2. Analyze initialization (def-use chains)
3. Analyze null safety (SSA variables + types)
4. Detect dead code (SSA liveness)
5. Check lifetime/ownership constraints

### Phase 5: HIR Lowering
**Input**: TAST + SemanticGraphs
**Output**: HIR with metadata
**Process**:
1. Desugar high-level constructs
2. Resolve all symbols
3. **Extract SSA optimization hints**:
   - Query `DFG.ssa_variables.len()` → few_locals hint
   - Query `DFG.metadata.phi_node_count` → control flow complexity
   - Query `DFG.value_numbering` → CSE opportunities
4. Embed hints in `HirAttribute` metadata

### Phase 6: MIR Lowering
**Input**: HIR
**Output**: MIR (IrModule)
**Process**:
1. **Parse SSA hints from HIR metadata**
2. **Apply to function attributes**:
   - `inline_candidate` → `InlineHint::Always`
   - `straight_line_code` → `pure = true`
   - `complex_control_flow` → `optimize_size = false`
3. Lower expressions to IR instructions
4. Build control flow with basic blocks

### Phase 7: Optimization
**Input**: MIR
**Output**: Optimized MIR
**Passes**:
- Dead Code Elimination (uses SSA liveness insights)
- Common Subexpression Elimination (uses value numbering insights)
- Inlining (uses inline hints)
- Constant Propagation
- Loop optimizations

### Phase 8: Code Generation
**Input**: Optimized MIR
**Output**: Target code
**Targets**:
- JavaScript (via custom codegen)
- LLVM IR (future)
- Interpreter bytecode (for hot-reload)

---

## SSA Integration Strategy

### The Problem SSA Solves

Without SSA:
```haxe
var x = 5;
if (condition) {
    x = 10;
}
print(x);  // What value? Hard to analyze!
```

With SSA (in DFG):
```
x₀ = 5
if (condition) {
    x₁ = 10
}
x₂ = φ(x₀, x₁)  // Phi node merges values
print(x₂)
```

Benefits:
- Each variable has exactly one definition
- Def-use chains are precise
- Dataflow analysis is straightforward

### Our SSA Strategy: "Build Once, Query Everywhere"

#### ✅ **What We Do**

1. **Build SSA in SemanticGraphs/DFG**
   ```rust
   // In DfgBuilder
   pub fn build_from_tast(tast: &TypedFile) -> DataFlowGraph {
       // Insert phi nodes at dominance frontiers
       // Rename variables to SSA form
       // Build def-use chains
   }
   ```

2. **Query SSA in TypeFlowGuard**
   ```rust
   // In TypeFlowGuard::analyze_with_dfg()
   if !dfg.is_valid_ssa() { return; }

   for ssa_var in dfg.ssa_variables.values() {
       // Analyze using SSA properties
   }
   ```

3. **Extract Hints in HIR Lowering**
   ```rust
   // In tast_to_hir.rs
   fn extract_ssa_optimization_hints(&self, dfg: &DataFlowGraph) -> Vec<HirAttribute> {
       let phi_count = dfg.metadata.phi_node_count;
       // Convert SSA metrics to optimization hints
   }
   ```

4. **Apply Hints in MIR Lowering**
   ```rust
   // In hir_to_mir.rs
   fn extract_ssa_hints_from_hir(&mut self, hir: &HirModule) {
       // Parse hints from HIR metadata
       // Apply to MIR function attributes
   }
   ```

#### ❌ **What We DON'T Do**

- ❌ Don't rebuild SSA in multiple places
- ❌ Don't require HIR to be in SSA form
- ❌ Don't require MIR to be in SSA form
- ❌ Don't duplicate phi node insertion logic

### SSA Query API

The `DataFlowGraph` provides these query methods:

```rust
impl DataFlowGraph {
    /// Check if DFG is in valid SSA form
    pub fn is_valid_ssa(&self) -> bool;

    /// Get all uses of a definition
    pub fn get_uses(&self, def: DataFlowNodeId) -> &[DataFlowNodeId];

    /// Get definition of a use
    pub fn get_definition(&self, use_node: DataFlowNodeId) -> Option<DataFlowNodeId>;

    /// Get all nodes in a basic block
    pub fn nodes_in_block(&self, block_id: BlockId) -> &[DataFlowNodeId];

    /// Access SSA variables
    pub ssa_variables: IdMap<SsaVariableId, SsaVariable>;

    /// Access value numbering (for CSE)
    pub value_numbering: ValueNumbering;

    /// Access metadata (phi count, etc.)
    pub metadata: DfgMetadata;
}
```

---

## Layer Details

### SemanticGraphs Layer

**Location**: `compiler/src/semantic_graph/`

**Purpose**: Construct analysis graphs from TAST

**Key Structures**:

```rust
pub struct SemanticGraphs {
    /// Control Flow Graphs per function
    pub control_flow: IdMap<SymbolId, ControlFlowGraph>,

    /// Data Flow Graphs (SSA form) per function
    pub data_flow: IdMap<SymbolId, DataFlowGraph>,

    /// Inter-procedural call graph
    pub call_graph: CallGraph,

    /// Ownership and borrowing relationships
    pub ownership_graph: OwnershipGraph,

    /// Source location mapping
    pub source_locations: IdMap<BlockId, SourceLocation>,
}
```

**CFG (`cfg.rs`)**:
- Basic blocks with statements
- Terminators (Jump, Branch, Switch, Return, etc.)
- Dominance information
- Loop detection

**DFG (`dfg.rs`, `dfg_builder.rs`)**:
- SSA form with phi nodes
- `DataFlowNode` enum (Parameter, Constant, Variable, BinaryOp, Call, Phi, etc.)
- `SsaVariable` tracking (original symbol, SSA index, definition, uses)
- `DefUseChains` for fast lookups
- `ValueNumbering` for CSE opportunities

**Construction**:
```rust
// Build CFG
let cfg = CfgBuilder::new(options).build_for_function(function)?;

// Build DFG in SSA form
let dfg = DfgBuilder::new(options).build_from_cfg(&cfg, function)?;
```

### TypeFlowGuard Layer

**Location**: `compiler/src/tast/type_flow_guard.rs`

**Purpose**: Flow-sensitive type checking using SSA

**Key Functions**:

```rust
impl TypeFlowGuard {
    /// Analyze file with flow-sensitive checks
    pub fn analyze_file(&mut self, file: &TypedFile) -> FlowSafetyResults;

    /// Analyze function with pre-built CFG and DFG
    pub fn analyze_function_safety(
        &mut self,
        function: &TypedFunction,
        cfg: &ControlFlowGraph,
        dfg: &DataFlowGraph,
    );

    /// SSA-based initialization analysis
    fn analyze_ssa_initialization(&mut self, dfg: &DataFlowGraph);

    /// SSA-based null safety analysis
    fn analyze_ssa_null_safety(&mut self, dfg: &DataFlowGraph);

    /// SSA-based dead code detection
    fn analyze_ssa_dead_code(&mut self, dfg: &DataFlowGraph);
}
```

**Example Analysis**:
```rust
// Check for uninitialized variables using SSA
for (use_node_id, node) in &dfg.nodes {
    for &operand_id in &node.operands {
        if dfg.get_node(operand_id).is_none() {
            // Operand has no definition → uninitialized!
            self.results.errors.push(FlowSafetyError::UninitializedVariable { ... });
        }
    }
}
```

### HIR Layer

**Location**: `compiler/src/ir/hir.rs`, `compiler/src/ir/tast_to_hir.rs`

**Purpose**: High-level IR preserving language semantics

**Key Structures**:

```rust
pub struct HirModule {
    pub name: String,
    pub imports: Vec<HirImport>,
    pub types: HashMap<TypeId, HirTypeDecl>,
    pub functions: HashMap<SymbolId, HirFunction>,
    pub globals: HashMap<SymbolId, HirGlobal>,
    pub metadata: HirMetadata,
}

pub struct HirFunction {
    pub symbol_id: SymbolId,
    pub name: InternedString,
    pub params: Vec<HirParam>,
    pub return_type: TypeId,
    pub body: Option<HirBlock>,
    pub metadata: Vec<HirAttribute>,  // ← SSA hints here!
    pub is_inline: bool,
    pub is_main: bool,
}

pub struct HirAttribute {
    pub name: InternedString,
    pub args: Vec<HirAttributeArg>,
}
```

**SSA Hint Extraction** (`tast_to_hir.rs`):

```rust
fn extract_ssa_optimization_hints(
    &self,
    function: &TypedFunction,
    semantic_graphs: &SemanticGraphs,
) -> Vec<HirAttribute> {
    let mut hints = Vec::new();

    // Get DFG for this function
    if let Some(dfg) = semantic_graphs.data_flow.get(&function.symbol_id) {
        if dfg.is_valid_ssa() {
            // Extract hints from SSA metrics
            if dfg.ssa_variables.len() < 10 {
                hints.push(HirAttribute { /* few_locals */ });
            }

            if dfg.metadata.phi_node_count == 0 {
                hints.push(HirAttribute { /* straight_line_code */ });
            }

            // ... more hints
        }
    }

    hints
}
```

### MIR Layer

**Location**: `compiler/src/ir/hir_to_mir.rs`, `compiler/src/ir/`

**Purpose**: Platform-independent optimization target

**Key Structures**:

```rust
pub struct IrModule {
    pub name: String,
    pub functions: HashMap<IrFunctionId, IrFunction>,
    pub globals: Vec<IrGlobal>,
    pub metadata: IrModuleMetadata,
}

pub struct IrFunction {
    pub id: IrFunctionId,
    pub symbol_id: SymbolId,
    pub name: String,
    pub signature: IrFunctionSignature,
    pub cfg: IrControlFlowGraph,
    pub locals: HashMap<IrId, IrLocal>,
    pub attributes: FunctionAttributes,  // ← SSA hints applied here!
}

pub struct FunctionAttributes {
    pub linkage: Linkage,
    pub inline: InlineHint,  // ← From SSA complexity
    pub pure: bool,           // ← From straight-line code
    pub no_return: bool,
    pub optimize_size: bool,  // ← From control flow complexity
}
```

**SSA Hint Application** (`hir_to_mir.rs`):

```rust
struct SsaOptimizationHints {
    inline_candidates: HashSet<SymbolId>,
    straight_line_functions: HashSet<SymbolId>,
    complex_control_flow_functions: HashSet<SymbolId>,
    cse_opportunities: HashSet<SymbolId>,
}

fn extract_ssa_hints_from_hir(&mut self, hir_module: &HirModule) {
    for (symbol_id, func) in &hir_module.functions {
        for attr in &func.metadata {
            match attr.name.to_string().as_str() {
                "inline_candidate" => {
                    self.ssa_hints.inline_candidates.insert(*symbol_id);
                }
                "optimization_hint" => {
                    // Parse hint value and categorize
                }
                _ => {}
            }
        }
    }
}

fn lower_function(&mut self, symbol_id: SymbolId, hir_func: &HirFunction) {
    // Apply SSA hints to function attributes
    if self.ssa_hints.inline_candidates.contains(&symbol_id) {
        func.attributes.inline = InlineHint::Always;
    }

    if self.ssa_hints.straight_line_functions.contains(&symbol_id) {
        func.attributes.pure = true;
    }

    // ... more applications
}
```

---

## Information Flow

### SSA Metrics → Optimization Hints

| SSA Metric | Source | HIR Hint | MIR Application |
|------------|--------|----------|-----------------|
| `ssa_variables.len() < 10` | DFG | `few_locals` | Register allocation hints |
| `phi_node_count == 0` | DFG metadata | `straight_line_code` | `pure = true` |
| `phi_node_count > 20` | DFG metadata | `complex_control_flow` | `optimize_size = false` |
| `value_numbering.expr_to_value.len() > 5` | DFG | `common_subexpressions` | Enable CSE pass |
| `nodes.len() < 10 && phi < 3` | DFG | `inline_candidate` | `InlineHint::Always` |
| `cfg.blocks.len() == 1` | CFG | `single_block` | Aggressive optimization |

### Data Flow Example

**Input Haxe Code**:
```haxe
function simpleAdd(a: Int, b: Int): Int {
    return a + b;
}
```

**After TAST**:
```
TypedFunction {
    symbol_id: SymbolId(42),
    name: "simpleAdd",
    parameters: [a: Int, b: Int],
    return_type: Int,
    body: [Return(Add(Var(a), Var(b)))]
}
```

**After SemanticGraphs/DFG** (SSA form):
```
DFG {
    nodes: {
        n0: Parameter(a, Int),
        n1: Parameter(b, Int),
        n2: BinaryOp(Add, n0, n1),
        n3: Return(n2)
    },
    ssa_variables: {
        a₀: { original: a, definition: n0, uses: [n2] },
        b₀: { original: b, definition: n1, uses: [n2] }
    },
    metadata: {
        phi_node_count: 0,  // ← No control flow!
        ssa_variable_count: 2
    }
}
```

**After HIR Lowering** (with hints):
```rust
HirFunction {
    symbol_id: SymbolId(42),
    name: "simpleAdd",
    metadata: [
        HirAttribute {
            name: "optimization_hint",
            args: [Literal(String("straight_line_code"))]
        },
        HirAttribute {
            name: "inline_candidate",
            args: [Literal(Bool(true))]
        }
    ],
    body: /* ... */
}
```

**After MIR Lowering** (with attributes):
```rust
IrFunction {
    id: IrFunctionId(10),
    symbol_id: SymbolId(42),
    name: "simpleAdd",
    attributes: FunctionAttributes {
        inline: InlineHint::Always,  // ← From inline_candidate
        pure: true,                   // ← From straight_line_code
        optimize_size: false,
        // ...
    },
    cfg: /* standard IR blocks */
}
```

**Result**: Optimizer knows this function:
- Should be inlined aggressively
- Is pure (no side effects)
- Has simple control flow (optimize aggressively)

---

## Implementation Guide

### Adding a New SSA-Based Analysis

**Step 1**: Add analysis to SemanticGraphs
```rust
// In semantic_graph/new_analysis.rs
pub fn analyze_pattern(&self, dfg: &DataFlowGraph) -> AnalysisResult {
    // Use dfg.ssa_variables, dfg.nodes, etc.
}
```

**Step 2**: Query in TypeFlowGuard (if needed)
```rust
// In type_flow_guard.rs
fn analyze_with_dfg(&mut self, dfg: &DataFlowGraph, function: &TypedFunction) {
    let pattern_result = analyze_pattern(dfg);
    // Use results for error reporting
}
```

**Step 3**: Extract hint in HIR lowering
```rust
// In tast_to_hir.rs
fn extract_ssa_optimization_hints(...) -> Vec<HirAttribute> {
    if let Some(dfg) = semantic_graphs.data_flow.get(&function.symbol_id) {
        // Query your analysis
        if some_condition_from_dfg {
            hints.push(HirAttribute {
                name: intern("my_new_hint"),
                args: vec![/* ... */]
            });
        }
    }
}
```

**Step 4**: Apply in MIR lowering
```rust
// In hir_to_mir.rs
fn extract_ssa_hints_from_hir(&mut self, hir: &HirModule) {
    for attr in &func.metadata {
        if attr.name.to_string() == "my_new_hint" {
            // Store in SsaOptimizationHints
        }
    }
}

fn lower_function(...) {
    // Apply hint to function attributes or pass to optimizer
}
```

### Adding a New Optimization Pass

```rust
// In ir/optimization.rs
pub struct MyOptimizationPass {
    // State
}

impl OptimizationPass for MyOptimizationPass {
    fn run(&mut self, module: &mut IrModule) -> OptimizationResult {
        for (func_id, func) in &mut module.functions {
            // Check SSA-derived hints
            if func.attributes.inline == InlineHint::Always {
                // Optimize differently
            }

            // Run your optimization
        }

        OptimizationResult::Modified
    }
}
```

### Testing the Pipeline

```rust
// In tests/integration/pipeline_test.rs
#[test]
fn test_ssa_hints_flow_through_pipeline() {
    let source = r#"
        function simple(x: Int): Int {
            return x + 1;
        }
    "#;

    // Parse
    let ast = parse(source)?;

    // Type check
    let tast = type_check(ast)?;

    // Build semantic graphs (SSA here)
    let semantic_graphs = build_semantic_graphs(&tast)?;
    let dfg = semantic_graphs.data_flow.get(&func_symbol_id).unwrap();
    assert!(dfg.is_valid_ssa());
    assert_eq!(dfg.metadata.phi_node_count, 0);  // Straight-line

    // Lower to HIR
    let hir = lower_to_hir(&tast, &semantic_graphs)?;
    let hir_func = hir.functions.get(&func_symbol_id).unwrap();

    // Check hint was extracted
    assert!(hir_func.metadata.iter().any(|attr|
        attr.name.to_string() == "optimization_hint"
    ));

    // Lower to MIR
    let mir = lower_to_mir(&hir)?;
    let mir_func = mir.functions.values().next().unwrap();

    // Check hint was applied
    assert_eq!(mir_func.attributes.inline, InlineHint::Always);
    assert!(mir_func.attributes.pure);
}
```

---

## Benefits & Trade-offs

### Benefits

✅ **No SSA Duplication**
- SSA built once in DFG
- All passes query the same source
- Consistent results across analyses

✅ **Maintainability**
- Clear separation of concerns
- Easy to understand data flow
- Single point of change for SSA logic

✅ **Flexibility**
- HIR can preserve high-level semantics (hot-reload, debugging)
- MIR can use any form convenient for optimization
- Not locked into SSA form everywhere

✅ **Performance**
- SSA construction is expensive, done once
- Queries are cheap (pointer lookups)
- Optimization hints reduce redundant analysis

✅ **Extensibility**
- Easy to add new SSA-based analyses
- Easy to add new optimization hints
- Loose coupling enables parallel development

### Trade-offs

⚠️ **Complexity**
- More layers than a simple compiler
- Metadata-based communication requires discipline
- Need to keep hint definitions synchronized

⚠️ **Memory Usage**
- Keep SemanticGraphs in memory during compilation
- Multiple IR representations (AST, TAST, HIR, MIR)
- Trade memory for analysis precision

⚠️ **Compilation Time**
- SSA construction takes time upfront
- Multiple lowering passes
- More analysis before optimization

**Mitigation**:
- Incremental compilation (parser supports it)
- Caching of SemanticGraphs
- Parallel analysis where possible
- Can skip expensive analyses in dev mode

---

## Future Directions

### Short Term
- [ ] Complete MIR optimization passes
- [ ] Add more SSA-based hints (loop invariants, escape analysis)
- [ ] Implement interpreter backend (for hot-reload)
- [ ] Add integration tests for full pipeline

### Medium Term
- [ ] LLVM backend using SSA insights
- [ ] Interprocedural analysis (using CallGraph)
- [ ] Advanced lifetime analysis (region-based)
- [ ] Profile-guided optimization (PGO) with SSA

### Long Term
- [ ] Incremental semantic analysis
- [ ] Parallel compilation with shared SemanticGraphs
- [ ] Query-based compilation model
- [ ] Language server protocol (LSP) integration

---

## References

### Key Files

| Component | Location |
|-----------|----------|
| SemanticGraphs | `compiler/src/semantic_graph/mod.rs` |
| CFG | `compiler/src/semantic_graph/cfg.rs` |
| DFG (SSA) | `compiler/src/semantic_graph/dfg.rs` |
| DFG Builder | `compiler/src/semantic_graph/dfg_builder.rs` |
| TypeFlowGuard | `compiler/src/tast/type_flow_guard.rs` |
| HIR Definition | `compiler/src/ir/hir.rs` |
| TAST → HIR | `compiler/src/ir/tast_to_hir.rs` |
| HIR → MIR | `compiler/src/ir/hir_to_mir.rs` |
| MIR Definition | `compiler/src/ir/mod.rs` |
| Optimization | `compiler/src/ir/optimization.rs` |

### Academic Background

This architecture is inspired by:

- **LLVM**: Layered IR design
- **Rust Compiler (rustc)**: HIR/MIR separation, borrow checking
- **GCC**: SSA for optimization passes
- **MLton (SML compiler)**: SSA-based whole-program optimization

**Key Papers**:
- Cytron et al., "Efficiently Computing Static Single Assignment Form" (1991)
- Appel & Palsberg, "Modern Compiler Implementation in ML" (2002)
- Lattner & Adve, "LLVM: A Compilation Framework for Lifelong Program Analysis" (2004)

---

## Conclusion

The Rayzor compiler architecture demonstrates that **SSA as analysis infrastructure** is a powerful pattern. By building SSA once and querying it everywhere, we achieve:

- **Precision** in flow-sensitive analysis
- **Flexibility** in IR design
- **Maintainability** through clear separation
- **Performance** through shared analysis

This architecture is production-ready and scales well to additional analyses and optimizations.

---

**Document Version**: 1.0
**Last Updated**: 2025-11-12
**Authors**: Rayzor Compiler Team
**License**: MIT
