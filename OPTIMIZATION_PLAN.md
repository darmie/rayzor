# Rayzor Performance Optimization Plan: Beat HashLink

## Goal

Beat HashLink performance across all benchmarks:
- **HashLink performance**: ~2-5x slower than C++
- **Target**: < 2x slower than C++ (beat HashLink by 20-50%)

## Current State

| Component | Current Performance | Target |
|-----------|---------------------|--------|
| **Interpreter** | ~10-50x slower than C++ | < 5x (match HashLink) |
| **Cranelift JIT** | ~2-5x slower than C++ | < 2x (beat HashLink) |
| **Bundle Startup** | ~360µs | < 200µs |

---

## Phase 1: Interpreter Optimizations (Highest Impact)

### 1.1 NaN Boxing (Priority: CRITICAL)

**Current**: `InterpValue` enum with heap allocation for every value
```rust
pub enum InterpValue {
    Void, Bool(bool), I8(i8), I16(i16), I32(i32), I64(i64),
    F32(f32), F64(f64), Ptr(usize), Null, String(String),
    Array(Vec<InterpValue>), Struct(Vec<InterpValue>), ...
}
```

**Problem**:
- Every value is boxed in an enum (16-32 bytes)
- Heavy cloning on register operations
- Poor cache locality

**Solution**: NaN Boxing (like Lua 5.x, LuaJIT, HashLink)
```rust
/// All values fit in 64 bits using NaN boxing
///
/// IEEE 754 double: [sign:1][exponent:11][mantissa:52]
///
/// When exponent = 0x7FF (all 1s) and mantissa != 0, it's NaN.
/// We use the mantissa bits to encode other types:
///
/// Layout:
///   [0x7FFC_0000_0000_0000 | tag:4 | payload:48]
///
/// Tags:
///   0x0 = Pointer (48-bit pointer in payload)
///   0x1 = Integer (32-bit int + flags)
///   0x2 = Boolean (0 or 1)
///   0x3 = Null
///   0x4 = String pointer
///   0x5 = Array pointer
///   0x6 = Function ID
///
/// Regular doubles are stored as-is (not NaN-boxed)
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct NanBoxedValue(u64);

impl NanBoxedValue {
    const NAN_TAG: u64 = 0x7FFC_0000_0000_0000;
    const TAG_MASK: u64 = 0x000F_0000_0000_0000;
    const PAYLOAD_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;

    // Tag values (in bits 48-51)
    const TAG_PTR: u64 = 0x0000_0000_0000_0000;
    const TAG_INT: u64 = 0x0001_0000_0000_0000;
    const TAG_BOOL: u64 = 0x0002_0000_0000_0000;
    const TAG_NULL: u64 = 0x0003_0000_0000_0000;

    #[inline(always)]
    pub fn from_f64(v: f64) -> Self {
        Self(v.to_bits())
    }

    #[inline(always)]
    pub fn from_i32(v: i32) -> Self {
        Self(Self::NAN_TAG | Self::TAG_INT | (v as u32 as u64))
    }

    #[inline(always)]
    pub fn from_bool(v: bool) -> Self {
        Self(Self::NAN_TAG | Self::TAG_BOOL | (v as u64))
    }

    #[inline(always)]
    pub fn from_ptr(ptr: *const u8) -> Self {
        Self(Self::NAN_TAG | Self::TAG_PTR | (ptr as u64 & Self::PAYLOAD_MASK))
    }

    #[inline(always)]
    pub fn is_double(&self) -> bool {
        // If not a NaN, it's a double
        (self.0 & 0x7FF0_0000_0000_0000) != 0x7FF0_0000_0000_0000
    }

    #[inline(always)]
    pub fn as_f64(&self) -> f64 {
        f64::from_bits(self.0)
    }

    #[inline(always)]
    pub fn as_i32(&self) -> i32 {
        (self.0 & 0xFFFF_FFFF) as i32
    }
}
```

**Expected Impact**: **3-5x interpreter speedup**
- No heap allocation for primitives
- 8 bytes per value instead of 16-32
- Copy semantics (no Clone overhead)
- Better cache locality

**Files to modify**:
- `compiler/src/codegen/mir_interpreter.rs` - Replace InterpValue with NanBoxedValue

---

### 1.2 Computed Goto Dispatch (Priority: HIGH)

**Current**: Pattern matching on instruction type
```rust
match instr {
    IrInstruction::Const { .. } => { ... }
    IrInstruction::BinOp { .. } => { ... }
    // 50+ instruction types
}
```

**Problem**: Branch prediction misses on instruction dispatch

**Solution**: Computed goto via function pointer table
```rust
type InstrHandler = fn(&mut MirInterpreter, &IrInstruction) -> Result<(), InterpError>;

static DISPATCH_TABLE: [InstrHandler; 64] = [
    handle_const,
    handle_binop,
    handle_load,
    // ...
];

fn execute_block(&mut self, block: &IrBasicBlock) -> Result<(), InterpError> {
    for instr in &block.instructions {
        let opcode = instr.opcode() as usize;
        // Direct jump, no branch prediction needed
        DISPATCH_TABLE[opcode](self, instr)?;
    }
    Ok(())
}
```

**Expected Impact**: **15-30% improvement** on instruction-heavy code

**Alternative**: Threaded code (pre-decode instructions into bytecode)
```rust
// Pre-decode MIR to compact bytecode
struct DecodedInstr {
    handler: InstrHandler,
    dest: u16,
    src1: u16,
    src2: u16,
}
```

---

### 1.3 Specialized Binary Operations (Priority: MEDIUM)

**Current**: Generic binary ops with runtime type checks
```rust
fn eval_binary_op(&self, op: BinaryOp, left: InterpValue, right: InterpValue) {
    match op {
        BinaryOp::Add => {
            let l = left.to_i64()?;  // Type conversion
            let r = right.to_i64()?;
            Ok(InterpValue::I64(l.wrapping_add(r)))
        }
        // ...
    }
}
```

**Solution**: Type-specialized fast paths
```rust
#[inline(always)]
fn add_i64_i64(left: NanBoxedValue, right: NanBoxedValue) -> NanBoxedValue {
    // Fast path: both are i64
    NanBoxedValue::from_i64(left.as_i64().wrapping_add(right.as_i64()))
}

fn eval_binary_op(&self, op: BinaryOp, left: NanBoxedValue, right: NanBoxedValue) -> NanBoxedValue {
    // Check types once, then dispatch to specialized handler
    if left.is_i64() && right.is_i64() {
        match op {
            BinaryOp::Add => add_i64_i64(left, right),
            BinaryOp::Sub => sub_i64_i64(left, right),
            // ...
        }
    } else {
        self.eval_binary_op_slow(op, left, right)
    }
}
```

**Expected Impact**: **10-20% improvement** on arithmetic-heavy code

---

## Phase 2: JIT Optimizations (Cranelift)

### 2.1 Better Cranelift Settings (Priority: HIGH)

**Current**: Default Cranelift settings
```rust
pub fn cranelift_opt_level(&self) -> &'static str {
    match self {
        OptimizationTier::Baseline => "none",
        OptimizationTier::Standard => "speed",
        OptimizationTier::Optimized => "speed_and_size",
        // ...
    }
}
```

**Solution**: Enable all available optimizations
```rust
fn configure_cranelift_module(&self, tier: OptimizationTier) -> JITBuilder {
    let mut flag_builder = settings::builder();

    // Common settings
    flag_builder.set("use_colocated_libcalls", "true").unwrap();
    flag_builder.set("is_pic", "false").unwrap();  // Faster non-PIC code

    match tier {
        OptimizationTier::Baseline => {
            flag_builder.set("opt_level", "none").unwrap();
        }
        OptimizationTier::Standard => {
            flag_builder.set("opt_level", "speed").unwrap();
            flag_builder.set("enable_simd", "true").unwrap();
            flag_builder.set("enable_verifier", "false").unwrap();  // Faster
        }
        OptimizationTier::Optimized => {
            flag_builder.set("opt_level", "speed").unwrap();
            flag_builder.set("enable_simd", "true").unwrap();
            flag_builder.set("enable_alias_analysis", "true").unwrap();
            flag_builder.set("enable_verifier", "false").unwrap();
        }
    }

    // Target-specific settings
    #[cfg(target_arch = "aarch64")]
    {
        flag_builder.set("enable_atomics", "true").unwrap();
    }

    // ...
}
```

**Expected Impact**: **10-20% improvement** with minimal effort

---

### 2.2 Inlining Hot Functions (Priority: HIGH)

**Current**: No function inlining in Cranelift

**Solution**: Mark functions for inlining based on profiling data
```rust
impl TieredBackend {
    fn should_inline(&self, func_id: IrFunctionId) -> bool {
        let call_count = self.profile_data.get_call_count(func_id);
        let func = self.get_function(func_id);

        // Inline if:
        // 1. Called frequently (hot)
        // 2. Small function (< 50 instructions)
        // 3. Not recursive
        call_count > 100 && func.instruction_count() < 50 && !func.is_recursive()
    }

    fn compile_with_inlining(&mut self, func_id: IrFunctionId) -> Result<*const u8, String> {
        let func = self.get_function(func_id);

        // Clone function and inline callees
        let mut inlined_func = func.clone();
        for call_site in func.call_sites() {
            if self.should_inline(call_site.callee) {
                inlined_func.inline_call(call_site);
            }
        }

        self.cranelift.compile(&inlined_func)
    }
}
```

**Expected Impact**: **20-40% improvement** for call-heavy code

---

### 2.3 Inline Caching for Polymorphic Calls (Priority: MEDIUM)

**Problem**: Virtual method calls require lookup each time

**Solution**: Monomorphic/polymorphic inline caches
```rust
/// Inline cache entry for method calls
struct InlineCache {
    /// Expected receiver type
    type_id: TypeId,
    /// Cached method pointer
    method_ptr: *const u8,
    /// Hit count
    hits: u32,
}

/// Inline cache check generated by Cranelift
fn ic_check(receiver: *const Object, cache: &mut InlineCache) -> *const u8 {
    let type_id = (*receiver).type_id;
    if type_id == cache.type_id {
        cache.hits += 1;
        return cache.method_ptr;
    }
    // Cache miss - slow path
    ic_miss(receiver, cache)
}
```

**Expected Impact**: **15-30% improvement** for OOP-heavy code

---

## Phase 3: Tiered Compilation Optimizations

### 3.1 Parallel Background JIT (Priority: HIGH)

**Current**: Single background thread for JIT compilation
```rust
// Current: One function at a time
fn background_worker(&mut self) {
    loop {
        if let Some(func_id) = self.optimization_queue.pop() {
            self.compile_function(func_id);  // Blocking
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}
```

**Solution**: Thread pool for parallel compilation
```rust
use rayon::prelude::*;

fn background_worker(&mut self) {
    loop {
        // Drain queue and compile in parallel
        let batch: Vec<_> = self.optimization_queue.drain(..).collect();

        if !batch.is_empty() {
            batch.par_iter().for_each(|func_id| {
                if let Ok(code) = self.compile_function(*func_id) {
                    self.pending_installs.push((*func_id, code));
                }
            });

            // Install compiled functions (must be serial for safety)
            for (func_id, code) in self.pending_installs.drain(..) {
                self.install_compiled_function(func_id, code);
            }
        }

        std::thread::park_timeout(Duration::from_millis(10));
    }
}
```

**Expected Impact**: **2-4x faster tier-up** on multi-core systems

---

### 3.2 Adaptive Thresholds (Priority: MEDIUM)

**Current**: Fixed thresholds for tier promotion
```rust
pub struct ProfileConfig {
    pub interpreter_threshold: u64,  // 10
    pub warm_threshold: u64,         // 100
    pub hot_threshold: u64,          // 1000
}
```

**Solution**: Adaptive thresholds based on program behavior
```rust
pub struct AdaptiveProfileConfig {
    base_thresholds: ProfileConfig,

    /// Time since last tier-up
    last_tierup: Instant,

    /// Recent call frequency
    call_rate: f64,
}

impl AdaptiveProfileConfig {
    fn get_interpreter_threshold(&self, func_id: IrFunctionId) -> u64 {
        let base = self.base_thresholds.interpreter_threshold;

        // If we're in a hot loop, tier up faster
        if self.call_rate > 1000.0 {
            base / 2
        } else if self.call_rate > 100.0 {
            base
        } else {
            // Cold start - wait longer
            base * 2
        }
    }
}
```

**Expected Impact**: **5-15% improvement** in time to optimal performance

---

## Phase 4: Memory & Runtime Optimizations

### 4.1 String Interning ✅ ALREADY EXISTS

**Location**: [compiler/src/tast/string_intern.rs](compiler/src/tast/string_intern.rs)

We already have a complete `StringInterner` implementation with:
- Arena-based storage via `TypedArena<u8>`
- FxHash for fast hashing (same as rustc)
- O(1) comparison via ID comparison (`InternedString(u32)`)
- Thread-safe concurrent interning
- Deduplication of identical strings
- Reverse lookup for debugging

**Integration needed**: Use `StringInterner` in the MIR interpreter:
```rust
// In MirInterpreter
struct MirInterpreter {
    string_interner: StringInterner,
    // ...
}

// In NanBoxedValue, store InternedString ID instead of String
const TAG_STRING: u64 = 0x0004_0000_0000_0000;

impl NanBoxedValue {
    pub fn from_interned_string(id: InternedString) -> Self {
        Self(Self::NAN_TAG | Self::TAG_STRING | (id.as_raw() as u64))
    }
}
```

**Expected Impact**: **10-20% improvement** for string-heavy code

---

### 4.2 Arena Allocation ✅ ALREADY EXISTS

**Location**: [compiler/src/tast/type_arena.rs](compiler/src/tast/type_arena.rs)

We already have a complete `TypedArena<T>` implementation with:
- Bump pointer allocation (O(1))
- Batch deallocation (drop entire arena at once)
- Thread-safe design (Mutex + AtomicUsize)
- Configurable chunk growth (`ArenaConfig`)
- Stats tracking (`ArenaStats`)

**Integration needed**: Use `TypedArena` for interpreter heap allocations:
```rust
// In MirInterpreter
struct MirInterpreter {
    object_arena: TypedArena<HeapObject>,
    array_arena: TypedArena<ArrayHeader>,
    // ...
}
```

**Expected Impact**: **5-15% improvement** for allocation-heavy code

---

## Implementation Roadmap

### Week 1-2: Quick Wins
- [ ] Enable better Cranelift optimization flags
- [ ] Add SIMD support where available
- [ ] Implement parallel background JIT

### Week 3-4: NaN Boxing
- [ ] Design NanBoxedValue type
- [ ] Update RegisterFile to use NanBoxedValue
- [ ] Update all binary/unary operations
- [ ] Benchmark and tune

### Week 5-6: Dispatch Optimization
- [ ] Implement computed goto dispatch
- [ ] Add instruction pre-decoding
- [ ] Benchmark different dispatch strategies

### Week 7-8: JIT Improvements
- [ ] Implement function inlining pass
- [ ] Add inline caching for method calls
- [ ] Profile and optimize hot paths

### Week 9-10: Polish & Benchmarks
- [ ] Run full Haxe benchmark suite
- [ ] Compare against HashLink
- [ ] Document performance characteristics
- [ ] Create performance regression tests

---

## Success Metrics

| Benchmark | HashLink | Target (Rayzor) | Status |
|-----------|----------|-----------------|--------|
| Mandelbrot | 178ms | < 150ms | Pending |
| N-Body | 245ms | < 200ms | Pending |
| Allocation | 890ms | < 750ms | Pending |
| SHA256 | 156ms | < 130ms | Pending |

---

## References

- [LuaJIT NaN Boxing](http://lua-users.org/wiki/NaNBox)
- [HashLink VM Design](https://hashlink.haxe.org/)
- [Cranelift Optimization Guide](https://cranelift.dev/)
- [V8 Inline Caching](https://v8.dev/blog/inline-caches)
