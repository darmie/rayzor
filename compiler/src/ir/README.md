# IR Architecture Overview

## Two-Level IR Pipeline

Rayzor uses a two-level IR pipeline optimized for both development iteration speed and production-quality machine code generation.

### 1. HIR (High-level IR) - `hir.rs`
- **Purpose**: Preserve source-level semantics with resolved types
- **Features**:
  - Close to Haxe syntax
  - Pattern matching preserved
  - Comprehensions, string interpolation intact
  - Metadata/attributes preserved
  - Lifetime and ownership information attached
- **Lowering**: TAST -> HIR via `tast_to_hir.rs`
- **Status**: Complete (~90%)

### 2. MIR (Mid-level IR) - Core IR implementation
- **Purpose**: SSA form for optimization AND interpretable for development
- **Features**:
  - SSA with phi nodes (`blocks.rs`, `instructions.rs`)
  - CFG construction (`functions.rs`)
  - Optimization passes (`optimization.rs`)
  - Type-checked and validated (`validation.rs`)
  - Platform-independent
  - **Interpretable for hot reloading** (development mode)
  - **VM execution support** (fast iteration)
- **Lowering**: HIR -> MIR via `hir_to_mir.rs` (~13,000 lines)
- **Usage Modes**:
  - Development: Direct interpretation via MIR Interpreter (Tier 0)
  - JIT: Lowered to Cranelift IR (Tiers 1-3)
  - Production: Lowered to LLVM IR (Tier 4, AOT)
- **Status**: Complete (~95%, ~31,000 LOC across IR modules)
- **Key Components**:
  - `IrBuilder`: Construct MIR programmatically
  - `IrInstruction`: Low-level operations (load, store, arithmetic, control flow)
  - `IrBasicBlock`: CFG nodes with phi nodes
  - `IrOptimization`: Dead code elimination, constant folding, inlining, etc.

### Code Generation Backends (serve as LIR)

Rather than a separate LIR layer, Rayzor lowers MIR directly to backend-specific representations:

- **MIR Interpreter** (`codegen/mir_interpreter.rs`): NaN-boxing interpreter for Tier 0
- **Cranelift** (`codegen/cranelift_backend.rs`): JIT compilation for Tiers 1-3
- **LLVM** (`codegen/llvm_backend.rs`, `codegen/llvm_jit_backend.rs`): Max optimization + AOT object files

Each backend handles instruction selection, register allocation, and target-specific optimizations internally.

## Current State

### Implemented
- **HIR**: Full Haxe feature support (closures, for-in, try-catch, pattern matching, string interpolation)
- **MIR**: Complete SSA-based IR with phi nodes, type metadata, global init
- **TAST -> HIR**: Full lowering implemented
- **HIR -> MIR**: Complete lowering (~13,000 lines) including closures, lambdas, stdlib mapping
- **MIR Optimizations**: 5 core passes + function inlining + loop analysis + SIMD vectorization infrastructure
- **MIR Interpreter**: Full interpreter with NaN-boxing value representation
- **Cranelift Backend**: Full JIT compilation with 3 optimization levels
- **LLVM Backend**: Full compilation with -O3 optimization and AOT .o file generation
- **Drop Analysis**: Automatic Free instruction insertion based on last-use analysis
- **Escape Analysis**: Stack allocation optimization for non-escaping allocations
- **BLADE Cache**: Per-module MIR serialization with source hash validation
- **RayzorBundle (.rzb)**: Single-file distributable format

## Integration Points

### Pipeline Flow

```text
TAST (from parser/type checker)
  | tast_to_hir.rs
HIR (high-level, source-like)
  | hir_to_mir.rs (~13,000 lines)
MIR (SSA form, optimizable)
  | optimization passes
Optimized MIR
  |
  +-- mir_interpreter.rs ----> Direct execution (Tier 0)
  +-- cranelift_backend.rs --> JIT native code (Tiers 1-3)
  +-- llvm_backend.rs -------> LLVM IR -> native .o files (Tier 4 / AOT)
  |
  +-- blade_cache.rs --------> .blade module cache (incremental builds)
  +-- rayzor_bundle.rs ------> .rzb bundle (distributable)
```

### Key Design Decisions

1. **No Source-to-Source**: We're targeting machine code only
   - No JavaScript/C++ source generation
   - Focus on Cranelift JIT, LLVM AOT, and MIR interpretation

2. **MIR as Optimization Layer**: The existing IR serves as MIR
   - Full SSA form with phi nodes
   - Rich optimization infrastructure (DCE, constant folding, copy propagation, inlining, loop analysis)
   - Platform-independent

3. **HIR for Language Features**: HIR preserves Haxe semantics
   - Pattern matching
   - Comprehensions
   - Metadata for optimization hints

4. **No Separate LIR**: Backend-specific lowering replaces a traditional LIR
   - Cranelift handles its own instruction selection and register allocation
   - LLVM handles optimization and code generation internally
   - MIR Interpreter directly executes MIR instructions

## Usage Example

```rust
use compiler::ir::{
    hir::HirModule,
    tast_to_hir::lower_tast_to_hir,
    hir_to_mir::lower_hir_to_mir,
    optimization::PassManager,
};

// Lower TAST to HIR
let hir_module = lower_tast_to_hir(&typed_file, &symbol_table, &type_table, None)?;

// Lower HIR to MIR
let mut mir_module = lower_hir_to_mir(&hir_module)?;

// Run optimization passes
let mut pass_manager = PassManager::default_pipeline();
pass_manager.run(&mut mir_module);

// Execute via interpreter (Tier 0)
let result = MirInterpreter::new().execute(&mir_module, "main")?;

// Or compile via Cranelift JIT (Tiers 1-3)
let mut backend = CraneliftBackend::new();
backend.compile_module(&mir_module)?;
let func_ptr = backend.get_function_pointer("main")?;
```

## Optimization Strategy

### HIR Level
- Lifetime analysis
- Ownership checking
- Effect analysis
- Purity detection

### MIR Level (5 core passes + extended)
- **Dead Code Elimination**: Remove unreachable instructions and blocks
- **Constant Folding**: Evaluate constant expressions at compile time
- **Copy Propagation**: Eliminate redundant copies
- **Unreachable Block Elimination**: Remove blocks with no predecessors
- **Control Flow Simplification**: Merge blocks, simplify branches
- **Function Inlining**: Inline small functions (~612 LOC)
- **Loop Analysis**: Loop detection, invariant motion (~663 LOC)
- **SIMD Vectorization**: Auto-vectorization infrastructure (~993 LOC)

### Backend Level
- **Cranelift**: 3 optimization levels (speed, default, best)
- **LLVM**: Full -O3 optimization pipeline
- **MIR Interpreter**: NaN-boxing for efficient value representation

## File Organization

```text
ir/
+-- README.md              # This file
+-- mod.rs                 # Module exports
|
+-- hir.rs                 # HIR definitions
+-- tast_to_hir.rs         # TAST -> HIR lowering
+-- hir_to_mir.rs          # HIR -> MIR lowering (~13,000 lines)
|
+-- types.rs               # MIR type system
+-- instructions.rs        # MIR instruction set
+-- blocks.rs              # MIR basic blocks & CFG
+-- functions.rs           # MIR function representation
+-- modules.rs             # MIR module structure
+-- builder.rs             # MIR construction API
+-- lowering.rs            # Legacy TAST -> MIR (superseded by HIR path)
|
+-- optimization.rs        # MIR optimization passes
+-- validation.rs          # MIR validation
+-- drop_analysis.rs       # Drop point analysis (AutoDrop/RuntimeManaged/NoDrop)
+-- escape_analyzer.rs     # Escape analysis (stack vs heap optimization)
```
