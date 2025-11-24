# IR Architecture Overview

## Three-Level IR Pipeline

According to the architecture plan, we have a three-level IR pipeline optimized for machine code generation:

### 1. HIR (High-level IR) - `hir.rs`
- **Purpose**: Preserve source-level semantics with resolved types
- **Features**:
  - Close to Haxe syntax
  - Pattern matching preserved
  - Comprehensions, string interpolation intact
  - Metadata/attributes preserved
  - Lifetime and ownership information attached
- **Lowering**: TAST → HIR via `tast_to_hir.rs`

### 2. MIR (Mid-level IR) - Current IR implementation
- **Purpose**: SSA form for optimization AND interpretable for development
- **Features**:
  - SSA with phi nodes (`blocks.rs`, `instructions.rs`)
  - CFG construction (`functions.rs`)
  - Optimization passes (`optimization.rs`)
  - Type-checked and validated (`validation.rs`)
  - Platform-independent
  - **Interpretable for hot reloading** (development mode)
  - **VM execution support** (fast iteration)
- **Lowering**: HIR → MIR via `hir_to_mir.rs`
- **Usage Modes**:
  - Development: Direct interpretation for hot reload
  - Production: Further lowering to LIR for machine code
- **Key Components**:
  - `IrBuilder`: Construct MIR programmatically
  - `IrInstruction`: Low-level operations (load, store, arithmetic, control flow)
  - `IrBasicBlock`: CFG nodes with phi nodes
  - `IrOptimization`: Dead code elimination, constant folding, inlining, etc.

### 3. LIR (Low-level IR) - To be implemented
- **Purpose**: Target-specific code generation
- **Features**:
  - Machine-specific instructions
  - Register allocation hints
  - Calling convention specifics
  - LLVM IR generation or direct assembly
- **Targets**:
  - LLVM backend
  - Custom x86_64/ARM64 assembly generation

## Current State

### ✅ Implemented
- **HIR**: Full Haxe feature support
- **MIR**: Complete SSA-based IR with optimizations
- **TAST → HIR**: Basic lowering implemented
- **HIR → MIR**: Framework in place, core lowering implemented

### 🚧 In Progress
- **HIR → MIR**: Complete all lowering cases
- **MIR Optimizations**: Additional optimization passes

### 📋 TODO
- **MIR → LIR**: Target-specific lowering
- **LLVM Backend**: Generate LLVM IR from LIR
- **Assembly Backend**: Direct assembly generation

## Integration Points

### Pipeline Flow
```
TAST (from parser/type checker)
  ↓ tast_to_hir.rs
HIR (high-level, source-like)
  ↓ hir_to_mir.rs
MIR (SSA form, optimizable)
  ↓ optimization passes
Optimized MIR
  ↓ mir_to_lir.rs (TODO)
LIR (target-specific)
  ↓ codegen backend
Machine Code (via LLVM or direct assembly)
```

### Key Design Decisions

1. **No Source-to-Source**: We're targeting machine code only
   - No JavaScript/C++ source generation
   - Focus on LLVM and native assembly backends

2. **MIR as Optimization Layer**: The existing IR serves as MIR
   - Already has SSA form
   - Rich optimization infrastructure
   - Platform-independent

3. **HIR for Language Features**: New HIR preserves Haxe semantics
   - Pattern matching
   - Comprehensions
   - Metadata for optimization hints

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

// Future: Lower to LIR and generate machine code
// let lir_module = lower_mir_to_lir(&mir_module, Target::X86_64)?;
// let machine_code = generate_llvm(&lir_module)?;
```

## Optimization Strategy

### HIR Level
- Lifetime analysis
- Ownership checking
- Effect analysis
- Purity detection

### MIR Level (Current IR)
- Dead code elimination
- Constant folding/propagation
- Function inlining
- Loop optimizations
- CSE (Common Subexpression Elimination)

### LIR Level (Future)
- Register allocation
- Instruction selection
- Peephole optimization
- Target-specific optimizations

## File Organization

```
ir/
├── README.md           # This file
├── mod.rs             # Module exports
│
├── hir.rs             # HIR definitions
├── tast_to_hir.rs     # TAST → HIR lowering
├── hir_to_mir.rs      # HIR → MIR lowering
│
├── types.rs           # MIR type system
├── instructions.rs    # MIR instruction set
├── blocks.rs          # MIR basic blocks & CFG
├── functions.rs       # MIR function representation
├── modules.rs         # MIR module structure
├── builder.rs         # MIR construction API
├── lowering.rs        # Legacy TAST → MIR (being replaced)
│
├── optimization.rs    # MIR optimization passes
├── validation.rs      # MIR validation
│
└── lir/              # Future: Low-level IR
    ├── mod.rs
    ├── x86_64.rs
    ├── arm64.rs
    └── llvm.rs
```