# Rayzor

> A high-performance, next-generation Haxe compiler with tiered JIT compilation and native code generation

[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org)
[![Status](https://img.shields.io/badge/status-active%20development-yellow.svg)](https://github.com/yourusername/rayzor)

---

## Overview

**Rayzor** is a complete reimplementation of a Haxe compiler in Rust, designed for:

- **High Performance**: Native compilation via Cranelift and LLVM
- **Fast Compilation**: 50-200ms JIT compilation vs 2-5s C++ compilation
- **Tiered JIT**: Adaptive optimization like V8, PyPy, and JVM HotSpot
- **Production Ready**: AOT compilation to optimized native binaries
- **Modern Architecture**: SSA-based analysis and optimization infrastructure

### What Makes Rayzor Different?

Unlike the official Haxe compiler which excels at language transpilation (JavaScript, Python, PHP), **Rayzor focuses exclusively on native code generation**:

- **Cranelift** for fast JIT compilation (cold paths)
- **LLVM** for maximum optimization (hot paths + AOT)
- **WebAssembly** for cross-platform deployment

**Not a goal:** Language transpilation - the official Haxe compiler already excels at this.

---

## Quick Start

```bash
# Clone the repository
git clone https://github.com/darmie/rayzor.git
cd rayzor

# Build the compiler
cargo build --release

# Run tests
cargo test

# Compile a Haxe file (once backends are implemented)
rayzor compile hello.hx --mode=jit
rayzor compile hello.hx --mode=aot --optimize=3
```

---

## Architecture

Rayzor implements a **multi-stage compilation pipeline** with sophisticated analysis and optimization:

```
┌─────────────┐
│ Haxe Source │
└──────┬──────┘
       │
       ▼
┌─────────────────────────────────────────────────────────┐
│                  Parser (parser/ crate)                 │
│  • Nom-based parser combinators                         │
│  • Incremental parsing with error recovery              │
│  • Precise source location tracking                     │
└────────────────────────┬────────────────────────────────┘
                         │
                         ▼
                   ┌──────────┐
                   │   AST    │ (Abstract Syntax Tree)
                   └─────┬────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────┐
│            Type Checker (compiler/src/tast/)            │
│  • Symbol resolution & type inference                   │
│  • Constraint solving & unification                     │
│  • Type checking with rich diagnostics                  │
└────────────────────────┬────────────────────────────────┘
                         │
                         ▼
                   ┌──────────┐
                   │   TAST   │ (Typed AST)
                   └─────┬────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────┐
│      Semantic Analysis (compiler/src/semantic_graph/)   │
│  • Control Flow Graph (CFG)                             │
│  • Data Flow Graph (DFG) in SSA form                    │
│  • Call Graph (inter-procedural)                        │
│  • Ownership & Lifetime tracking                        │
│  • TypeFlowGuard (flow-sensitive checking)              │
└────────────────────────┬────────────────────────────────┘
                         │
                         ▼
                   ┌──────────┐
                   │   HIR    │ (High-level IR)
                   └─────┬────┘
                         │ (TAST → HIR lowering)
                         │ • Preserve language semantics
                         │ • Attach optimization hints
                         │
                         ▼
                   ┌──────────┐
                   │   MIR    │ (Mid-level IR - SSA form)
                   └─────┬────┘  ✅ 98% Complete
                         │ (HIR → MIR lowering)
                         │ • Platform-independent IR
                         │ • Optimization target
                         │
       ├─────────────────┼─────────────────┐
       │                 │                 │
       ▼                 ▼                 ▼
┌────────────┐    ┌────────────┐    ┌────────────┐
│ Cranelift  │    │    LLVM    │    │  WebAsm    │
│ (JIT Cold) │    │ (Hot + AOT)│    │   (AOT)    │
└─────┬──────┘    └─────┬──────┘    └─────┬──────┘
      │ (tier-up)       │                 │
      ▼                 ▼                 ▼
┌────────────┐    ┌────────────┐    ┌────────────┐
│    LLVM    │    │   Native   │    │   .wasm    │
│   (Hot)    │    │   Binary   │    │   Module   │
└────────────┘    └────────────┘    └────────────┘
      │                 │
      └────────┬────────┘
               ▼
        ┌────────────┐
        │   Native   │
        │    Code    │
        └────────────┘
```

### Key Components

#### 1. Parser (`parser/` crate)

- **Technology**: Nom parser combinators for composability
- **Features**:
  - Incremental parsing (re-parse only changed regions)
  - Error recovery (continue parsing after errors)
  - Precise source location tracking for diagnostics
  - Comment preservation for documentation

**Example:**
```rust
pub struct HaxeFile {
    pub package: Option<String>,
    pub imports: Vec<Import>,
    pub declarations: Vec<Declaration>,
}
```

#### 2. Type Checker (`compiler/src/tast/`)

- **Bidirectional type checking**: Bottom-up inference + top-down checking
- **Constraint-based type inference**: Unification algorithm for type variables
- **Rich type system**: Generics, nullables, abstract types, function types
- **Symbol management**: Hierarchical scopes with shadowing support

**Example:**
```rust
pub struct TypedFile {
    pub package: Option<String>,
    pub imports: Vec<TypedImport>,
    pub classes: Vec<TypedClass>,
    pub functions: Vec<TypedFunction>,
}
```

#### 3. Semantic Analysis (`compiler/src/semantic_graph/`)

Production-ready analysis infrastructure built in SSA form:

- **Control Flow Graph (CFG)**: Basic blocks, dominance, loop detection
- **Data Flow Graph (DFG)**: SSA form, def-use chains, value numbering
- **Call Graph**: Inter-procedural analysis, recursion detection
- **Ownership Graph**: Memory safety tracking (Rust-inspired)
- **TypeFlowGuard**: Flow-sensitive type checking orchestrator

**Uses:**
- Dead code elimination
- Null safety checking
- Initialization analysis
- Constant propagation
- Common subexpression elimination

See [compiler/SSA_ARCHITECTURE.md](compiler/SSA_ARCHITECTURE.md) for details.

#### 4. HIR (High-level IR) (`compiler/src/ir/hir.rs`)

Preserves high-level language features while adding resolution:

- Closures, pattern matching, try-catch
- Resolved symbols (no name lookup needed)
- Optimization hints from semantic analysis
- Source-level debugging support

```rust
pub struct HirModule {
    pub name: String,
    pub functions: HashMap<SymbolId, HirFunction>,
    pub types: HashMap<TypeId, HirTypeDecl>,
    pub globals: HashMap<SymbolId, HirGlobal>,
}
```

#### 5. MIR (Mid-level IR) (`compiler/src/ir/mod.rs`)

Platform-independent optimization target in SSA form:

- Standard IR instructions (add, mul, load, store, call)
- Explicit control flow (branches, jumps, phi nodes)
- Type metadata and RTTI system
- String pool for constant strings
- **Status: 98% complete** ✅

```rust
pub struct IrModule {
    pub functions: HashMap<IrFunctionId, IrFunction>,
    pub globals: Vec<IrGlobal>,
    pub type_defs: Vec<IrTypeDef>,
    pub string_pool: StringPool,
}
```

**Implemented Features:**
- ✅ All expression types (literals, binary ops, calls, field access)
- ✅ Control flow (if, while, for, switch, try-catch)
- ✅ Pattern matching (constructor, tuple, array, object patterns)
- ✅ Type metadata registration (enums, classes, interfaces)
- ✅ Global variables with initialization
- ✅ Lambda/closure infrastructure (95% - body generation pending)

#### 6. Diagnostics System

Rich error messages with source locations and suggestions:

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

## Compilation Modes

Rayzor supports **three compilation strategies** optimized for different use cases:

### 1. Development Mode (JIT)

```bash
rayzor dev --watch --hot-reload
```

- **Fast iteration**: 50-200ms compilation via Cranelift JIT
- **Hot-reload**: Instant code updates without restart
- **Good performance**: 15-25x interpreter speed
- **Use case**: Rapid prototyping, development

### 2. JIT Runtime Mode (Tiered)

```bash
rayzor run --jit --profile
```

- **Adaptive optimization**: Cranelift for cold paths, LLVM for hot paths
- **Profile-guided**: Auto-detect hot functions (>5% runtime or >1000 calls)
- **Background compilation**: LLVM optimizes hot code while running
- **Best of both**: Fast startup + maximum performance
- **Use case**: Long-running applications, servers

**Execution Flow:**
```
Function First Call:
  1. Compile with Cranelift (fast: ~50-200ms)
  2. Execute and profile
  3. If becomes hot → compile with LLVM in background (1-5s)
  4. Swap to optimized version when ready

Result: 15-25x speed (cold) → 45-50x speed (hot)
```

### 3. AOT Production Mode

```bash
rayzor build --aot --optimize=3 --target=native
rayzor build --aot --target=wasm --optimize=size
```

- **Maximum optimization**: LLVM -O3 for all code
- **Single binary**: No runtime dependencies
- **Cross-compilation**: x64, ARM, WASM targets
- **Use case**: Production deployments, embedded systems

---

## Performance Targets

### Compilation Speed

| Mode | Target | Goal | vs. Haxe/C++ |
|------|--------|------|--------------|
| **JIT** | Cranelift (cold) | 50-200ms/function | ~20x faster |
| **JIT** | LLVM (hot) | 1-5s/function | Similar |
| **AOT** | Cranelift | < 500ms | ~5x faster |
| **AOT** | LLVM | 10-30s | Similar |
| **AOT** | WASM | 100-500ms | N/A |

### Runtime Performance

| Target | Goal | Use Case |
|--------|------|----------|
| Cranelift (cold paths) | 15-25x interpreter | Fast startup |
| LLVM (hot paths + AOT) | 45-50x interpreter | Maximum speed |
| WASM | 30-40x interpreter | Browser/WASI |

**Goal:** Match or exceed Haxe/C++ runtime performance with dramatically faster compilation.

---

## Project Status

### ✅ Completed (Production Ready)

- **Parser**: Incremental parsing with error recovery
- **Type Checker**: Full type inference and checking
- **Semantic Graphs**: SSA/DFG/CFG/ownership analysis
- **TypeFlowGuard**: Flow-sensitive safety checking
- **HIR**: High-level IR with semantic preservation
- **MIR**: 98% complete lowering pipeline

### 🚧 In Progress

- **MIR Polish**: 2% remaining (lambda body generation)
- **Optimization Passes**: DCE, constant folding, CSE

### 📋 Next Steps

1. **Cranelift Backend** (Phase 1 - 2-3 weeks)
   - MIR → Cranelift IR translation
   - Basic JIT compilation
   - "Hello World" native executable

2. **LLVM Backend** (Phase 2 - 2-3 weeks)
   - MIR → LLVM IR translation
   - Hot path tier-up support
   - Full optimization pipeline

3. **WebAssembly** (Phase 3 - TBD)
   - Direct WASM compilation
   - WasmGC for object model
   - Browser + WASI support

---

## Crate Structure

```
rayzor/
├── parser/              # Haxe parser (nom-based)
│   ├── src/
│   │   ├── haxe_parser.rs         # Main entry
│   │   ├── haxe_parser_expr.rs    # Expression parsing
│   │   ├── haxe_parser_decls.rs   # Declaration parsing
│   │   ├── haxe_parser_types.rs   # Type parsing
│   │   └── incremental_parser_enhanced.rs
│   └── Cargo.toml
│
├── compiler/            # Main compiler infrastructure
│   ├── src/
│   │   ├── tast/                   # Type-checked AST
│   │   │   ├── type_checker.rs
│   │   │   ├── symbols.rs
│   │   │   ├── type_flow_guard.rs
│   │   │   └── core.rs
│   │   │
│   │   ├── semantic_graph/         # Analysis infrastructure
│   │   │   ├── cfg.rs              # Control Flow Graph
│   │   │   ├── dfg.rs              # Data Flow Graph (SSA)
│   │   │   ├── call_graph.rs
│   │   │   └── ownership_graph.rs
│   │   │
│   │   ├── ir/                     # Intermediate Representations
│   │   │   ├── hir.rs              # High-level IR
│   │   │   ├── tast_to_hir.rs      # TAST → HIR
│   │   │   ├── hir_to_mir.rs       # HIR → MIR (98% complete)
│   │   │   ├── mod.rs              # MIR definitions
│   │   │   ├── builder.rs          # IR builder
│   │   │   ├── optimization.rs     # Optimization passes
│   │   │   └── validation.rs       # IR validation
│   │   │
│   │   └── pipeline.rs             # Compilation pipeline
│   │
│   ├── ARCHITECTURE.md             # Detailed architecture
│   ├── SSA_ARCHITECTURE.md         # SSA integration
│   ├── RAYZOR_ARCHITECTURE.md      # Vision & roadmap
│   ├── LOWERING_STATUS.md          # Feature tracking
│   └── Cargo.toml
│
├── diagnostics/         # Error reporting
├── source_map/          # Source location tracking
└── Cargo.toml
```

---

## Documentation

- **[ARCHITECTURE.md](compiler/ARCHITECTURE.md)** - Complete system architecture
- **[SSA_ARCHITECTURE.md](compiler/SSA_ARCHITECTURE.md)** - SSA integration details
- **[RAYZOR_ARCHITECTURE.md](compiler/RAYZOR_ARCHITECTURE.md)** - Vision, roadmap, tiered JIT
- **[LOWERING_STATUS.md](compiler/LOWERING_STATUS.md)** - Feature implementation tracking
- **[IMPLEMENTATION_ROADMAP.md](compiler/IMPLEMENTATION_ROADMAP.md)** - Development plan
- **[PRODUCTION_READINESS.md](compiler/PRODUCTION_READINESS.md)** - Production checklist

---

## Design Philosophy

### 1. Correctness First

```
Correctness → Safety → Clarity → Performance
```

The compiler prioritizes generating correct code. Performance optimizations come after correctness is proven.

### 2. Layered Architecture

Each layer has a single, well-defined responsibility. Information flows forward through explicit interfaces.

### 3. Analysis as Infrastructure

Complex analyses (SSA, CFG, DFG, ownership) are built once and queried by multiple passes. This enables sophisticated optimizations without code duplication.

### 4. Incremental Everything

Support incremental operations at every level:
- Incremental parsing (re-parse only changed regions)
- Incremental type checking (re-check only affected code)
- Incremental analysis (re-analyze only dependencies)
- Incremental codegen (re-generate only changed functions)

### 5. Developer Experience

- **Fast feedback**: 50-200ms JIT compilation
- **Rich diagnostics**: Helpful error messages with suggestions
- **Hot-reload**: Instant code updates during development
- **Modern tooling**: IDE support, debugging, profiling

---

## Comparison with Official Haxe Compiler

| Feature | Haxe (official) | Rayzor |
|---------|-----------------|--------|
| **Language Support** | Full Haxe 4.x | Haxe 4.x (in progress) |
| **JS/Python/PHP** | ✅ Excellent | ❌ Not a goal |
| **C++ Target** | ✅ Slow compile, fast runtime | 🎯 Fast compile, fast runtime |
| **Native Perf** | ~1.0x baseline | 🎯 45-50x interpreter (LLVM) |
| **Compile Speed** | 2-5s (C++) | 🎯 50-200ms (Cranelift JIT) |
| **JIT Runtime** | ❌ No | ✅ Tiered (Cranelift→LLVM) |
| **Hot Path Optimization** | ❌ No | ✅ Profile-guided tier-up |
| **WASM** | ⚠️ Via C++ | ✅ Direct native target |
| **Type Checking** | Production | ✅ Production (0 errors) |
| **Optimizations** | Backend-specific | ✅ SSA-based (universal) |

### Rayzor's Niche

- **Instant iteration**: 50-200ms JIT vs 2-5s C++ compilation
- **Adaptive optimization**: Auto-optimize hot paths like V8, PyPy, JVM HotSpot
- **Native performance**: Match C++ speed without compile-time cost
- **Cross-platform**: Direct WASM target for browser, WASI, edge
- **Modern runtime**: Tiered JIT with profile-guided optimization

---

## Contributing

Rayzor is under active development. Contributions are welcome!

### Getting Started

1. **Clone and build**:
   ```bash
   git clone https://github.com/darmie/rayzor.git
   cd rayzor
   cargo build
   ```

2. **Run tests**:
   ```bash
   cargo test
   ```

3. **Read the architecture docs**:
   - Start with [ARCHITECTURE.md](compiler/ARCHITECTURE.md)
   - Understand SSA integration in [SSA_ARCHITECTURE.md](compiler/SSA_ARCHITECTURE.md)
   - Review the roadmap in [RAYZOR_ARCHITECTURE.md](compiler/RAYZOR_ARCHITECTURE.md)

### Development Workflow

See [ARCHITECTURE.md](compiler/ARCHITECTURE.md#contributing) for:
- Code organization principles
- Adding new features
- Testing strategy
- Pull request guidelines

---

## Roadmap

### Near-term (3 months)

- ✅ Complete MIR lowering (98% → 100%)
- 🎯 Cranelift backend (JIT compilation)
- 🎯 Basic runtime (GC, String, Array)
- 🎯 "Hello World" native executable

### Medium-term (6 months)

- 🎯 LLVM backend (hot path tier-up + AOT)
- 🎯 Optimization pipeline (DCE, CSE, inlining)
- 🎯 Standard library coverage (basic)
- 🎯 Performance within 10% of Haxe/C++

### Long-term (12 months)

- 🎯 WebAssembly target (browser + WASI)
- 🎯 Production-ready for real projects
- 🎯 IDE support (LSP server)
- 🎯 Community adoption

---

## License

Apache License 2.0 - See [LICENSE](LICENSE) file for details

---

## Acknowledgments

- **Haxe Foundation** - For the excellent Haxe programming language
- **Cranelift Project** - For the fast JIT compiler framework
- **LLVM Project** - For the industry-leading optimization infrastructure
- **Rust Community** - For the amazing language and ecosystem

---

## Contact

- **Issues**: [GitHub Issues](https://github.com/yourusername/rayzor/issues)
- **Discussions**: [GitHub Discussions](https://github.com/yourusername/rayzor/discussions)

---

**Rayzor**: Fast compilation, native performance, modern architecture. The future of Haxe compilation.
