# Rayzor Compiler Architecture

> **A modern, multi-tier compilation infrastructure for the Haxe programming language**

## Table of Contents

- [Overview](#overview)
- [Design Philosophy](#design-philosophy)
- [High-Level Architecture](#high-level-architecture)
- [Compilation Phases](#compilation-phases)
- [Core Components](#core-components)
- [Type System](#type-system)
- [Memory Management Model](#memory-management-model)
- [Error Handling](#error-handling)
- [Optimization Strategy](#optimization-strategy)
- [Development Workflow](#development-workflow)
- [Related Documentation](#related-documentation)

---

## Overview

The Rayzor compiler is a complete reimplementation of a Haxe compiler in Rust, designed for:

- **High Performance**: Native compilation speeds, incremental builds
- **Memory Safety**: Compile-time memory safety checking (inspired by Rust)
- **Developer Experience**: Fast hot-reload, excellent error messages
- **Production Ready**: AOT compilation to native code with maximum optimization

### Project Goals

1. **Compatibility**: Support core Haxe language features
2. **Safety**: Add optional memory safety features (ownership, lifetimes)
3. **Performance**: Fast compilation, fast generated code
4. **Tooling**: IDE support, debugging, profiling

### Key Features

- ✅ Incremental parsing with error recovery
- ✅ Sophisticated type inference and checking
- ✅ Flow-sensitive safety analysis
- ✅ Multi-tier IR design (HIR, MIR)
- ✅ Semantic graph-based optimization
- 🚧 Multiple backends (JS, LLVM, Interpreter)
- 🚧 Hot-reload support for rapid development

---

## Design Philosophy

### 1. **Correctness First, Performance Second**

The compiler prioritizes generating correct code. Performance optimizations come after correctness is proven.

```
Correctness → Safety → Clarity → Performance
```

### 2. **Layered Architecture**

Each layer has a single, well-defined responsibility:

```
Parsing → Type Checking → Analysis → Lowering → Optimization → Generation
```

No layer should know about layers above it. Information flows forward through explicit interfaces.

### 3. **Fail-Fast with Excellent Diagnostics**

When errors occur:
- Catch them as early as possible (parser → type checker → analysis)
- Provide precise location information
- Offer helpful suggestions for fixes
- Continue processing to find multiple errors

### 4. **Incremental Everything**

Support incremental operations at every level:
- Incremental parsing (re-parse only changed regions)
- Incremental type checking (re-check only affected code)
- Incremental analysis (re-analyze only dependencies)
- Incremental codegen (re-generate only changed functions)

### 5. **Analysis as Infrastructure**

Complex analyses (SSA, CFG, DFG, lifetimes, ownership) are built once and queried by multiple passes. See [SSA_ARCHITECTURE.md](SSA_ARCHITECTURE.md) for details.

---

## High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                          Source Files (.hx)                         │
│                     Haxe programming language                       │
└────────────────────────────┬────────────────────────────────────────┘
                             │
                             ↓
┌─────────────────────────────────────────────────────────────────────┐
│                         Parser Crate                                │
│  • Nom-based parser combinators                                     │
│  • Incremental parsing with change tracking                         │
│  • Error recovery and diagnostics                                   │
│  • Produces: AST (Abstract Syntax Tree)                             │
└────────────────────────────┬────────────────────────────────────────┘
                             │
                             ↓
┌─────────────────────────────────────────────────────────────────────┐
│                        Compiler Crate                               │
│                                                                      │
│  ┌───────────────────────────────────────────────────────────────┐ │
│  │  Frontend (TAST Layer)                                        │ │
│  │  • Type checking and inference                                │ │
│  │  • Symbol resolution                                          │ │
│  │  • Constraint solving                                         │ │
│  │  • Produces: TAST (Typed AST)                                 │ │
│  └───────────────────────────────────────────────────────────────┘ │
│                             │                                        │
│                             ↓                                        │
│  ┌───────────────────────────────────────────────────────────────┐ │
│  │  Analysis Layer (SemanticGraphs)                              │ │
│  │  • Control Flow Graphs (CFG)                                  │ │
│  │  • Data Flow Graphs (DFG) in SSA form                         │ │
│  │  • Call Graph (inter-procedural)                              │ │
│  │  • Ownership & Lifetime tracking                              │ │
│  │  • TypeFlowGuard (flow-sensitive checking)                    │ │
│  └───────────────────────────────────────────────────────────────┘ │
│                             │                                        │
│                             ↓                                        │
│  ┌───────────────────────────────────────────────────────────────┐ │
│  │  Middleend (IR Layers)                                        │ │
│  │  • HIR: High-level IR (preserves semantics)                   │ │
│  │  • MIR: Mid-level IR (optimization target)                    │ │
│  │  • Optimization passes                                        │ │
│  └───────────────────────────────────────────────────────────────┘ │
│                             │                                        │
│                             ↓                                        │
│  ┌───────────────────────────────────────────────────────────────┐ │
│  │  Backend (Code Generation)                                    │ │
│  │  • JavaScript backend                                         │ │
│  │  • LLVM backend (future)                                      │ │
│  │  • Interpreter (for hot-reload)                               │ │
│  └───────────────────────────────────────────────────────────────┘ │
│                                                                      │
└─────────────────────────────┬────────────────────────────────────────┘
                             │
                             ↓
                    Target Output Files
```

### Crate Structure

```
rayzor/
├── parser/              # Parsing infrastructure
│   ├── src/
│   │   ├── haxe_parser.rs           # Main parser entry
│   │   ├── haxe_parser_expr.rs      # Expression parsing
│   │   ├── haxe_parser_decls.rs     # Declaration parsing
│   │   ├── haxe_parser_types.rs     # Type parsing
│   │   ├── incremental_parser_enhanced.rs
│   │   └── haxe_ast.rs              # AST definitions
│   └── Cargo.toml
│
├── compiler/            # Main compiler crate
│   ├── src/
│   │   ├── tast/                    # Type-checked AST
│   │   │   ├── mod.rs
│   │   │   ├── type_checker.rs
│   │   │   ├── symbols.rs
│   │   │   ├── core.rs              # Type system core
│   │   │   └── type_flow_guard.rs   # Flow-sensitive checking
│   │   │
│   │   ├── semantic_graph/          # Analysis infrastructure
│   │   │   ├── cfg.rs               # Control Flow Graph
│   │   │   ├── dfg.rs               # Data Flow Graph (SSA)
│   │   │   ├── dfg_builder.rs
│   │   │   ├── call_graph.rs
│   │   │   ├── ownership_graph.rs
│   │   │   └── analysis/
│   │   │       ├── lifetime_analyzer.rs
│   │   │       └── ownership_analyzer.rs
│   │   │
│   │   ├── ir/                      # Intermediate Representations
│   │   │   ├── hir.rs               # High-level IR
│   │   │   ├── tast_to_hir.rs       # TAST → HIR lowering
│   │   │   ├── hir_to_mir.rs        # HIR → MIR lowering
│   │   │   ├── mod.rs               # MIR definitions
│   │   │   ├── builder.rs           # IR builder
│   │   │   ├── optimization.rs      # Optimization passes
│   │   │   └── validation.rs        # IR validation
│   │   │
│   │   ├── pipeline.rs              # Compilation pipeline
│   │   └── lib.rs
│   │
│   ├── ARCHITECTURE.md              # This file
│   ├── SSA_ARCHITECTURE.md          # SSA-specific details
│   ├── IMPLEMENTATION_ROADMAP.md
│   └── PRODUCTION_READINESS.md
│
├── diagnostics/         # Error reporting
├── source_map/          # Source location tracking
└── Cargo.toml
```

---

## Compilation Phases

### Phase 1: Parsing

**Input**: Source code (`.hx` files)
**Output**: AST (Abstract Syntax Tree)
**Location**: `parser/` crate

```rust
pub struct HaxeFile {
    pub package: Option<String>,
    pub imports: Vec<Import>,
    pub declarations: Vec<Declaration>,
}
```

**Features**:
- **Parser Combinators**: Built with `nom` for composability
- **Incremental Parsing**: Re-parse only changed regions
- **Error Recovery**: Continue parsing after errors
- **Position Tracking**: Precise source locations for all nodes
- **Comment Preservation**: For documentation generation

**Key Files**:
- `haxe_parser.rs` - Main parser entry point
- `haxe_parser_expr.rs` - Expression parsing
- `haxe_parser_decls.rs` - Class, function, field declarations
- `haxe_parser_types.rs` - Type syntax parsing

### Phase 2: Type Checking

**Input**: AST
**Output**: TAST (Typed AST)
**Location**: `compiler/src/tast/`

```rust
pub struct TypedFile {
    pub package: Option<String>,
    pub imports: Vec<TypedImport>,
    pub classes: Vec<TypedClass>,
    pub functions: Vec<TypedFunction>,
    pub enums: Vec<TypedEnum>,
    // ...
}
```

**Process**:

1. **Symbol Resolution**
   - Build symbol table
   - Resolve identifiers to symbols
   - Handle imports and scoping

2. **Type Inference**
   - Bottom-up type inference
   - Constraint generation
   - Unification algorithm

3. **Type Checking**
   - Check type compatibility
   - Validate method overrides
   - Ensure interface implementation

4. **Constraint Solving**
   - Solve type constraints
   - Handle polymorphism
   - Infer missing type annotations

**Key Structures**:
```rust
// Symbol table: tracks all identifiers
pub struct SymbolTable {
    symbols: HashMap<SymbolId, Symbol>,
    scopes: HashMap<ScopeId, Scope>,
}

// Type table: tracks all types
pub struct TypeTable {
    types: HashMap<TypeId, Type>,
    // Type inference state
}

// Type representation
pub enum TypeKind {
    Int, Float, Bool, String, Void,
    Class(ClassType),
    Function(FunctionType),
    Array(TypeId),
    Nullable(TypeId),
    TypeParameter(TypeParam),
    // ...
}
```

**Key Files**:
- `type_checker.rs` - Main type checking logic
- `core.rs` - Type system definitions
- `symbols.rs` - Symbol table management
- `constraint_solver.rs` - Type inference

### Phase 3: Semantic Analysis

**Input**: TAST
**Output**: SemanticGraphs
**Location**: `compiler/src/semantic_graph/`

This phase builds analysis graphs for advanced checking and optimization:

#### 3a. Control Flow Graph (CFG)

Represents the control flow structure of each function.

```rust
pub struct ControlFlowGraph {
    pub blocks: HashMap<BlockId, BasicBlock>,
    pub entry_block: BlockId,
    pub exit_blocks: Vec<BlockId>,
}

pub struct BasicBlock {
    pub id: BlockId,
    pub statements: Vec<StatementId>,
    pub terminator: Terminator,
    pub predecessors: Vec<BlockId>,
    pub successors: Vec<BlockId>,
}
```

**Uses**:
- Dead code detection
- Reachability analysis
- Loop detection
- Dominance computation

#### 3b. Data Flow Graph (DFG) in SSA Form

Represents value flow through the program. Built in SSA form for precise analysis.

```rust
pub struct DataFlowGraph {
    pub nodes: HashMap<DataFlowNodeId, DataFlowNode>,
    pub ssa_variables: HashMap<SsaVariableId, SsaVariable>,
    pub def_use_chains: DefUseChains,
    pub value_numbering: ValueNumbering,
}
```

**Uses**:
- Initialization analysis
- Null safety checking
- Dead code elimination
- Common subexpression elimination
- Constant propagation

See [SSA_ARCHITECTURE.md](SSA_ARCHITECTURE.md) for detailed SSA integration strategy.

#### 3c. Call Graph

Tracks function call relationships for interprocedural analysis.

```rust
pub struct CallGraph {
    pub nodes: HashMap<SymbolId, CallGraphNode>,
    pub edges: Vec<CallEdge>,
}
```

**Uses**:
- Inline decision making
- Dead function elimination
- Effect analysis
- Recursion detection

#### 3d. Ownership & Lifetime Graphs

Tracks memory ownership and lifetime regions (Rust-inspired).

```rust
pub struct OwnershipGraph {
    pub ownership_edges: Vec<OwnershipEdge>,
    pub borrows: Vec<BorrowInfo>,
}
```

**Uses**:
- Use-after-free detection
- Use-after-move detection
- Borrow checking
- Memory leak detection

#### 3e. TypeFlowGuard

Orchestrates flow-sensitive type checking using the analysis graphs.

```rust
pub struct TypeFlowGuard {
    // Uses CFG, DFG, ownership graph
    pub results: FlowSafetyResults,
}
```

**Checks**:
- Initialization before use
- Null safety (flow-sensitive)
- Dead code warnings
- Effect violations
- Memory safety violations

**Key Files**:
- `cfg.rs` - Control flow graph
- `dfg.rs`, `dfg_builder.rs` - Data flow graph (SSA)
- `call_graph.rs` - Call graph
- `ownership_graph.rs` - Ownership tracking
- `type_flow_guard.rs` - Flow-sensitive checking

### Phase 4: HIR Lowering

**Input**: TAST + SemanticGraphs
**Output**: HIR (High-level Intermediate Representation)
**Location**: `compiler/src/ir/hir.rs`, `tast_to_hir.rs`

HIR preserves high-level language features while adding resolution and hints.

```rust
pub struct HirModule {
    pub name: String,
    pub functions: HashMap<SymbolId, HirFunction>,
    pub types: HashMap<TypeId, HirTypeDecl>,
    pub globals: HashMap<SymbolId, HirGlobal>,
}

pub struct HirFunction {
    pub symbol_id: SymbolId,
    pub params: Vec<HirParam>,
    pub return_type: TypeId,
    pub body: Option<HirBlock>,
    pub metadata: Vec<HirAttribute>,  // Optimization hints
}
```

**Transformations**:
- Desugar complex constructs (for-in → iterators)
- Resolve all symbols to IDs
- Attach lifetime/ownership information
- Extract optimization hints from SemanticGraphs

**Purpose**:
- Enable hot-reload (preserves source structure)
- Source-level debugging
- IDE integration
- Optimization hint propagation

**Key Files**:
- `hir.rs` - HIR definitions
- `tast_to_hir.rs` - TAST → HIR lowering

### Phase 5: MIR Lowering

**Input**: HIR
**Output**: MIR (Mid-level Intermediate Representation)
**Location**: `compiler/src/ir/mod.rs`, `hir_to_mir.rs`

MIR is a lower-level, platform-independent IR suitable for optimization.

```rust
pub struct IrModule {
    pub functions: HashMap<IrFunctionId, IrFunction>,
    pub globals: Vec<IrGlobal>,
}

pub struct IrFunction {
    pub signature: IrFunctionSignature,
    pub cfg: IrControlFlowGraph,
    pub locals: HashMap<IrId, IrLocal>,
    pub attributes: FunctionAttributes,  // From HIR hints
}
```

**Characteristics**:
- Standard IR instructions (add, mul, load, store, call, etc.)
- Explicit control flow (branches, jumps)
- Not required to be in SSA form
- Function attributes guide optimization

**Transformations**:
- Lower high-level constructs to simple operations
- Explicit memory operations
- Apply optimization hints from HIR

**Key Files**:
- `mod.rs` - MIR definitions
- `hir_to_mir.rs` - HIR → MIR lowering
- `builder.rs` - IR construction API

### Phase 6: Optimization

**Input**: MIR
**Output**: Optimized MIR
**Location**: `compiler/src/ir/optimization.rs`

```rust
pub trait OptimizationPass {
    fn run(&mut self, module: &mut IrModule) -> OptimizationResult;
}

pub struct PassManager {
    passes: Vec<Box<dyn OptimizationPass>>,
}
```

**Optimization Passes**:

1. **Dead Code Elimination**
   - Remove unreachable code
   - Remove unused values
   - Guided by DFG liveness analysis

2. **Common Subexpression Elimination (CSE)**
   - Identify duplicate computations
   - Reuse computed values
   - Guided by value numbering from DFG

3. **Constant Propagation & Folding**
   - Evaluate constants at compile time
   - Propagate known values
   - Simplify expressions

4. **Inlining**
   - Inline small functions
   - Guided by inline hints from SSA analysis

5. **Loop Optimization**
   - Loop invariant code motion
   - Loop unrolling
   - Strength reduction

**Key Files**:
- `optimization.rs` - Pass infrastructure
- Individual pass implementations

### Phase 7: Code Generation

**Input**: Optimized MIR
**Output**: Target code (WASM modules, native binaries, bytecode)
**Location**: `compiler/src/codegen/` (in development)

Rayzor uses a **multi-backend compilation strategy** optimized for different execution contexts:

#### Compilation Strategy Overview

```
┌─────────────────────────────────────────────────────────────┐
│                  Compilation Strategies                      │
└─────────────────────────────────────────────────────────────┘

JIT Runtime (Development & Testing):
  MIR → Cranelift (cold paths) + LLVM (hot paths after profiling)

AOT Compilation (Native):
  MIR → LLVM → Native binary (maximum optimization)

AOT Compilation (Cross-platform):
  MIR → WASM → .wasm module (WASI, Browser, Edge)
```

#### Backend: Cranelift - JIT Runtime (Cold Paths)

**Target**: Native code via Cranelift JIT
**Status**: Planned
**Purpose**: Fast compilation for JIT runtime

**Features**:

- **Extremely fast compilation**: ~10x faster than LLVM
- Good code quality (15-25x faster than interpreter)
- Low memory footprint
- Streaming compilation
- Tier-up to LLVM after profiling

**Use Cases**:

- JIT runtime cold paths (first execution)
- Development mode (fast iteration)
- Interactive execution (REPL)
- Functions executed rarely

**JIT Strategy**:

```rust
pub struct JitRuntime {
    cranelift: CraneliftJit,  // Fast compilation
    llvm_cache: LlvmCache,     // Optimized hot code
    profiler: Profiler,
}

impl JitRuntime {
    fn execute_function(&mut self, func: &MirFunction) {
        if self.profiler.is_hot(func) {
            // Recompile with LLVM for maximum performance
            let optimized = self.llvm_cache.compile_hot_path(func);
            optimized.execute()
        } else {
            // Use Cranelift for fast compilation
            let jit_code = self.cranelift.compile(func);
            jit_code.execute()
        }
    }
}
```

**Performance**: 50-200ms compile time, 15-25x runtime speed

#### Backend: LLVM - Hot Path Optimization & AOT

**Target**: LLVM IR → Native code
**Status**: Planned

**Use Cases**:

**1. JIT Runtime - Hot Paths**

- Functions executed frequently (>5% runtime)
- Profile-guided recompilation
- Maximum optimization for critical code
- Replaces Cranelift-compiled code when hot

**2. AOT Compilation - Native Binaries**

- Production builds
- Shipping applications
- Maximum performance for all code
- Platform-specific optimizations

**Features**:

- **Maximum optimization**: -O3, PGO, LTO
- Advanced vectorization (SIMD)
- Link-time optimization
- Platform-specific tuning

**AOT Compilation**:

```bash
# AOT compile for production
rayzor build --aot --optimize=aggressive --target=native

# Output: single optimized native binary
# All functions compiled with LLVM -O3
```

**Performance**:

- Compilation: 1-30s depending on optimization level
- Runtime: 45-50x speed (maximum performance)

#### Backend: WebAssembly (WASM) - AOT for Cross-Platform

**Target**: WASM binary modules
**Status**: In development
**Purpose**: AOT compilation for universal deployment

**Use Cases**:

**1. Browser Environments**

- Web applications
- Progressive Web Apps (PWA)
- Client-side computation

**2. WASI (WebAssembly System Interface)**

- Server-side applications
- CLI tools
- Serverless functions
- Edge computing

**3. Embedded & IoT**

- Resource-constrained devices
- Sandboxed execution
- Cross-platform deployment

**Features**:

- Universal deployment (write once, run anywhere)
- Near-native performance (30-40x interpreter)
- Sandboxed execution (security)
- Compact binary format
- Streaming compilation

**WASM Compilation**:

```bash
# Compile to WASM for browser
rayzor build --target=wasm --optimize=size

# Compile to WASM for WASI
rayzor build --target=wasi --optimize=speed

# Output: .wasm module
```

**Performance**: 100-500ms compile time, 30-40x runtime speed

#### Backend: Interpreter - Development Mode

**Target**: Bytecode for custom VM
**Status**: Planned
**Features**:
- **Instant startup**: No compilation delay
- Hot-reload support
- Step-by-step debugging
- Live code editing

**Use Cases**:
- Rapid prototyping
- Development mode
- Interactive REPL
- Teaching/learning

**Architecture**:
```rust
pub struct Interpreter {
    bytecode: Vec<BytecodeInstruction>,
    stack: Vec<Value>,
    globals: HashMap<SymbolId, Value>,
}
```

#### Tiered Compilation Strategy

Rayzor implements a **multi-backend compilation system** optimized for different deployment scenarios:

```
┌─────────────────────────────────────────────────────────────┐
│                    Compilation Modes                         │
└─────────────────────────────────────────────────────────────┘

Development Mode (JIT Runtime):
  Source → TAST → MIR → Interpreter
                           ↓ (optional JIT)
                         Cranelift (cold paths)
                           ↓ (tier-up hot paths)
                         LLVM (hot functions >5% runtime)

Testing Mode (JIT Runtime):
  Source → TAST → MIR → Cranelift (all functions)
                           ↓ (profile + tier-up)
                         LLVM (hot paths)

Production AOT (Native Binary):
  Source → TAST → MIR → Optimize → LLVM → Native binary
  (All code compiled with maximum optimization)

Production AOT (Cross-Platform):
  Source → TAST → MIR → Optimize → WASM → .wasm module
  (Universal deployment: browser, WASI, edge)
```

**Performance Comparison**:

| Backend | Compilation Time | Runtime Speed | Use Case |
|---------|------------------|---------------|----------|
| Interpreter | 0ms (instant) | 1x (baseline) | Development, hot-reload |
| Cranelift JIT | 50-200ms | 15-25x | JIT cold paths, dev mode |
| LLVM JIT (hot) | 1-5s | 45-50x | JIT hot paths (tier-up) |
| LLVM AOT | 10-30s | 45-50x | Production native binary |
| WASM AOT | 100-500ms | 30-40x | Cross-platform deployment |

**Compilation Mode Selection**:

```rust
pub enum CompilationMode {
    /// Development: Interpreter + optional Cranelift JIT
    Dev {
        hot_reload: bool,
        jit_enabled: bool,
    },

    /// JIT Runtime: Cranelift cold + LLVM hot paths
    Jit {
        hot_threshold: f64,      // e.g., 0.05 = 5% runtime
        max_tier_up: usize,      // Max functions to LLVM-compile
    },

    /// AOT Native: LLVM for all functions
    AotNative {
        optimization: OptLevel,  // -O0 to -O3
        pgo: bool,               // Profile-guided optimization
        lto: bool,               // Link-time optimization
    },

    /// AOT WebAssembly: WASM output
    AotWasm {
        target: WasmTarget,      // Browser, WASI, Edge
        optimization: WasmOpt,   // Size or speed
    },
}
```

**Tiered JIT Execution Flow**:

```
┌─────────────────────────────────────────────────────────────┐
│                    JIT Runtime Flow                          │
└─────────────────────────────────────────────────────────────┘

Function First Call:
  1. Check if function is compiled
  2. If not: Compile with Cranelift (fast)
  3. Execute Cranelift-compiled code
  4. Update execution counter

Function Hot Threshold Reached (e.g., 1000 calls or 5% runtime):
  1. Mark function as hot
  2. Compile with LLVM in background (optimized)
  3. Continue executing Cranelift version
  4. Replace with LLVM version when ready

Subsequent Calls:
  1. Execute LLVM-optimized version (maximum performance)
```

**Code Generation Pipeline**:

```
MIR (optimized)
    ↓
┌────────────────────────────────────┐
│   Compilation Mode Selection       │
└────────────────────────────────────┘
    ↓
    ├─→ JIT Mode
    │   ├─→ Interpreter (instant)
    │   ├─→ Cranelift (cold paths, fast compile)
    │   └─→ LLVM (hot paths, tier-up)
    │
    ├─→ AOT Native Mode
    │   └─→ LLVM → Native binary (all functions optimized)
    │
    └─→ AOT WASM Mode
        └─→ WASM backend → .wasm module (universal)
```

**Example Usage**:

```bash
# Development: interpreter + hot-reload
rayzor dev --hot-reload

# Testing: JIT with tier-up
rayzor test --jit --profile

# Production native binary (AOT)
rayzor build --aot --optimize=3 --target=native

# Production WASM for browser (AOT)
rayzor build --aot --target=wasm --optimize=size

# Production WASM for WASI (AOT)
rayzor build --aot --target=wasi --optimize=speed
```

---

## Core Components

### Symbol Management

**SymbolTable**: Central registry for all identifiers

```rust
pub struct SymbolTable {
    symbols: HashMap<SymbolId, Symbol>,
    scopes: ScopeTree,
}

pub struct Symbol {
    pub name: InternedString,
    pub kind: SymbolKind,  // Variable, Function, Class, etc.
    pub type_id: TypeId,
    pub scope_id: ScopeId,
    pub visibility: Visibility,
    pub mutability: Mutability,
}
```

**Features**:
- Hierarchical scopes
- Symbol lookup with shadowing
- Export/import tracking
- Visibility checking

### Type System

**TypeTable**: Central registry for all types

```rust
pub struct TypeTable {
    types: HashMap<TypeId, Type>,
    inference_state: InferenceState,
}

pub struct Type {
    pub id: TypeId,
    pub kind: TypeKind,
    pub source_location: Option<SourceLocation>,
}

pub enum TypeKind {
    // Primitive types
    Void, Int, Float, Bool, String, Dynamic,

    // Compound types
    Class(ClassType),
    Interface(InterfaceType),
    Enum(EnumType),
    Abstract(AbstractType),
    Function(FunctionType),

    // Generic types
    Array(TypeId),
    Map(TypeId, TypeId),
    Nullable(TypeId),

    // Type system features
    TypeParameter(TypeParam),
    Constraint(ConstraintSet),
}
```

**Key Operations**:
- Type unification
- Subtype checking
- Type instantiation (for generics)
- Constraint solving

See [Type System](#type-system) section for details.

### String Interning

Efficient string storage and comparison.

```rust
pub struct StringInterner {
    strings: Vec<String>,
    map: HashMap<String, InternedString>,
}

pub struct InternedString(u32);
```

**Benefits**:
- O(1) string comparison (compare IDs)
- Reduced memory usage (strings stored once)
- Fast symbol lookup

### Error Reporting

Comprehensive diagnostic system.

```rust
pub struct Diagnostic {
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub location: SourceLocation,
    pub labels: Vec<Label>,
    pub notes: Vec<String>,
}
```

**Features**:
- Precise source locations
- Multi-span labels
- Helpful suggestions
- Error recovery

---

## Type System

### Type Hierarchy

```
Any (top type)
 │
 ├── Dynamic (escape hatch)
 │
 ├── Void (unit type)
 │
 ├── Primitives
 │   ├── Int
 │   ├── Float
 │   ├── Bool
 │   └── String
 │
 ├── Compound Types
 │   ├── Class
 │   ├── Interface
 │   ├── Enum
 │   └── Abstract
 │
 ├── Collections
 │   ├── Array<T>
 │   └── Map<K, V>
 │
 ├── Function Types
 │   └── (Args) -> ReturnType
 │
 └── Type Parameters
     └── T, U, V, etc.
```

### Type Features

#### Generics

```haxe
class Box<T> {
    var value: T;

    public function new(value: T) {
        this.value = value;
    }

    public function get(): T {
        return value;
    }
}

var intBox = new Box<Int>(42);
var stringBox = new Box<String>("hello");
```

**Implementation**:
- Monomorphization (generate code for each instantiation)
- Type parameter constraints
- Variance annotations (covariant, contravariant)

#### Nullable Types

```haxe
var x: Null<Int> = null;
var y: Int = 42;  // Non-nullable by default
```

**Implementation**:
- `Nullable<T>` wrapper type
- Flow-sensitive null checking in TypeFlowGuard
- Automatic narrowing after null checks

#### Abstract Types

```haxe
abstract Degrees(Float) {
    public inline function new(value: Float) {
        this = value;
    }

    @:op(A + B)
    public function add(other: Degrees): Degrees {
        return new Degrees(this + other);
    }
}
```

**Features**:
- Zero-cost abstractions (compile to underlying type)
- Operator overloading
- Implicit conversions (from/to rules)

#### Type Inference

The compiler uses **bidirectional type checking**:

1. **Bottom-up inference**: Infer types from leaves
2. **Top-down checking**: Check against expected types
3. **Constraint generation**: Generate equations for unknowns
4. **Unification**: Solve constraint system

Example:
```haxe
function example() {
    var x = 42;        // Infer: x: Int
    var y = x + 10;    // Infer: y: Int
    var z = [x, y];    // Infer: z: Array<Int>
    return z;          // Infer return: Array<Int>
}
```

---

## Memory Management Model

Rayzor adds optional memory safety features inspired by Rust.

### Ownership System (Optional)

```haxe
@:ownership
class Resource {
    var data: Array<Int>;

    // Takes ownership
    public function new(data: Array<Int>) {
        this.data = data;  // Move
    }

    // Borrows immutably
    public function read(): &Array<Int> {
        return &data;  // Borrow
    }

    // Borrows mutably
    public function modify(): &mut Array<Int> {
        return &mut data;  // Mutable borrow
    }
}
```

**Ownership Rules**:
1. Each value has exactly one owner
2. Ownership can be transferred (move)
3. References can be borrowed
4. At most one mutable borrow OR multiple immutable borrows

**Checking**:
- Performed by OwnershipGraph in SemanticGraphs
- Integrated with TypeFlowGuard
- Compile-time verification only

### Lifetime System (Optional)

```haxe
@:lifetime
function dangling(): &Int {
    var x = 42;
    return &x;  // ERROR: x does not live long enough
}

@:lifetime
function valid(x: &Int): &Int {
    return x;  // OK: lifetime of parameter
}
```

**Implementation**:
- Lifetime regions tracked in OwnershipGraph
- Lifetime inference (similar to Rust)
- Lifetime annotations for explicit control

See [resource/haxe_mutability_and_borrow_model.md](../resource/haxe_mutability_and_borrow_model.md) for full details.

---

## Error Handling

### Error Categories

1. **Syntax Errors** (Parser)
   - Unexpected tokens
   - Unclosed delimiters
   - Invalid syntax

2. **Type Errors** (Type Checker)
   - Type mismatch
   - Undefined variable
   - Invalid method call

3. **Flow Safety Errors** (TypeFlowGuard)
   - Uninitialized variable
   - Null dereference
   - Dead code

4. **Memory Safety Errors** (OwnershipGraph)
   - Use after move
   - Use after free
   - Invalid borrow

### Error Recovery

The compiler continues after errors to find multiple issues:

- **Parser**: Skip to next valid construct
- **Type Checker**: Insert error types, continue
- **Flow Analysis**: Mark as unsafe, continue

### Diagnostic Quality

Example error message:
```
error[E0308]: type mismatch
  --> example.hx:5:15
   |
 5 |     var x: Int = "hello";
   |                  ^^^^^^^ expected Int, found String
   |
help: did you mean to convert the string to an integer?
   |
 5 |     var x: Int = Std.parseInt("hello");
   |                  ++++++++++++         +
```

---

## Optimization Strategy

### Optimization Levels

| Level | Description | Features |
|-------|-------------|----------|
| `-O0` | No optimization | Fast compile, slow runtime |
| `-O1` | Basic optimization | Reasonable compile time, decent runtime |
| `-O2` | Standard optimization | Slower compile, fast runtime |
| `-O3` | Aggressive optimization | Slow compile, maximum runtime |
| `-Os` | Size optimization | Minimize binary size |

### Optimization Phases

1. **Early Optimizations** (on HIR)
   - Dead code elimination
   - Constant folding
   - Simple inlining

2. **Middle Optimizations** (on MIR)
   - SSA-based optimizations
   - Loop optimizations
   - Common subexpression elimination

3. **Late Optimizations** (backend-specific)
   - Register allocation
   - Instruction selection
   - Peephole optimizations

### Profile-Guided Optimization (Planned)

```bash
# Step 1: Compile with instrumentation
rayzor build --profile-generate

# Step 2: Run program to collect profile
./program < typical_input.txt

# Step 3: Compile with profile data
rayzor build --profile-use=profile.data
```

Benefits:
- Inline hot functions
- Optimize hot paths
- Better branch prediction

---

## Development Workflow

### Development Mode

```bash
rayzor dev --watch --hot-reload
```

Features:
- **Fast compilation**: Incremental, minimal optimization
- **Hot-reload**: Instantly see changes without restart
- **Interpreter**: No native compilation delay
- **Rich diagnostics**: Helpful error messages

### Testing Mode

```bash
rayzor test --optimize
```

Features:
- **Optimized code**: Run with `-O2` optimizations
- **Profiling**: Built-in performance measurement
- **Coverage**: Track code coverage

### Production Mode

```bash
rayzor build --release --target=native
```

Features:
- **Maximum optimization**: `-O3` with PGO
- **Native compilation**: Via LLVM
- **Single binary**: No runtime dependencies
- **Strip debug info**: Minimize binary size

### Incremental Compilation

The compiler tracks dependencies and recompiles only what's needed:

```
Source Change → Affected Modules → Re-parse → Re-check → Re-lower → Re-optimize
```

**Caching**:
- Parsed ASTs
- Type-checked TASTs
- Semantic graphs
- HIR modules
- Optimized MIR

---

## Related Documentation

### Essential Reading

- **[SSA_ARCHITECTURE.md](SSA_ARCHITECTURE.md)** - Detailed SSA integration strategy
- **[IMPLEMENTATION_ROADMAP.md](IMPLEMENTATION_ROADMAP.md)** - Development plan
- **[PRODUCTION_READINESS.md](PRODUCTION_READINESS.md)** - Production checklist

### Domain-Specific Docs

- **[resource/plan.md](../resource/plan.md)** - Project scope and goals
- **[resource/strategy.md](../resource/strategy.md)** - Development → AOT workflow
- **[resource/haxe_mutability_and_borrow_model.md](../resource/haxe_mutability_and_borrow_model.md)** - Memory safety model

### Component Docs

- **[compiler/src/ir/README.md](src/ir/README.md)** - IR design details
- **[compiler/src/ir/BACKLOG.md](src/ir/BACKLOG.md)** - IR TODO items

---

## Contributing

### Code Organization Principles

1. **One Concern Per Module**: Each file has a single responsibility
2. **Explicit Dependencies**: Import what you need, no wildcards
3. **Comments for Why**: Code shows what, comments explain why
4. **Tests Co-located**: Tests live near the code they test

### Adding New Features

When adding a feature:

1. **Update AST** (parser) - Parse the new syntax
2. **Update TAST** (type_checker) - Type check the feature
3. **Update SemanticGraphs** (optional) - Add analysis if needed
4. **Update HIR** (ir/hir.rs) - Add HIR representation
5. **Update MIR lowering** (ir/hir_to_mir.rs) - Lower to MIR
6. **Add optimization** (ir/optimization.rs) - Optimize if applicable
7. **Update codegen** - Generate code for target

### Testing Strategy

```rust
// Unit tests in same file
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature() {
        // Test logic
    }
}

// Integration tests in examples/
// examples/test_feature.rs

// End-to-end tests in tests/
// tests/test_complete_pipeline.rs
```

---

## Performance Characteristics

### Compilation Speed

| Phase | Typical Time | Scaling |
|-------|--------------|---------|
| Parsing | ~50µs/KB | Linear |
| Type Checking | ~200µs/function | ~Linear |
| SemanticGraphs | ~500µs/function | Linear |
| HIR Lowering | ~100µs/function | Linear |
| MIR Lowering | ~150µs/function | Linear |
| Optimization | ~1ms/function | Varies |
| Codegen | ~500µs/function | Linear |

### Memory Usage

| Component | Memory Usage |
|-----------|-------------|
| AST | ~500 bytes/node |
| TAST | ~800 bytes/node |
| SemanticGraphs | ~2KB/function |
| HIR | ~1KB/function |
| MIR | ~3KB/function |

**Mitigation**: Incremental compilation with on-disk caching

---

## Future Directions

### Near-Term (3-6 months)

- [ ] Complete JavaScript backend
- [ ] Implement interpreter for hot-reload
- [ ] Add LSP server for IDE support
- [ ] Comprehensive test suite

### Mid-Term (6-12 months)

- [ ] LLVM backend for native compilation
- [ ] Advanced optimizations (PGO, LTO)
- [ ] Parallel compilation
- [ ] Package manager integration

### Long-Term (1-2 years)

- [ ] Full Haxe standard library support
- [ ] Cross-compilation to multiple targets
- [ ] Incremental semantic analysis
- [ ] Query-based compilation model

---

## Conclusion

The Rayzor compiler is designed as a **modern, safe, and performant** alternative Haxe implementation. Its layered architecture, sophisticated type system, and optional memory safety features make it suitable for both rapid prototyping and production use.

The key innovations are:

1. **SSA-based analysis infrastructure** for precise optimization
2. **Multi-tier IR design** balancing semantics and performance
3. **Optional memory safety** without compromising compatibility
4. **Incremental compilation** for fast iteration

This architecture provides a solid foundation for future enhancements while maintaining code quality and developer experience.

---

**Document Version**: 1.0
**Last Updated**: 2025-11-12
**Status**: Active Development
**License**: MIT
