# RayzorBundle Implementation Plan

## Overview

RayzorBundle (.rzb) builds on the BLADE caching infrastructure to provide single-file executable distribution. While BLADE caches individual modules for incremental compilation, RayzorBundle packages an entire application for instant startup.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         Compilation Pipeline                             │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  Source Code (.hx)                                                       │
│       │                                                                  │
│       ▼                                                                  │
│  ┌─────────────┐     ┌─────────────┐     ┌─────────────┐                │
│  │   Parser    │ ──▶ │    TAST     │ ──▶ │     MIR     │                │
│  └─────────────┘     └─────────────┘     └─────────────┘                │
│                                                 │                        │
│                           ┌─────────────────────┼─────────────────────┐  │
│                           │                     │                     │  │
│                           ▼                     ▼                     ▼  │
│                    ┌─────────────┐       ┌─────────────┐       ┌──────┐ │
│                    │   .blade    │       │    .rzb     │       │ JIT  │ │
│                    │   (cache)   │       │  (bundle)   │       │      │ │
│                    └─────────────┘       └─────────────┘       └──────┘ │
│                           │                     │                     │  │
│                           └─────────────────────┼─────────────────────┘  │
│                                                 │                        │
│                                                 ▼                        │
│                                          ┌─────────────┐                 │
│                                          │ Interpreter │                 │
│                                          │   or JIT    │                 │
│                                          └─────────────┘                 │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

## Implementation Status

### Completed

- [x] RayzorBundle struct definition
- [x] Bundle serialization with postcard
- [x] `save_bundle()` and `load_bundle()` functions
- [x] `load_bundle_from_bytes()` for embedded bundles
- [x] Entry module/function tracking
- [x] Module table with metadata
- [x] Build info embedding
- [x] `preblade --bundle` CLI tool
- [x] Bundle loading test example
- [x] Interpreter execution from bundle

### Pending

- [ ] Compression support (zstd)
- [ ] Bundle signing
- [ ] Lazy module loading
- [ ] Stdlib bundling option
- [ ] CLI `rayzor run app.rzb` support

---

## Files Created/Modified

### New Files

| File | Purpose |
|------|---------|
| `compiler/src/ir/blade.rs` | RayzorBundle struct and save/load functions |
| `compiler/src/bin/preblade.rs` | Bundle creation tool |
| `compiler/examples/test_bundle_loading.rs` | Bundle loading test |

### Modified Files

| File | Changes |
|------|---------|
| `compiler/src/ir/mod.rs` | Export RayzorBundle types |
| `compiler/src/codegen/mir_interpreter.rs` | Added trace handlers for bundle execution |

---

## Key Implementation Details

### Bundle Creation (preblade.rs)

```rust
fn create_bundle(input: &str, output: &str) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Compile source to MIR
    let mut unit = CompilationUnit::new(CompilationConfig::default());
    unit.load_stdlib()?;
    unit.add_file(&source, input)?;
    unit.lower_to_tast()?;

    // 2. Collect all MIR modules
    let mir_modules = unit.get_mir_modules();
    let modules: Vec<IrModule> = mir_modules
        .iter()
        .map(|m| (**m).clone())
        .collect();

    // 3. Create bundle
    let bundle = RayzorBundle::new(modules, "main", "main");

    // 4. Save to disk
    save_bundle(output, &bundle)?;

    Ok(())
}
```

### Bundle Loading (blade.rs)

```rust
pub fn load_bundle(path: impl AsRef<Path>) -> Result<RayzorBundle, BladeError> {
    let bytes = std::fs::read(path.as_ref())
        .map_err(BladeError::IoError)?;

    load_bundle_from_bytes(&bytes)
}

pub fn load_bundle_from_bytes(bytes: &[u8]) -> Result<RayzorBundle, BladeError> {
    // Validate magic number first (fast rejection)
    if bytes.len() < 4 || &bytes[0..4] != BUNDLE_MAGIC {
        return Err(BladeError::InvalidMagic);
    }

    // Deserialize with postcard
    let bundle: RayzorBundle = postcard::from_bytes(bytes)
        .map_err(|e| BladeError::SerializationError(e.to_string()))?;

    // Validate version
    if bundle.version != BUNDLE_VERSION {
        return Err(BladeError::VersionMismatch {
            expected: BUNDLE_VERSION,
            found: bundle.version,
        });
    }

    Ok(bundle)
}
```

### Bundle Execution (test_bundle_loading.rs)

```rust
fn execute_bundle_interpreted(bundle: &RayzorBundle) -> Result<Duration, String> {
    // 1. Get runtime symbols for FFI
    let plugin = rayzor_runtime::plugin_impl::get_plugin();
    let symbols = plugin.runtime_symbols();

    // 2. Create interpreter backend
    let config = TieredConfig {
        start_interpreted: true,
        ..Default::default()
    };
    let mut backend = TieredBackend::with_symbols(config, &symbols)?;

    // 3. Get entry module
    let entry_module = bundle.entry_module()
        .ok_or("No entry module")?;

    // 4. Find main function
    let main_func_id = entry_module.functions.iter()
        .find(|(_, f)| f.name.ends_with("_main"))
        .map(|(id, _)| *id)
        .ok_or("Main not found")?;

    // 5. Load and execute
    backend.compile_module(entry_module.clone())?;
    backend.execute_function(main_func_id, vec![])?;

    Ok(elapsed)
}
```

---

## CLI Usage

### Creating Bundles

```bash
# Basic bundle creation
cargo run --release --package compiler --bin preblade -- \
    --bundle output.rzb \
    input.hx

# With verbose output
cargo run --release --package compiler --bin preblade -- \
    --bundle output.rzb \
    --verbose \
    input.hx

# Future: Include stdlib in bundle
cargo run --release --package compiler --bin preblade -- \
    --bundle output.rzb \
    --include-stdlib \
    input.hx
```

### Running Bundles

```bash
# Current: Via test example
cargo run --release --package compiler --example test_bundle_loading -- app.rzb

# Future: Direct CLI support
rayzor run app.rzb
rayzor run app.rzb --interpreter  # Force interpreter mode
rayzor run app.rzb --jit          # Force JIT compilation
```

### Inspecting Bundles

```bash
# Future: Bundle inspection tool
rayzor bundle info app.rzb

# Output:
#   Bundle: app.rzb
#   Version: 1
#   Size: 22.94 KB
#   Modules: 1
#     - main (135 functions)
#   Entry: main::main
#   Built: 2024-01-15 10:30:00
#   Compiler: rayzor 0.1.0
```

---

## Performance Benchmarks

### Comprehensive Benchmark Results

```
╔════════════════════════════════════════════════════════════════╗
║                         SUMMARY                                ║
╠════════════════════════════════════════════════════════════════╣
║ Full Compile + JIT     │    20.59ms │   1.0x │
║ Full Compile + Interp  │     6.03ms │   3.4x │
║ Fast Compile + Interp  │     3.77ms │   5.5x │
║ Bundle + Interp        │   596.58µs │  34.5x │
╚════════════════════════════════════════════════════════════════╝
```

### Execution Mode Comparison

| Mode | Median Time | Speedup |
|------|-------------|---------|
| Full Compile + JIT | 20.59ms | 1.0x (baseline) |
| Full Compile + Interpreter | 6.03ms | 3.4x |
| Fast Compile + Interpreter | 3.77ms | 5.5x |
| **Bundle + Interpreter** | **596.58µs** | **34.5x** |

### Detailed Breakdown

**Full Compilation Phases:**

| Phase | Time |
|-------|------|
| stdlib load | 2.26ms |
| parse | 670µs |
| TAST lowering | 2.39ms |
| MIR generation | 208ns |
| **Total** | **5.36ms** |

**Bundle Load Phases:**

| Phase | Time |
|-------|------|
| file read | 275µs |
| backend setup | 136µs |
| module load | 128µs |
| **Total** | **539µs** |

**Key Insight**: Bundle loading is **10x faster** than full compilation, enabling instant startup.

Note: Interpreter execution is slower than JIT but startup is instant. For hot functions, the tiered system automatically promotes to JIT.

---

## Integration with Tiered Compilation

```
┌─────────────────────────────────────────────────────────────────────────┐
│                     5-Phase Tiered Compilation                          │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  .rzb Bundle Load     │ Instant startup (~500µs)                       │
│          │            │ No compilation needed                           │
│          ▼                                                              │
│  Phase 0: Interpreter │ Instant execution (~1-5x native)               │
│          │            │ Direct MIR interpretation                       │
│          ▼ (after N calls)                                              │
│  Phase 1: Cranelift   │ ~14ms compile, ~15x native                     │
│          │            │                                                 │
│          ▼ (warm)                                                       │
│  Phase 2: Cranelift+  │ ~20ms compile, ~25x native                     │
│          │            │                                                 │
│          ▼ (hot)                                                        │
│  Phase 3: LLVM        │ ~1-5s compile, ~50x native                     │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

The RayzorBundle format enables:
1. **Skip compilation entirely** - Load pre-compiled MIR directly
2. **Start in interpreter** - Instant execution, no JIT warmup
3. **Gradual optimization** - Hot functions JIT-compiled on demand

---

## Future Enhancements

### 1. Compression (Priority: High)

```rust
pub struct BundleFlags {
    pub compressed: bool,  // Use zstd compression
    // ...
}

// Compressed save
pub fn save_bundle_compressed(path: &Path, bundle: &RayzorBundle) -> Result<()> {
    let bytes = postcard::to_allocvec(bundle)?;
    let compressed = zstd::encode_all(&bytes[..], 3)?;  // Level 3
    std::fs::write(path, compressed)?;
    Ok(())
}
```

Expected: 50-70% size reduction.

### 2. Lazy Module Loading (Priority: Medium)

```rust
impl RayzorBundle {
    /// Load only module table, defer module loading
    pub fn load_lazy(path: &Path) -> Result<LazyBundle> {
        // Read header and module table only
        // Load modules on demand via get_module()
    }
}
```

### 3. Stdlib Bundling (Priority: Medium)

```bash
# Create bundle with stdlib included
preblade --bundle app.rzb --include-stdlib Main.hx

# Creates self-contained executable with no external dependencies
```

### 4. Bundle Signing (Priority: Low)

```rust
pub struct BundleFlags {
    pub signed: bool,
    // ...
}

pub struct SignedBundle {
    bundle: RayzorBundle,
    signature: [u8; 64],  // Ed25519 signature
}
```

### 5. Incremental Bundle Updates (Priority: Low)

```bash
# Create patch for changed modules only
preblade patch app.rzb --update Main.hx

# Apply patch to existing bundle
preblade apply app.rzb app.patch
```

---

## Testing

### Unit Tests

```bash
# Test bundle serialization roundtrip
cargo test --package compiler bundle_roundtrip

# Test bundle validation
cargo test --package compiler bundle_validation
```

### Integration Tests

```bash
# Test bundle creation and execution
cargo run --release --package compiler --example test_bundle_loading -- /tmp/test.rzb

# Test with different source files
for f in examples/*.hx; do
    preblade --bundle /tmp/test.rzb "$f"
    cargo run --example test_bundle_loading -- /tmp/test.rzb
done
```

### Benchmarks

```bash
# Compare bundle vs compilation performance
cargo run --release --package compiler --example test_bundle_loading -- app.rzb

# Expected output:
#   Bundle load: ~500µs
#   Full compilation: ~7ms
#   Speedup: ~13x
```

---

## Error Handling

### Common Errors

| Error | Cause | Solution |
|-------|-------|----------|
| `InvalidMagic` | Not a .rzb file | Check file path/format |
| `VersionMismatch` | Old bundle format | Rebuild with current compiler |
| `SerializationError` | Corrupted bundle | Rebuild bundle |
| `EntryModuleNotFound` | Missing main module | Check entry module name |

### Debugging

```bash
# Enable debug logging
RUST_LOG=debug cargo run --example test_bundle_loading -- app.rzb

# Check bundle contents (future)
rayzor bundle dump app.rzb --format json
```

---

## Migration from Source Distribution

### Before (Source Distribution)

```
my-app/
├── src/
│   ├── Main.hx
│   ├── Utils.hx
│   └── Config.hx
├── haxe-std/         # Must include stdlib!
└── README.md
```

**Deployment**: Ship entire directory, compile on target

### After (Bundle Distribution)

```
my-app/
├── app.rzb           # Single file, pre-compiled
└── README.md
```

**Deployment**: Ship single .rzb file, instant startup

---

## Comparison with HashLink

| Feature | RayzorBundle (.rzb) | HashLink (.hl) |
|---------|---------------------|----------------|
| Format | postcard binary | Custom binary |
| Compression | Optional (zstd) | None |
| Size | ~20 KB (hello world) | ~15 KB |
| Load Time | ~500µs | ~1-2ms |
| Execution | MIR interpreter | HL VM |
| JIT | Cranelift/LLVM | None |
| Debugging | Source maps (future) | Debug info |

RayzorBundle achieves similar goals to HashLink's .hl format but with the advantage of tiered JIT compilation for hot functions.
