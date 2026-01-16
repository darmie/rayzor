# BLADE Implementation Plan

## Overview

This plan builds on the topological import loading system (commit 0711a2d) to implement BLADE bytecode caching for fast incremental compilation.

## Phase 1: Make MIR Serializable

**Goal**: Add serde derives to all MIR types

### Files to Modify

```
compiler/src/ir/mod.rs
compiler/src/ir/types.rs
compiler/src/ir/instructions.rs
compiler/src/ir/functions.rs
compiler/src/ir/module.rs
```

### Changes Required

1. Add dependencies to `compiler/Cargo.toml`:
```toml
[dependencies]
serde = { version = "1.0", features = ["derive"] }
postcard = { version = "1.0", features = ["alloc"] }
```

2. Add derives to IR types:
```rust
// types.rs
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum IrType { ... }

// instructions.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IrInstruction { ... }

// functions.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrFunction { ... }

// module.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrModule { ... }
```

### Challenges
- `IrId` uses internal indexing that may not serialize cleanly
- Function pointers and extern references need special handling
- May need custom Serialize/Deserialize for some types

---

## Phase 2: Create BLADE Module

**Goal**: Implement save/load for compiled modules

### New File: `compiler/src/ir/blade.rs`

```rust
use serde::{Serialize, Deserialize};
use std::path::Path;

pub const BLADE_MAGIC: &[u8; 4] = b"BLAD";
pub const BLADE_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
pub struct BladeModule {
    pub magic: [u8; 4],
    pub version: u32,
    pub metadata: BladeMetadata,
    pub mir: IrModule,
}

#[derive(Serialize, Deserialize)]
pub struct BladeMetadata {
    pub module_name: String,
    pub source_path: String,
    pub source_hash: u64,           // FNV or xxHash of source
    pub compile_timestamp: u64,
    pub dependencies: Vec<String>,  // Qualified names of dependencies
    pub compiler_version: String,
}

#[derive(Debug)]
pub enum BladeError {
    InvalidMagic,
    VersionMismatch { expected: u32, found: u32 },
    SerializationError(String),
    IoError(std::io::Error),
}

pub fn save_blade(path: &Path, module: &IrModule, metadata: BladeMetadata)
    -> Result<(), BladeError>
{
    let blade = BladeModule {
        magic: *BLADE_MAGIC,
        version: BLADE_VERSION,
        metadata,
        mir: module.clone(),
    };

    let bytes = postcard::to_allocvec(&blade)
        .map_err(|e| BladeError::SerializationError(e.to_string()))?;

    std::fs::write(path, bytes)
        .map_err(BladeError::IoError)?;

    Ok(())
}

pub fn load_blade(path: &Path) -> Result<(IrModule, BladeMetadata), BladeError> {
    let bytes = std::fs::read(path)
        .map_err(BladeError::IoError)?;

    let blade: BladeModule = postcard::from_bytes(&bytes)
        .map_err(|e| BladeError::SerializationError(e.to_string()))?;

    if &blade.magic != BLADE_MAGIC {
        return Err(BladeError::InvalidMagic);
    }

    if blade.version != BLADE_VERSION {
        return Err(BladeError::VersionMismatch {
            expected: BLADE_VERSION,
            found: blade.version,
        });
    }

    Ok((blade.mir, blade.metadata))
}

/// Check if a .blade file is still valid (not stale)
pub fn is_blade_valid(blade_path: &Path, source_path: &Path) -> bool {
    // Load metadata only (fast path)
    let bytes = match std::fs::read(blade_path) {
        Ok(b) => b,
        Err(_) => return false,
    };

    let blade: BladeModule = match postcard::from_bytes(&bytes) {
        Ok(b) => b,
        Err(_) => return false,
    };

    // Check source hash
    let source = match std::fs::read_to_string(source_path) {
        Ok(s) => s,
        Err(_) => return false,
    };

    let current_hash = hash_source(&source);
    blade.metadata.source_hash == current_hash
}

fn hash_source(source: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    use std::collections::hash_map::DefaultHasher;
    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    hasher.finish()
}
```

---

## Phase 3: Pre-BLADE Stdlib with build.rs

**Goal**: Pre-compile stdlib to .blade files at build time

### New File: `compiler/build.rs`

```rust
//! Build script to pre-compile stdlib to BLADE format
//!
//! This runs at `cargo build` time and generates .blade files for
//! all stdlib modules, dramatically speeding up runtime compilation.

use std::path::{Path, PathBuf};
use std::env;

fn main() {
    // Only run in release builds or when PREBLADE_STDLIB is set
    let profile = env::var("PROFILE").unwrap_or_default();
    let force_preblade = env::var("PREBLADE_STDLIB").is_ok();

    if profile != "release" && !force_preblade {
        println!("cargo:warning=Skipping stdlib pre-BLADE (debug build)");
        return;
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let blade_dir = out_dir.join("stdlib_blade");

    std::fs::create_dir_all(&blade_dir).unwrap();

    // Get stdlib path
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let stdlib_path = manifest_dir.join("haxe-std");

    // Pre-compile each stdlib module
    preblade_stdlib(&stdlib_path, &blade_dir);

    // Tell Cargo to rerun if stdlib changes
    println!("cargo:rerun-if-changed=haxe-std");

    // Export blade directory path for runtime use
    println!("cargo:rustc-env=STDLIB_BLADE_DIR={}", blade_dir.display());
}

fn preblade_stdlib(stdlib_path: &Path, blade_dir: &Path) {
    // Core modules to pre-compile (in dependency order)
    let core_modules = [
        "haxe/Int64.hx",
        "haxe/io/Bytes.hx",
        "haxe/io/BytesBuffer.hx",
        "haxe/io/Input.hx",
        "haxe/io/Output.hx",
        "haxe/io/Eof.hx",
        "haxe/iterators/ArrayIterator.hx",
        "haxe/iterators/ArrayKeyValueIterator.hx",
        "haxe/exceptions/PosException.hx",
        "haxe/exceptions/NotImplementedException.hx",
        "sys/io/File.hx",
        "sys/io/FileInput.hx",
        "sys/io/FileOutput.hx",
        "sys/FileSystem.hx",
        "sys/FileStat.hx",
        // ... other common modules
    ];

    for module in &core_modules {
        let source_path = stdlib_path.join(module);
        if !source_path.exists() {
            continue;
        }

        let module_name = module
            .replace('/', ".")
            .replace(".hx", "");
        let blade_path = blade_dir.join(format!("{}.blade", module_name));

        // Skip if blade is newer than source
        if is_blade_current(&blade_path, &source_path) {
            continue;
        }

        println!("cargo:warning=Pre-BLADE: {}", module_name);

        // Compile to BLADE
        if let Err(e) = compile_to_blade(&source_path, &blade_path, &module_name) {
            println!("cargo:warning=Failed to pre-BLADE {}: {}", module_name, e);
        }
    }
}

fn is_blade_current(blade_path: &Path, source_path: &Path) -> bool {
    let blade_mtime = std::fs::metadata(blade_path)
        .and_then(|m| m.modified())
        .ok();
    let source_mtime = std::fs::metadata(source_path)
        .and_then(|m| m.modified())
        .ok();

    match (blade_mtime, source_mtime) {
        (Some(b), Some(s)) => b >= s,
        _ => false,
    }
}

fn compile_to_blade(source_path: &Path, blade_path: &Path, module_name: &str)
    -> Result<(), String>
{
    // This would call into the compiler library
    // For build.rs, we need to be careful about dependencies

    // Option 1: Shell out to rayzor CLI
    // let status = std::process::Command::new("cargo")
    //     .args(["run", "--", "blade", source_path, "-o", blade_path])
    //     .status();

    // Option 2: Use compiler library directly (requires careful dep management)
    // compiler::blade::compile_file_to_blade(source_path, blade_path, module_name)

    Ok(())
}
```

### Alternative: Separate Pre-BLADE Binary

Instead of build.rs (which has dependency complexities), create a separate tool:

```
compiler/
├── src/
│   └── bin/
│       └── preblade.rs    # Standalone pre-BLADE tool
```

```rust
// compiler/src/bin/preblade.rs
//! Pre-compile stdlib to BLADE format
//!
//! Usage: cargo run --bin preblade -- --stdlib-path ./haxe-std --out ./blade-cache

use compiler::compilation::{CompilationConfig, CompilationUnit};
use compiler::ir::blade::{save_blade, BladeMetadata};
use std::path::PathBuf;
use clap::Parser;

#[derive(Parser)]
struct Args {
    #[arg(long)]
    stdlib_path: PathBuf,

    #[arg(long, short)]
    out: PathBuf,

    #[arg(long)]
    force: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    std::fs::create_dir_all(&args.out)?;

    let mut unit = CompilationUnit::new(CompilationConfig::default());
    unit.load_stdlib()?;

    // Compile and save each module
    for module in discover_stdlib_modules(&args.stdlib_path) {
        let blade_path = args.out.join(format!("{}.blade", module.name));

        if !args.force && is_current(&blade_path, &module.source_path) {
            println!("  [skip] {}", module.name);
            continue;
        }

        println!("  [blade] {}", module.name);

        let mir = compile_module(&mut unit, &module)?;
        let metadata = BladeMetadata {
            module_name: module.name.clone(),
            source_path: module.source_path.to_string_lossy().to_string(),
            source_hash: hash_file(&module.source_path)?,
            compile_timestamp: now_timestamp(),
            dependencies: module.dependencies.clone(),
            compiler_version: env!("CARGO_PKG_VERSION").to_string(),
        };

        save_blade(&blade_path, &mir, metadata)?;
    }

    println!("✓ Pre-BLADE complete: {} modules", count);
    Ok(())
}
```

---

## Phase 4: Runtime BLADE Loading

**Goal**: Load pre-compiled .blade files at runtime

### Modify `CompilationUnit`

```rust
impl CompilationUnit {
    /// Try to load a module from BLADE cache
    pub fn try_load_blade(&mut self, module_name: &str) -> Option<IrModule> {
        // Check embedded stdlib blade first
        if let Some(mir) = self.load_embedded_stdlib_blade(module_name) {
            return Some(mir);
        }

        // Check user cache directory
        if let Some(cache_dir) = &self.config.blade_cache_dir {
            let blade_path = cache_dir.join(format!("{}.blade", module_name));
            if let Ok((mir, metadata)) = load_blade(&blade_path) {
                // Validate cache is current
                if is_blade_valid(&blade_path, Path::new(&metadata.source_path)) {
                    return Some(mir);
                }
            }
        }

        None
    }

    /// Load pre-compiled stdlib blade (embedded at build time)
    fn load_embedded_stdlib_blade(&self, module_name: &str) -> Option<IrModule> {
        // Use include_bytes! for embedded blades
        let blade_bytes = match module_name {
            "haxe.Int64" => include_bytes!(concat!(env!("STDLIB_BLADE_DIR"), "/haxe.Int64.blade")),
            "haxe.io.Bytes" => include_bytes!(concat!(env!("STDLIB_BLADE_DIR"), "/haxe.io.Bytes.blade")),
            // ... other modules
            _ => return None,
        };

        let blade: BladeModule = postcard::from_bytes(blade_bytes).ok()?;
        Some(blade.mir)
    }
}
```

---

## Phase 5: Integration with Topological Loading

**Goal**: Use BLADE in the efficient loading path

### Modify `load_imports_efficiently`

```rust
pub fn load_imports_efficiently(&mut self, imports: &[String]) -> Result<(), String> {
    // ... existing dependency collection ...

    // Step 3: Compile in topological order, using BLADE when available
    for name in compile_order {
        // Try BLADE cache first
        if let Some(mir) = self.try_load_blade(&name) {
            debug!("[BLADE] Loaded from cache: {}", name);
            self.mir_modules.push(Arc::new(mir));
            continue;
        }

        // Fall back to compilation
        if let Some((file_path, source, _)) = all_files.remove(&name) {
            // ... existing compilation code ...

            // Save to BLADE cache for next time
            if let Some(cache_dir) = &self.config.blade_cache_dir {
                let blade_path = cache_dir.join(format!("{}.blade", name));
                if let Err(e) = save_blade(&blade_path, &mir, metadata) {
                    debug!("Failed to cache {}: {}", name, e);
                }
            }
        }
    }

    Ok(())
}
```

---

## Implementation Order

1. **Week 1**: Phase 1 - Make MIR serializable
   - Add serde derives
   - Handle edge cases (IrId, function refs)
   - Write unit tests for serialization roundtrip

2. **Week 2**: Phase 2 - Create BLADE module
   - Implement save/load functions
   - Add validation and error handling
   - Write integration tests

3. **Week 3**: Phase 3 - Pre-BLADE tool
   - Create `preblade` binary
   - Script to pre-compile stdlib
   - CI integration to build .blade files

4. **Week 4**: Phase 4 & 5 - Runtime integration
   - Load BLADE at runtime
   - Integrate with topological loading
   - Benchmark and optimize

---

## Expected Performance

| Scenario | Current | With BLADE |
|----------|---------|------------|
| First compile (sys.io) | 766ms | 766ms (no change) |
| Second compile (cached) | 766ms | ~50ms (15x faster) |
| With pre-BLADE stdlib | 766ms | ~20ms (38x faster) |

---

## Cache Directory Structure

```
.rayzor/
└── blade/
    ├── stdlib/           # Pre-compiled stdlib (can be committed)
    │   ├── haxe.Int64.blade
    │   ├── haxe.io.Bytes.blade
    │   └── ...
    └── user/             # User code cache (gitignored)
        ├── my.app.Main.blade
        └── my.app.Utils.blade
```

---

## CLI Integration

```bash
# Compile with BLADE caching
rayzor run Main.hx --blade-cache .rayzor/blade

# Pre-compile stdlib
rayzor preblade --stdlib ./haxe-std --out .rayzor/blade/stdlib

# Clear cache
rayzor cache clear

# Show cache stats
rayzor cache stats
```

---

## Related: RayzorBundle (.rzb)

For single-file executable distribution, see:

- [RZB_FORMAT_SPEC.md](RZB_FORMAT_SPEC.md) - Format specification
- [RZB_IMPLEMENTATION_PLAN.md](RZB_IMPLEMENTATION_PLAN.md) - Implementation details

BLADE caches individual modules for incremental compilation, while RayzorBundle packages entire applications for distribution.

---

## Notes

- **postcard** format is ~10x smaller than JSON and ~5x faster to parse
- Source hashing is more reliable than timestamps for cache invalidation
- Pre-BLADE stdlib can be committed to repo for consistent CI builds
- BLADE version field allows format evolution without breaking compatibility
