# Rayzor Runtime Architecture

## Overview

Rayzor uses a **pure Rust runtime library** (`rayzor-runtime`) for memory management and runtime support. This works for both **JIT** and **AOT** compilation without any C dependencies.

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                    Haxe Source Code                          │
└────────────────────┬────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│                  Parser → AST → TAST                         │
└────────────────────┬────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│                   MIR (Mid-level IR)                         │
│  - Calls: malloc(size), realloc(ptr, size), free(ptr)       │
│  - These are declared in stdlib/memory.rs                   │
└────────────────┬─────────────────┬──────────────────────────┘
                 │                 │
        ┌────────┴────────┐       └─────────────┐
        │                 │                     │
        ▼                 ▼                     ▼
┌──────────────┐  ┌──────────────┐    ┌──────────────┐
│   Cranelift  │  │     LLVM     │    │   Bytecode   │
│   Backend    │  │   Backend    │    │   Backend    │
└──────┬───────┘  └──────┬───────┘    └──────┬───────┘
       │                 │                    │
       │                 │                    │
       ▼                 ▼                    ▼
   ┌───────┐        ┌────────┐          ┌─────────┐
   │  JIT  │        │  AOT   │          │  BLADE  │
   └───┬───┘        └───┬────┘          │  Cache  │
       │                │               └─────────┘
       │                │
       ▼                ▼
  ┌────────────────────────────────┐
  │    rayzor-runtime Library      │
  │  ┌──────────────────────────┐  │
  │  │ rayzor_malloc(size)      │  │
  │  │ rayzor_realloc(ptr,size) │  │
  │  │ rayzor_free(ptr, size)   │  │
  │  └──────────────────────────┘  │
  │                                │
  │  Uses: std::alloc::Global      │
  │  Pure Rust, no libc            │
  └────────────────────────────────┘
```

## Components

### 1. MIR Standard Library (`compiler/src/stdlib/`)

Declares memory management functions with MIR bodies:

```rust
// stdlib/memory.rs
fn build_heap_alloc(builder: &mut MirBuilder) {
    let func_id = builder.begin_function("malloc")
        .param("size", u64_ty)
        .returns(ptr_u8_ty)
        .calling_convention(CallingConvention::Haxe)
        .build();

    // Uses Alloca temporarily - backend replaces with call to rayzor_malloc
    let ptr = builder.alloc(u8_ty, Some(size));
    builder.ret(Some(ptr));
}
```

### 2. Runtime Library (`runtime/`)

Pure Rust implementations using `std::alloc`:

```rust
// runtime/src/lib.rs
#[no_mangle]
pub unsafe extern "C" fn rayzor_malloc(size: u64) -> *mut u8 {
    let layout = Layout::from_size_align(size as usize, 1).ok()?;
    let ptr = alloc::alloc(layout);
    ptr
}

#[no_mangle]
pub unsafe extern "C" fn rayzor_realloc(ptr: *mut u8, old_size: u64, new_size: u64) -> *mut u8 {
    let old_layout = Layout::from_size_align(old_size as usize, 1).ok()?;
    let new_ptr = alloc::realloc(ptr, old_layout, new_size as usize);
    new_ptr
}

#[no_mangle]
pub unsafe extern "C" fn rayzor_free(ptr: *mut u8, size: u64) {
    let layout = Layout::from_size_align(size as usize, 1).ok()?;
    alloc::dealloc(ptr, layout);
}
```

### 3. Cranelift Backend (`compiler/src/codegen/cranelift_backend.rs`)

Recognizes MIR memory functions and emits calls to runtime:

```rust
// When lowering MIR Call to "malloc":
fn lower_call(&mut self, call: &IrCall) -> Value {
    match call.function_name {
        "malloc" => {
            // Emit call to rayzor_malloc in runtime
            let sig = self.runtime_signature("rayzor_malloc");
            let func_ref = self.import_runtime_function("rayzor_malloc", sig);
            self.builder.ins().call(func_ref, &args)
        }
        // Similar for realloc, free
    }
}
```

### 4. LLVM Backend (`compiler/src/codegen/llvm_backend.rs`)

Similar to Cranelift - emits calls to runtime functions:

```llvm
; Generated LLVM IR
declare i8* @rayzor_malloc(i64)
declare i8* @rayzor_realloc(i8*, i64, i64)
declare void @rayzor_free(i8*, i64)

define i8* @vec_u8_new() {
  %ptr = call i8* @rayzor_malloc(i64 16)
  ret i8* %ptr
}
```

## Compilation Modes

### JIT Mode

1. **Compile MIR** → Cranelift IR
2. **Link runtime**: Load `rayzor-runtime` as dynamic library or link statically
3. **Resolve symbols**: JIT linker finds `rayzor_malloc`, `rayzor_realloc`, `rayzor_free`
4. **Execute**: JIT calls runtime functions directly

```rust
// In JIT:
let mut jit = CraneliftJIT::new();
jit.load_runtime_library("rayzor-runtime"); // Links runtime
jit.compile_module(&mir_module);
jit.execute("main"); // Calls use rayzor_malloc internally
```

### AOT Mode

1. **Compile MIR** → LLVM IR
2. **Compile runtime**: Build `rayzor-runtime` as static library (.a) or object (.o)
3. **Link together**: Use system linker to combine generated code + runtime
4. **Output**: Single executable with embedded runtime

```bash
# AOT compilation:
rayzor compile main.hx --output main --mode aot

# Internally:
# 1. main.hx → MIR → LLVM IR → main.o
# 2. Compile rayzor-runtime → librayzor_runtime.a
# 3. ld main.o librayzor_runtime.a -o main
```

## Memory Management Flow

### Example: Vec<u8> Push

```
Haxe Code:
  var v = new Vec<u8>();
  v.push(42);

     ↓ (Lower to MIR)

MIR:
  %0 = call malloc(16)           // malloc returns *u8
  %1 = struct { %0, 0, 16 }      // Vec<u8> { ptr, len, cap }

     ↓ (Backend recognizes "malloc")

Cranelift/LLVM:
  call rayzor_malloc(16)         // Emit call to runtime

     ↓ (Runtime)

rayzor-runtime:
  unsafe fn rayzor_malloc(16) {
    let layout = Layout::from_size_align(16, 1);
    alloc::alloc(layout)         // Uses Rust's allocator
  }

     ↓ (Rust allocator)

std::alloc::Global:
  - On Linux: calls jemalloc or system malloc
  - On macOS: calls system malloc
  - On Windows: calls HeapAlloc
  - All through Rust's safe abstractions
```

## Benefits

### 1. Pure Rust, No C Dependencies
- ✅ No `libc` linking required
- ✅ Portable across platforms
- ✅ Uses Rust's battle-tested allocator

### 2. Works for JIT and AOT
- ✅ JIT: Runtime linked into process
- ✅ AOT: Runtime compiled with binary
- ✅ Single codebase for both

### 3. Type-Safe
- ✅ Rust's allocator is memory-safe
- ✅ No manual pointer arithmetic in runtime
- ✅ Layout calculations checked at runtime

### 4. Efficient
- ✅ Direct calls to allocator (no indirection)
- ✅ Inlineable in optimized builds
- ✅ Uses platform-specific optimized allocators

### 5. Extensible
- ✅ Easy to add GC support later
- ✅ Can swap allocators (arena, pool, etc.)
- ✅ Runtime can provide other services (reflection, etc.)

## Size Tracking

Notice that `rayzor_free` requires the size parameter:

```rust
rayzor_free(ptr: *mut u8, size: u64)
```

This is necessary because:
1. Rust's `dealloc` requires the layout (size + alignment)
2. More efficient than storing size in a header
3. Vec<u8> already tracks its capacity

### Alternative: Size Headers

If we want to hide size from callers:

```rust
rayzor_malloc(size) {
    let total = size + 8; // +8 for size header
    let ptr = alloc(total);
    *(ptr as *mut u64) = size; // Store size
    return ptr + 8; // Return after header
}

rayzor_free(ptr) {
    let header = ptr - 8;
    let size = *(header as *u64);
    dealloc(header, size + 8);
}
```

But current approach is more efficient since Vec already tracks capacity.

## Testing

### Runtime Unit Tests

```bash
cd runtime
cargo test
```

Tests include:
- ✅ Basic allocation/deallocation
- ✅ Reallocation with data preservation
- ✅ Zero-size handling
- ✅ Null pointer handling

### Integration Tests

```bash
cd compiler
cargo run --example test_vec_u8_operations
```

Tests Vec<u8> through full pipeline:
- ✅ MIR generation
- ✅ Validation
- ✅ Cranelift lowering
- ✅ JIT execution with runtime

## Future Enhancements

### 1. Garbage Collection
Add GC support alongside manual memory management:

```rust
rayzor_gc_alloc(size) -> *u8  // GC-managed allocation
rayzor_gc_collect()           // Force collection
```

### 2. Arena Allocators
For faster batch allocations:

```rust
rayzor_arena_create() -> ArenaId
rayzor_arena_alloc(arena, size) -> *u8
rayzor_arena_destroy(arena)  // Free all at once
```

### 3. Custom Allocators
Per-thread or per-module allocators:

```rust
rayzor_set_allocator(thread_id, allocator)
```

### 4. Memory Profiling
Runtime memory tracking:

```rust
rayzor_get_memory_stats() -> MemoryStats
```

## Platform Support

The runtime works on all platforms Rust supports:

| Platform | Allocator Used | Status |
|----------|----------------|--------|
| Linux | jemalloc/system | ✅ |
| macOS | system malloc | ✅ |
| Windows | HeapAlloc | ✅ |
| WASM | wasm-heap | 🚧 Future |
| Embedded | custom/no_std | 🚧 Future |

## Conclusion

Rayzor uses a **pure Rust runtime library** for memory management that:

- ✅ Works for both JIT and AOT
- ✅ No C dependencies
- ✅ Type-safe and efficient
- ✅ Portable and extensible
- ✅ Uses Rust's proven allocator

This architecture provides the foundation for Vec<u8>, String, and all future dynamic data structures.
