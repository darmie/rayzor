# Rayzor Compiler Architecture

**Goal:** High-performance native compilation for Haxe, rivaling C++ targets

---

## Vision

Rayzor is a **next-generation Haxe compiler** focused on:

1. **Native Performance** - Match or exceed C++ compilation speed and runtime performance
2. **Fast Compilation** - Leverage Cranelift for rapid JIT/AOT compilation
3. **Cross-Platform Deployment** - Generate WASM modules for universal compatibility
4. **Modern Optimization** - Advanced SSA-based optimizations via semantic graphs

**Not a goal:** Language transpilation (JavaScript, Python, etc.) - the official Haxe compiler already excels at this.

**Target competitors:**
- Haxe/C++ target (slow compilation, good runtime)
- Haxe/JVM and Haxe/C# targets

---

## Compilation Pipeline

```
┌─────────────┐
│ Haxe Source │
└──────┬──────┘
       │
       ▼
┌─────────────┐
│   Parser    │ ◄─── parser/ crate
└──────┬──────┘
       │
       ▼
┌─────────────┐
│    AST      │
└──────┬──────┘
       │
       ▼
┌─────────────┐
│    TAST     │ ◄─── Type tables, symbols
│ (Typed AST) │
└──────┬──────┘
       │
       ▼
┌──────────────────┐
│  Type Checking   │ ◄─── TypeFlowGuard (diagnostics)
│    + Flow        │      semantic_graph (SSA/DFG/CFG)
│    Analysis      │
└────────┬─────────┘
         │
         ▼
┌─────────────┐
│     HIR     │ ◄─── High-level IR (preserves language semantics)
│  (High IR)  │      - Closures, for-in loops, try-catch
└──────┬──────┘      - Pattern matching, string interpolation
       │
       ▼
┌─────────────┐
│     MIR     │ ◄─── Mid-level IR (SSA form, optimizable)
│   (Mid IR)  │      - Phi nodes, basic blocks
└──────┬──────┘      - Type metadata, global init
       │              [YOU ARE HERE - 98% Complete]
       │
       ├──────────────┬──────────────┬──────────────┐
       │              │              │
       │ (JIT Mode)   │ (AOT Mode)   │ (WASM Mode)
       │              │              │
       ▼              ▼              ▼
┌────────────┐ ┌────────────┐ ┌────────────┐
│ Cranelift  │ │    LLVM    │ │  WebAsm    │
│  (Cold)    │ │  (AOT All) │ │            │
└─────┬──────┘ └─────┬──────┘ └─────┬──────┘
      │              │              │
      │ (tier-up)    │              │
      ▼              ▼              ▼
┌────────────┐ ┌────────────┐ ┌────────────┐
│    LLVM    │ │ Native ARM │ │   .wasm    │
│   (Hot)    │ │  x64, etc  │ │   Module   │
└────────────┘ └────────────┘ └────────────┘
      │              │
      └──────┬───────┘
             ▼
      ┌────────────┐
      │   Native   │
      │    Code    │
      └────────────┘
```

---

## Target Backends

### 1. Cranelift (Primary JIT Target - Phase 1)

**Why Cranelift?**
- Extremely fast compilation (10-100x faster than LLVM)
- Low latency JIT compilation
- Modern SSA-based design
- Rust ecosystem integration
- Used by Wasmtime, Spidermonkey

**Use Cases:**
- **JIT cold paths** - First execution of functions (compile in ~50-200ms)
- **Development builds** - Fast iteration with instant feedback
- **Interactive REPL** - Immediate code execution
- **Testing mode** - Fast compilation for test runs

**Timeline:** Next immediate step after MIR completion

**Performance Target:**
- Compilation: 50-200ms per function
- Runtime: 15-25x interpreter speed

### 2. LLVM (Hot Path Optimizer - Phase 2)

**Why LLVM?**
- Industry-leading optimizations
- Maximum performance for hot code
- Multiple architecture support (x64, ARM, RISC-V)
- Profile-guided optimization (PGO)
- Link-time optimization (LTO)

**Use Cases:**

**A. JIT Hot Paths (Tier-up Strategy):**
- Functions executed frequently (>5% runtime or >1000 calls)
- Recompile hot code with LLVM while running
- Replace Cranelift-compiled code with optimized version
- Profile-guided optimization based on runtime data

**B. AOT Production Builds:**
- Native binaries for deployment
- Maximum optimization for all code
- Embedded systems
- Server deployments

**Timeline:** After Cranelift backend stabilizes

**Performance Target:**
- Compilation: 1-5s per hot function (JIT), 10-30s full AOT
- Runtime: 45-50x interpreter speed, match or exceed Haxe/C++ performance

### 3. WebAssembly (Cross-Platform Target - Phase 3)

**Why WASM?**
- Universal deployment (browser, WASI, edge)
- Near-native performance
- Portable bytecode
- WasmGC for Haxe object model
- Growing ecosystem (WASI, component model)

**Use Cases:**

- Web applications
- Serverless functions
- Cross-platform distribution
- Embedded & IoT devices

**Timeline:** After LLVM backend

**Performance Target:** 30-40x interpreter speed, compact binary format

---

## Tiered JIT Compilation Strategy

Rayzor uses a **multi-tier JIT approach** for optimal performance across different execution patterns:

```
┌─────────────────────────────────────────────────────────────┐
│                   JIT Execution Flow                         │
└─────────────────────────────────────────────────────────────┘

Function First Call:
  1. Compile with Cranelift (fast: ~50-200ms)
  2. Execute Cranelift-compiled code
  3. Profile execution (call count, runtime %)

Function Becomes Hot (>5% runtime OR >1000 calls):
  1. Mark function as hot candidate
  2. Compile with LLVM in background thread (optimized: 1-5s)
  3. Continue executing Cranelift version
  4. Swap to LLVM version when ready

Subsequent Calls:
  1. Execute LLVM-optimized version (maximum performance)
```

### Compilation Modes

**Development Mode:**
```
Source → MIR → Cranelift JIT → Execute
                (fast compile, good performance)
```

**JIT Runtime Mode:**
```
Source → MIR → Cranelift (cold paths) ─┐
                                        ├→ Execute
             → LLVM (hot paths) ────────┘
                (tier-up on profiling)
```

**AOT Production Mode:**
```
Source → MIR → Optimize → LLVM → Native Binary
                (all code maximally optimized)
```

### Performance Trade-offs

| Mode | Compile Time | Runtime Speed | Use Case |
|------|--------------|---------------|----------|
| Cranelift JIT | 50-200ms | 15-25x | Cold paths, dev mode |
| LLVM JIT | 1-5s | 45-50x | Hot paths (tier-up) |
| LLVM AOT | 10-30s | 45-50x | Production binaries |
| WASM AOT | 100-500ms | 30-40x | Cross-platform |

---

## Key Architectural Decisions

### 1. SSA-Based MIR

**Decision:** Use Static Single Assignment form in MIR

**Benefits:**
- Enables powerful optimizations (DCE, constant folding, CSE)
- Natural fit for Cranelift/LLVM
- Simplified dataflow analysis
- Better register allocation

**Implementation:** `semantic_graph` module provides production-ready SSA

### 2. Three-Level IR

**Decision:** AST → HIR → MIR → Backend IR (not AST → Backend)

**Benefits:**
- HIR preserves high-level semantics for better error messages
- MIR optimizes in platform-independent way
- Multiple backends share optimization pipeline
- Clear separation of concerns

**Trade-off:** More passes, but each pass is simpler

### 3. Separate Analysis Systems

**Decision:** TypeFlowGuard (diagnostics) + semantic_graph (SSA)

**Benefits:**
- User-facing errors don't depend on optimization internals
- SSA graph optimized for compiler, not error messages
- TypeFlowGuard can be improved independently

**Location:**
- `compiler/src/tast/type_flow_guard.rs` - Developer diagnostics
- `compiler/src/semantic_graph/` - Compiler-internal SSA/DFG/CFG

### 4. Metadata-Driven Codegen

**Decision:** Store runtime type info in MIR

**Benefits:**
- Pattern matching knows enum discriminants
- Reflection/RTTI available
- GC can traverse object graphs
- Enables devirtualization

**Implementation:** `IrTypeDef` system in `modules.rs`

---

## Performance Targets

### Compilation Speed

| Mode | Target | Goal | Official Haxe |
|------|--------|------|---------------|
| **JIT** | Cranelift (cold paths) | 50-200ms/function | N/A |
| **JIT** | LLVM (hot paths) | 1-5s/function | N/A |
| **AOT** | Cranelift | < 500ms | 2-5 seconds (C++) |
| **AOT** | LLVM | 10-30s | 2-5 seconds (C++) |
| **AOT** | WASM | 100-500ms | N/A |

### Runtime Performance

| Target | Goal | Comparison |
|--------|------|------------|
| Cranelift (cold paths) | 15-25x interpreter | Fast startup, good performance |
| LLVM (hot paths + AOT) | 45-50x interpreter | Maximum performance |
| WASM | 30-40x interpreter | Near-native in browser/WASI |

### Binary Size

| Target | Goal |
|--------|------|
| Cranelift | 500KB - 2MB |
| LLVM | 300KB - 1MB (with LTO) |
| WASM | 200KB - 800KB |

---

## Optimization Strategy

### MIR-Level Optimizations (Platform-Independent)

Already have SSA infrastructure from `semantic_graph`:

1. **Dead Code Elimination (DCE)**
   - Remove unreachable blocks
   - Eliminate unused values

2. **Constant Propagation & Folding**
   - Evaluate compile-time constants
   - Simplify expressions

3. **Common Subexpression Elimination (CSE)**
   - Reuse computed values
   - Reduce redundant operations

4. **Inlining**
   - Inline small functions
   - Devirtualize interface calls when possible

5. **Escape Analysis**
   - Stack-allocate non-escaping objects
   - Reduce GC pressure

### Backend-Specific Optimizations

**Cranelift:**
- Fast register allocation
- Peephole optimizations
- Branch prediction hints

**LLVM:**
- Full optimization pipeline (-O2/-O3)
- Profile-guided optimization (PGO)
- Link-time optimization (LTO)
- Auto-vectorization

**WASM:**
- Binaryen optimization passes
- WasmGC object layout
- Bulk memory operations

---

## Memory Management

### Garbage Collection Strategy

**Phase 1:** Conservative GC (Boehm GC)
- Easy integration
- Proven performance
- No language changes needed

**Phase 2:** Precise GC
- Stack maps from MIR
- Generational collection
- Better performance for long-running apps

**Phase 3:** Reference Counting (optional)
- Deterministic cleanup
- Better for resource management
- Cycle collection for circular refs

### Object Layout

```
Haxe Object → Native Representation

class Point {
  var x: Int;
  var y: Int;
}

Native Layout (64-bit):
┌────────────┐
│ GC Header  │ 8 bytes (type info, mark bits)
├────────────┤
│ vtable ptr │ 8 bytes (for virtual dispatch)
├────────────┤
│ x: i32     │ 4 bytes
├────────────┤
│ y: i32     │ 4 bytes
├────────────┤
│ padding    │ 4 bytes (align to 8)
└────────────┘
Total: 32 bytes
```

---

## Standard Library Integration

### Native Runtime

**Core Types:**
- String (UTF-8 or UTF-16)
- Array<T> (dynamic array)
- Map<K,V> (hash map)
- Class/Interface infrastructure

**Platform Abstraction:**
- File I/O
- Network sockets
- Threading
- Process management

**Math & Utilities:**
- Math functions
- Regex (via PCRE or Rust regex)
- Date/Time
- Random

### FFI (Foreign Function Interface)

**Support for:**
- C libraries (via cranelift-ffi or LLVM)
- Rust crates (direct integration)
- System libraries (libc, etc.)

**Strategy:**
- Automatic C header binding generation
- Type-safe wrappers
- Zero-copy where possible

---

## Development Roadmap

### ✅ Completed (99%)
- Parser (Haxe syntax)
- Type checker (full type inference)
- HIR lowering (language semantics)
- MIR lowering (SSA form)
- Type metadata system
- Pattern matching
- Exception handling
- Global variables
- **Abstract types with operator overloading**
  - ✅ Binary operators (11/11 complete)
  - ⏳ Unary operators (not yet implemented)
  - ⏳ Array access operators (not yet implemented)

### 🔄 Current Phase: Abstract Types Polish (1% remaining)
- Unary operator overloading (~1 hour)
- Array access operators (~2 hours)
- Constructor expression bug fix (~2-3 hours)
- Total remaining: ~5-6 hours

### 📋 Next: Cranelift Backend (Phase 1)
**Timeline:** 2-3 weeks
- MIR → Cranelift IR translation
- Basic JIT compilation
- Simple runtime (malloc/free, no GC)
- "Hello World" native executable

### 📋 Then: Optimization Pipeline
**Timeline:** 1-2 weeks
- Implement DCE, constant folding, CSE
- Leverage existing SSA infrastructure
- Inline small functions

### 📋 Then: GC Integration
**Timeline:** 1-2 weeks
- Integrate Boehm GC
- Implement core runtime (String, Array, etc.)
- Run simple Haxe programs

### 📋 Then: LLVM Backend (Phase 2)
**Timeline:** 2-3 weeks
- MIR → LLVM IR translation
- Enable full optimization pipeline
- Cross-compilation support

### 📋 Future: WASM Backend (Phase 3)
**Timeline:** TBD
- WASM target with WasmGC
- Browser, WASI, and edge deployment

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

**Rayzor's Niche:**
- **Instant iteration**: JIT compilation in 50-200ms vs 2-5s C++ compile
- **Adaptive optimization**: Auto-optimize hot code paths with LLVM
- **Native performance**: Match C++ speed without the compile-time cost
- **Modern runtime**: Tiered JIT like V8, PyPy, JVM HotSpot
- **Cross-platform**: Direct WASM target for browser, WASI, edge
- **Developer experience**: Fast feedback loop + production performance

---

## Technical Advantages

### 1. Modern Rust Implementation
- Memory safety
- Fearless concurrency
- Rich ecosystem (Cranelift, LLVM bindings)
- Fast compile times for compiler itself

### 2. SSA from the Start
- Official Haxe compiler doesn't use SSA
- Enables optimizations not possible in source-to-source transpilation
- Better register allocation, inlining decisions

### 3. Unified Optimization Pipeline
- All backends benefit from same MIR optimizations
- Official Haxe relies on C++/LLVM optimizing generated code
- Rayzor optimizes Haxe semantics directly

### 4. Fast Iteration Cycles
- Cranelift JIT for instant feedback
- No C++ compilation wait
- Better developer experience

---

## Challenges & Mitigations

### Challenge 1: Language Coverage
**Issue:** Haxe is a large language with many features

**Mitigation:**
- Focus on core language first (95% of real code)
- Macro system can wait (most Haxe code doesn't need it)
- Incremental implementation

### Challenge 2: Standard Library
**Issue:** Haxe has extensive standard library

**Mitigation:**
- Start with subset (String, Array, Map)
- FFI to existing C libraries
- Community can contribute bindings

### Challenge 3: Debugging Experience
**Issue:** Native code harder to debug than JS

**Mitigation:**
- DWARF debug info from Cranelift/LLVM
- Source maps
- GDB/LLDB integration

### Challenge 4: Cross-Compilation
**Issue:** Supporting multiple architectures

**Mitigation:**
- Cranelift supports x64 and ARM64 out of box
- LLVM supports everything
- CI/CD for testing on all platforms

---

## Success Metrics

### Short-term (3 months)
- ✅ Compile "Hello World" to native executable via Cranelift
- ✅ 10+ core runtime functions working
- ✅ Pass 50% of official Haxe test suite

### Medium-term (6 months)
- ✅ Full standard library coverage (basic)
- ✅ LLVM backend working
- ✅ Compilation speed < 500ms for medium projects
- ✅ Runtime performance within 10% of C++ target

### Long-term (12 months)
- ✅ Production-ready for real projects
- ✅ WASM target working
- ✅ Community adoption
- ✅ Cross-platform deployment tools

---

## Contributing

See `IMPLEMENTATION_ROADMAP.md` for current status and next tasks.

**Key areas needing work:**
1. Cranelift backend (MIR → Cranelift IR)
2. Runtime library (String, Array, etc.)
3. Garbage collector integration
4. Standard library bindings

---

## References

- **Cranelift:** https://cranelift.dev/
- **LLVM:** https://llvm.org/
- **Haxe:** https://haxe.org/
- **WebAssembly:** https://webassembly.org/
- **WASI:** https://wasi.dev/

---

**Rayzor Vision:** The fastest path from Haxe source to optimized native code, with compilation speed that doesn't compromise runtime performance.
