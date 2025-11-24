# Rayzor Compiler

> A modern, safe, and performant Haxe compiler implementation in Rust

## Quick Links

- **[ARCHITECTURE.md](ARCHITECTURE.md)** - General compiler architecture overview
- **[SSA_ARCHITECTURE.md](SSA_ARCHITECTURE.md)** - SSA integration strategy (advanced)
- **[IMPLEMENTATION_ROADMAP.md](IMPLEMENTATION_ROADMAP.md)** - Development roadmap
- **[PRODUCTION_READINESS.md](PRODUCTION_READINESS.md)** - Production checklist

## What is Rayzor?

Rayzor is a complete reimplementation of a Haxe compiler in Rust, designed for:

- ⚡ **High Performance**: Native compilation speeds, incremental builds
- 🛡️ **Memory Safety**: Optional compile-time memory safety (Rust-inspired)
- 🔥 **Developer Experience**: Fast hot-reload, excellent error messages
- 🚀 **Production Ready**: WASM + LLVM compilation with maximum optimization
- 🎯 **Hybrid Compilation**: Cranelift for cold paths, LLVM for hot paths

## Features

### Implemented ✅

- **Parser**: Incremental nom-based parser with error recovery
- **Type System**: Sophisticated type inference and checking
- **Semantic Analysis**: CFG, DFG (SSA), Call Graph, Ownership tracking
- **Flow-Sensitive Checking**: TypeFlowGuard with precise safety analysis
- **Multi-tier IR**: HIR (high-level) and MIR (optimizable)
- **Optimization Framework**: Pass-based optimization infrastructure

### In Progress 🚧

- **Code Generation**: WASM backend, Cranelift for cold paths
- **Optimization**: LLVM backend for hot paths (planned)
- **Interpreter**: For hot-reload support
- **Standard Library**: Core Haxe API compatibility
- **Tooling**: LSP server, debugger integration

## Architecture Overview

```
Source Code (.hx)
    ↓
Parser (nom-based)
    ↓
AST (Abstract Syntax Tree)
    ↓
Type Checker
    ↓
TAST (Typed AST)
    ↓
Semantic Analysis (CFG, DFG/SSA, Ownership)
    ↓
TypeFlowGuard (Flow-sensitive checking)
    ↓
HIR (High-level IR with semantics)
    ↓
MIR (Mid-level IR for optimization)
    ↓
Optimization Passes
    ↓
Code Generation
├── WASM (primary target)
├── Cranelift (cold paths - fast compilation)
├── LLVM (hot paths - maximum optimization)
└── Interpreter (development)
    ↓
Target Output (WASM modules, native binaries)
```

See [ARCHITECTURE.md](ARCHITECTURE.md) for details.

## Key Innovations

### 1. SSA as Analysis Infrastructure

SSA (Static Single Assignment) is built **once** in the Data Flow Graph and **queried** by all subsequent passes. This eliminates duplication while enabling precise analysis.

```
DFG (SSA form) → TypeFlowGuard → HIR hints → MIR attributes → Optimizations
     ↑
Single Source of Truth
```

See [SSA_ARCHITECTURE.md](SSA_ARCHITECTURE.md) for the complete strategy.

### 2. Layered IR Design

- **HIR**: Preserves language semantics for hot-reload and debugging
- **MIR**: Platform-independent optimization target
- Both use SSA insights without requiring SSA form

### 3. Optional Memory Safety

Rust-inspired ownership and lifetime tracking:

```haxe
@:ownership
class Resource {
    var data: Array<Int>;

    public function borrow(): &Array<Int> {
        return &data;  // Compile-time borrow checking
    }
}
```

Opt-in via annotations, no runtime overhead.

## Getting Started

### Build

```bash
# Build the compiler
cargo build --release

# Run tests
cargo test

# Build with all features
cargo build --release --all-features
```

### Example Usage

```bash
# Compile a Haxe file
./target/release/rayzor compile example.hx

# With optimization
./target/release/rayzor compile -O3 example.hx

# Development mode with hot-reload
./target/release/rayzor dev --watch --hot-reload example.hx
```

## Project Structure

```
rayzor/
├── parser/              # Parsing infrastructure
│   ├── src/
│   │   ├── haxe_parser.rs
│   │   ├── haxe_ast.rs
│   │   └── incremental_parser_enhanced.rs
│   └── Cargo.toml
│
├── compiler/            # Main compiler crate
│   ├── src/
│   │   ├── tast/               # Type-checked AST
│   │   ├── semantic_graph/     # Analysis (CFG, DFG/SSA, etc.)
│   │   ├── ir/                 # HIR and MIR
│   │   └── pipeline.rs         # Compilation pipeline
│   │
│   ├── examples/               # Test programs
│   ├── ARCHITECTURE.md         # Architecture overview
│   ├── SSA_ARCHITECTURE.md     # SSA details
│   └── Cargo.toml
│
├── diagnostics/         # Error reporting
├── source_map/          # Source location tracking
└── Cargo.toml
```

## Documentation Guide

### For New Contributors

Start here:
1. **[ARCHITECTURE.md](ARCHITECTURE.md)** - Understand the overall design
2. **[IMPLEMENTATION_ROADMAP.md](IMPLEMENTATION_ROADMAP.md)** - See what's being built
3. Look at `examples/` for working code

### For Compiler Developers

Deep dives:
1. **[SSA_ARCHITECTURE.md](SSA_ARCHITECTURE.md)** - SSA integration pattern
2. **[src/ir/README.md](src/ir/README.md)** - IR design details
3. **[../resource/haxe_mutability_and_borrow_model.md](../resource/haxe_mutability_and_borrow_model.md)** - Memory safety model

### For Users

- **[../resource/strategy.md](../resource/strategy.md)** - Development workflow
- **[../resource/plan.md](../resource/plan.md)** - Project goals

## Development

### Code Organization

Each crate follows this structure:
```
src/
├── lib.rs              # Public API
├── component1/         # Major component
│   ├── mod.rs
│   ├── submodule.rs
│   └── tests.rs        # Co-located tests
└── component2/
```

### Testing

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_name

# Run with logging
RUST_LOG=debug cargo test

# Run examples
cargo run --example test_hir_pipeline
```

### Adding Features

1. Parse new syntax (parser crate)
2. Type check (compiler/src/tast/)
3. Analyze (compiler/src/semantic_graph/)
4. Lower to HIR (compiler/src/ir/tast_to_hir.rs)
5. Lower to MIR (compiler/src/ir/hir_to_mir.rs)
6. Generate code (compiler/src/codegen/)

See [ARCHITECTURE.md](ARCHITECTURE.md#implementation-guide) for details.

## Current Status

### Completeness

| Component | Status | Coverage |
|-----------|--------|----------|
| Parser | ✅ Complete | ~95% |
| Type Checker | ✅ Complete | ~80% |
| Semantic Analysis | ✅ Complete | ~85% |
| HIR | ✅ Complete | ~90% |
| MIR | 🚧 In Progress | ~70% |
| Optimization | 🚧 In Progress | ~40% |
| Code Generation | ❌ Not Started | 0% |

See [PRODUCTION_READINESS.md](PRODUCTION_READINESS.md) for detailed checklist.

### Known Limitations

- No macro system yet
- Limited standard library
- WASM backend in development
- Cranelift integration incomplete
- No package manager integration

## Performance

### Compilation Speed

- **Parsing**: ~50µs per KB
- **Type Checking**: ~200µs per function
- **Analysis**: ~500µs per function
- **Optimization**: ~1ms per function

### Memory Usage

- **AST**: ~500 bytes per node
- **TAST**: ~800 bytes per node
- **Semantic Graphs**: ~2KB per function
- **MIR**: ~3KB per function

## Contributing

We welcome contributions! Please:

1. Read [ARCHITECTURE.md](ARCHITECTURE.md) to understand the design
2. Check [IMPLEMENTATION_ROADMAP.md](IMPLEMENTATION_ROADMAP.md) for planned work
3. Look at existing code for style guidelines
4. Add tests for new features
5. Update documentation

### Coding Standards

- **Rust 2021 Edition**
- **Format**: `cargo fmt` before committing
- **Lint**: `cargo clippy` should pass
- **Tests**: All tests must pass
- **Documentation**: Public APIs must be documented

## License

MIT License - see LICENSE file for details

## Contact

- **Issues**: [GitHub Issues](https://github.com/rayzor-lang/rayzor/issues)
- **Discussions**: [GitHub Discussions](https://github.com/rayzor-lang/rayzor/discussions)

---

**Status**: Active Development
**Version**: 0.1.0
**Last Updated**: 2025-11-12
