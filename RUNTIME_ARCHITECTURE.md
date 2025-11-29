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

---

## Extern Native Function Handling

Rayzor has a comprehensive system for mapping Haxe standard library methods to native runtime functions. This section documents the complete architecture.

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                    HAXE SOURCE CODE                                 │
│   extern class String { function charAt(i: Int): String; }          │
└───────────────────────────────┬─────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────────┐
│               STDLIB MAPPING (runtime_mapping.rs)                   │
│   MethodSignature { class: "String", method: "charAt" }             │
│        ↓                                                            │
│   RuntimeFunctionCall { runtime_name: "haxe_string_char_at", ... }  │
└───────────────────────────────┬─────────────────────────────────────┘
                                │
                    ┌───────────┴───────────┐
                    ▼                       ▼
┌───────────────────────────┐   ┌──────────────────────────────────┐
│   MIR WRAPPER PATH        │   │    DIRECT EXTERN PATH            │
│   (Thread, Channel, Arc)  │   │    (String, Math, File)          │
│                           │   │                                  │
│   thread.rs builds MIR:   │   │   hir_to_mir.rs generates:       │
│   Thread_spawn calls      │   │   CallDirect to extern func      │
│   rayzor_thread_spawn     │   │                                  │
└───────────────────────────┘   └──────────────────────────────────┘
                    │                       │
                    └───────────┬───────────┘
                                ▼
┌─────────────────────────────────────────────────────────────────────┐
│               CRANELIFT BACKEND (cranelift_backend.rs)              │
│   - Declares extern functions with Linkage::Import                  │
│   - Handles C ABI (ARM64 i32→i64 extension)                         │
│   - Links runtime symbols                                           │
└───────────────────────────────┬─────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────────┐
│               RUNTIME (plugin_impl.rs + *.rs)                       │
│   register_symbol!("haxe_string_char_at", haxe_sys::...)            │
│   pub extern "C" fn haxe_string_char_at(s: *const u8, i: i32)       │
└─────────────────────────────────────────────────────────────────────┘
```

### Key Components

#### 1. Stdlib Mapping (`compiler/src/stdlib/runtime_mapping.rs`)

The central registry that maps Haxe methods to runtime functions:

```rust
/// Describes how to call a runtime function
pub struct RuntimeFunctionCall {
    /// Name of the runtime function (e.g., "haxe_string_char_at")
    pub runtime_name: &'static str,

    /// Whether the function needs an output pointer as first argument
    /// True for functions that return complex types (String, Array)
    pub needs_out_param: bool,

    /// Whether the instance is passed as first argument
    pub has_self_param: bool,

    /// Number of additional parameters (not counting self or out)
    pub param_count: usize,

    /// Whether this method returns a value
    pub has_return: bool,

    /// Which parameters should be passed as raw u64 bits (no boxing)
    /// Bitmask: bit N indicates parameter N should be cast to u64
    pub raw_value_params: u32,

    /// Whether return value is raw u64 that needs cast to type parameter
    pub returns_raw_value: bool,

    /// Which parameters should be sign-extended from i32 to i64
    /// Used for IntMap keys (Haxe Int i32 → runtime i64)
    pub extend_to_i64_params: u32,
}

/// Method signature in Haxe stdlib
pub struct MethodSignature {
    pub class: &'static str,    // "String", "Array", "Math"
    pub method: &'static str,   // "charAt", "push", "sin"
    pub is_static: bool,
    pub is_constructor: bool,
}
```

**Registration Macro:**

```rust
// Instance method returning primitive
map_method!(instance "String", "charAt" => "haxe_string_char_at_ptr",
            params: 1, returns: primitive)

// Constructor returning complex type via out param
map_method!(constructor "Channel", "new" => "Channel_init",
            params: 1, returns: complex)

// Instance method with i64 extension for IntMap keys
map_method!(instance "IntMap", "get" => "haxe_intmap_get",
            params: 1, returns: raw_value, extend_i64: 0b010)
```

#### 2. Two Implementation Patterns

**Pattern A: Direct Extern Functions** (String, Math, Array, File)

Haxe method call → lookup StdlibMapping → generate `CallDirect` to C function

```
str.charAt(0)
    ↓
StdlibMapping.get("String", "charAt")
    ↓
RuntimeFunctionCall { runtime_name: "haxe_string_char_at_ptr", ... }
    ↓
MIR: CallDirect(extern "haxe_string_char_at_ptr", [str, 0])
    ↓
Cranelift: call @haxe_string_char_at_ptr(str, 0)
```

**Pattern B: MIR Wrapper Functions** (Thread, Channel, Arc, Mutex, Vec)

Used when extra logic is needed (closure extraction, type conversions):

```rust
// compiler/src/stdlib/thread.rs
fn build_thread_spawn(builder: &mut MirBuilder) {
    let func_id = builder.begin_function("Thread_spawn")
        .param("closure_obj", ptr_u8.clone())
        .returns(ptr_u8.clone())
        .build();

    // Extract function pointer from closure object (offset 0)
    let fn_ptr = builder.load(closure_obj, ptr_u8.clone());

    // Extract environment pointer (offset 8)
    let env_ptr = builder.load(env_ptr_addr, ptr_u8.clone());

    // Call runtime function with extracted pointers
    let spawn_id = builder.get_function_by_name("rayzor_thread_spawn")?;
    let handle = builder.call(spawn_id, vec![fn_ptr, env_ptr]).unwrap();

    builder.ret(Some(handle));
}
```

#### 3. MIR Lowering (`compiler/src/ir/hir_to_mir.rs`)

Key functions for extern function handling:

```rust
// Get runtime info for a stdlib method
fn get_stdlib_runtime_info(&self, method_symbol: SymbolId, receiver_type: &TypeKind)
    -> Option<(String, String, RuntimeFunctionCall)>

// Check if class uses MIR wrappers (Thread, Channel, Arc, Mutex, Vec*)
fn is_mir_wrapper_class(class_name: &str) -> bool

// Register extern function in MIR module
fn get_or_register_extern_function(&mut self, name: &str, params: Vec<IrType>, ret: IrType)
    -> IrFunctionId
```

**Parameter Type Conversions:**

```rust
// Raw value params: cast to u64 for inline storage collections
if runtime_info.raw_value_params & (1 << param_idx) != 0 {
    // Int/Float/Bool/Ptr → u64 raw bits
    arg_value = builder.bitcast(arg_value, IrType::U64);
}

// i64 extension: for IntMap keys
if runtime_info.extend_to_i64_params & (1 << param_idx) != 0 {
    // Haxe Int (i32) → runtime i64
    arg_value = builder.sextend(arg_value, IrType::I64);
}
```

#### 4. Runtime Symbol Registration (`runtime/src/plugin_impl.rs`)

```rust
/// Thread-safe function pointer wrapper
pub struct FunctionPtr(*const u8);

/// Runtime symbol for inventory-based registration
pub struct RuntimeSymbol {
    pub name: &'static str,
    pub ptr: FunctionPtr,
}

inventory::collect!(RuntimeSymbol);

/// Register a runtime symbol
macro_rules! register_symbol {
    ($name:expr, $func:path) => {
        inventory::submit! {
            RuntimeSymbol {
                name: $name,
                ptr: FunctionPtr::new($func as *const u8),
            }
        }
    };
}

// String functions
register_symbol!("haxe_string_char_at_ptr", crate::haxe_string::haxe_string_char_at_ptr);
register_symbol!("haxe_string_length", crate::haxe_string::haxe_string_length);

// Concurrency functions
register_symbol!("rayzor_thread_spawn", crate::concurrency::rayzor_thread_spawn);
register_symbol!("rayzor_channel_send", crate::concurrency::rayzor_channel_send);

// Math functions
register_symbol!("haxe_math_sin", crate::haxe_math::haxe_math_sin);
```

**Symbol Categories (~250 total):**

| Category | Count | Examples |
|----------|-------|----------|
| String | 54 | `haxe_string_char_at`, `haxe_string_concat` |
| Array | 20 | `haxe_array_push`, `haxe_array_pop` |
| Math | 22 | `haxe_math_sin`, `haxe_math_sqrt` |
| File I/O | 24 | `haxe_file_get_content`, `haxe_filesystem_exists` |
| Concurrency | 46 | `rayzor_thread_spawn`, `rayzor_channel_send` |
| Vec (monomorphized) | 70 | `rayzor_vec_i32_push`, `rayzor_vec_f64_get` |
| Collections | 18 | `haxe_stringmap_set`, `haxe_intmap_get` |

#### 5. Cranelift Backend Integration (`compiler/src/codegen/cranelift_backend.rs`)

**Function Declaration:**

```rust
fn declare_function(&mut self, mir_func_id: IrFunctionId, function: &IrFunction) {
    // Extern functions have empty CFG
    let is_extern = function.cfg.blocks.is_empty();

    let mut sig = self.module.make_signature();

    // ARM64 C ABI: extend i32/u32 to i64
    for param in &function.signature.parameters {
        let cranelift_type = if is_extern && !cfg!(target_os = "windows")
            && matches!(param.ty, IrType::I32 | IrType::U32)
        {
            types::I64  // C ABI promotion
        } else {
            self.mir_type_to_cranelift(&param.ty)?
        };
        sig.params.push(AbiParam::new(cranelift_type));
    }

    // Extern: Import linkage, MIR wrapper: Export linkage
    let linkage = if is_extern { Linkage::Import } else { Linkage::Export };
    let func_id = self.module.declare_function(&function.name, linkage, &sig)?;
}
```

**Call Site Code Generation:**

```rust
IrInstruction::CallDirect { dest, func_id, args, .. } => {
    if let Some(extern_func) = mir_module.extern_functions.get(func_id) {
        let mut arg_values = Vec::new();

        for (i, &arg_reg) in args.iter().enumerate() {
            let mut cl_value = *value_map.get(&arg_reg)?;

            // ARM64 C ABI: extend i32/u32 to i64
            if !cfg!(target_os = "windows") {
                match extern_func.signature.parameters.get(i).map(|p| &p.ty) {
                    Some(IrType::I32) => {
                        cl_value = builder.ins().sextend(types::I64, cl_value);
                    }
                    Some(IrType::U32) => {
                        cl_value = builder.ins().uextend(types::I64, cl_value);
                    }
                    _ => {}
                }
            }
            arg_values.push(cl_value);
        }

        let call_inst = builder.ins().call(func_ref, &arg_values);
    }
}
```

### C ABI Handling

**ARM64 Integer Promotion:**

On ARM64 (except Windows), the C ABI requires i32/u32 parameters to be promoted to i64:

```text
Haxe: str.charAt(0)        // 0 is Int (i32)
                ↓
MIR:  CallDirect("haxe_string_char_at_ptr", [str, 0:i32])
                ↓
Cranelift (ARM64):
  v1 = iconst.i32 0
  v2 = sextend.i64 v1      // Promote i32 → i64
  call @haxe_string_char_at_ptr(str, v2)
```

**Consistency Requirements:**

Both declaration and call site must apply the same promotion:

- **Declaration**: Signature uses `i64` for promoted params
- **Call site**: Values are `sextend`/`uextend` before call

### Monomorphization for Generic Types

`Vec<T>` is specialized to avoid runtime type dispatch:

```text
Vec<Int>   → VecI32  → rayzor_vec_i32_push, rayzor_vec_i32_get
Vec<Float> → VecF64  → rayzor_vec_f64_push, rayzor_vec_f64_get
Vec<Bool>  → VecBool → rayzor_vec_bool_push, rayzor_vec_bool_get
Vec<T*>    → VecPtr  → rayzor_vec_ptr_push, rayzor_vec_ptr_get
```

The compiler detects the type parameter and dispatches to the correct specialization:

```rust
fn get_monomorphized_vec_class(&self, element_type: &TypeKind) -> Option<&'static str> {
    match element_type {
        TypeKind::Int | TypeKind::I32 => Some("VecI32"),
        TypeKind::Float | TypeKind::F64 => Some("VecF64"),
        TypeKind::Bool => Some("VecBool"),
        TypeKind::Class(_) | TypeKind::Instance(_) => Some("VecPtr"),
        _ => None,
    }
}
```

### File Reference Summary

| File | Purpose | Lines |
|------|---------|-------|
| `compiler/src/stdlib/runtime_mapping.rs` | Central stdlib → runtime mapping registry | ~1,472 |
| `compiler/src/ir/hir_to_mir.rs` | MIR lowering with extern function handling | ~9,393 |
| `compiler/src/stdlib/thread.rs` | MIR wrapper for Thread | ~242 |
| `compiler/src/stdlib/channel.rs` | MIR wrapper for Channel | ~300 |
| `compiler/src/stdlib/sync.rs` | MIR wrapper for Arc/Mutex | ~400 |
| `compiler/src/codegen/cranelift_backend.rs` | JIT compilation with C ABI handling | ~2,803 |
| `runtime/src/plugin_impl.rs` | Runtime symbol registration | ~539 |
| `runtime/src/concurrency.rs` | Thread/Arc/Mutex/Channel implementations | ~600 |
| `runtime/src/haxe_string.rs` | String runtime functions | ~500 |
| `runtime/src/haxe_array.rs` | Array runtime functions | ~400 |

### Adding a New Extern Function

To add a new extern function:

**Step 1: Declare in Haxe stdlib** (`compiler/haxe-std/`)

```haxe
extern class MyClass {
    public function myMethod(x: Int): String;
}
```

**Step 2: Register mapping** (`compiler/src/stdlib/runtime_mapping.rs`)

```rust
fn register_myclass_methods(&mut self) {
    let (sig, call) = map_method!(instance "MyClass", "myMethod"
        => "haxe_myclass_my_method", params: 1, returns: primitive);
    self.register(sig, call);
}
```

**Step 3: Implement in runtime** (`runtime/src/haxe_myclass.rs`)

```rust
#[no_mangle]
pub unsafe extern "C" fn haxe_myclass_my_method(
    self_ptr: *const u8,
    x: i64,  // i32 promoted to i64 on ARM64
) -> *const u8 {
    // Implementation
}
```

**Step 4: Register symbol** (`runtime/src/plugin_impl.rs`)

```rust
register_symbol!("haxe_myclass_my_method", crate::haxe_myclass::haxe_myclass_my_method);
```

### Testing Extern Functions

```bash
# Run all stdlib e2e tests
cargo run --example test_rayzor_stdlib_e2e

# Test specific functionality
cargo run --example test_rayzor_bytes
cargo run --example test_haxe_io_bytes
```

Current test coverage: 8/8 e2e tests passing (Thread, Channel, Mutex, Arc, Array, ForIn).
