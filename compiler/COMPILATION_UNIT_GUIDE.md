# Compilation Unit Guide

## Overview

The `CompilationUnit` is Rayzor's multi-file compilation infrastructure. It handles:
- Standard library discovery and loading
- Multi-file project compilation
- Package and import resolution
- Cross-file type checking and symbol resolution

## Quick Start

### Basic Usage

```rust
use compiler::compilation::{CompilationUnit, CompilationConfig};

fn main() {
    // Create compilation unit with automatic stdlib discovery
    let mut unit = CompilationUnit::new(CompilationConfig::default());

    // Load standard library
    unit.load_stdlib().expect("Failed to load stdlib");

    // Add source files
    let source = r#"
        package com.example;

        class Main {
            public static function main():Void {
                trace("Hello, World!");
            }
        }
    "#;
    unit.add_file(source, "Main.hx").expect("Failed to add file");

    // Compile to TAST
    let typed_files = unit.lower_to_tast().expect("Compilation failed");

    println!("Compiled {} files successfully!", typed_files.len());
}
```

## Standard Library Discovery

The compiler automatically discovers the Haxe standard library from multiple sources:

### Search Order

1. **HAXE_STD_PATH** environment variable (highest priority)
2. **HAXE_HOME/std** environment variable
3. Project-local directories:
   - `compiler/haxe-std`
   - `../haxe-std`
   - `./haxe-std`
4. Platform-specific standard installations:

#### Linux
- `/usr/share/haxe/std`
- `/usr/local/share/haxe/std`
- `/opt/haxe/std`

#### macOS
- `/usr/local/lib/haxe/std`
- `/opt/homebrew/lib/haxe/std` (Apple Silicon)
- `/Library/Haxe/std`
- `~/.haxe/std` (user installation)

#### Windows
- `C:\HaxeToolkit\haxe\std`
- `C:\Program Files\Haxe\std`
- `C:\Program Files (x86)\Haxe\std`
- `%APPDATA%\Haxe\std`

### Setting Custom Stdlib Path

```bash
# Use environment variable
export HAXE_STD_PATH=/path/to/haxe/std

# Or set in code
let mut config = CompilationConfig::default();
config.stdlib_paths = vec![PathBuf::from("/custom/path/to/std")];
let unit = CompilationUnit::new(config);
```

## Adding Files

### Method 1: Inline Source

```rust
let source = r#"
    package com.example;
    class User {
        public var name:String;
        public function new(name:String) {
            this.name = name;
        }
    }
"#;
unit.add_file(source, "User.hx")?;
```

### Method 2: Filesystem Path

```rust
use std::path::PathBuf;

let path = PathBuf::from("src/com/example/User.hx");
unit.add_file_from_path(&path)?;
```

### Method 3: Import Path Resolution

```rust
let source_paths = vec![PathBuf::from("src"), PathBuf::from("lib")];

// Loads from src/com/example/User.hx or lib/com/example/User.hx
unit.add_file_by_import("com.example.User", &source_paths)?;
```

### Method 4: Directory Scanning

```rust
let src_dir = PathBuf::from("src");

// Recursively load all .hx files in src/
let count = unit.add_directory(&src_dir, true)?;
println!("Loaded {} files from src/", count);
```

## Multi-File Projects

### Project Structure

```
my-project/
├── src/
│   ├── com/
│   │   └── example/
│   │       ├── model/
│   │       │   └── User.hx
│   │       ├── service/
│   │       │   └── UserService.hx
│   │       └── Main.hx
│   └── utils/
│       └── Helper.hx
└── lib/
    └── external/
        └── Library.hx
```

### Loading Multi-File Project

```rust
let mut unit = CompilationUnit::new(CompilationConfig::default());
unit.load_stdlib()?;

// Method 1: Load entire source tree
let src_count = unit.add_directory(&PathBuf::from("src"), true)?;
let lib_count = unit.add_directory(&PathBuf::from("lib"), true)?;

println!("Loaded {} source files, {} library files", src_count, lib_count);

// Method 2: Load specific files
let source_paths = vec![PathBuf::from("src"), PathBuf::from("lib")];
unit.add_file_by_import("com.example.Main", &source_paths)?;
unit.add_file_by_import("com.example.model.User", &source_paths)?;
unit.add_file_by_import("com.example.service.UserService", &source_paths)?;

// Compile
let typed_files = unit.lower_to_tast()?;
```

## Package and Import Resolution

### How It Works

1. **Package Declaration**: Each file declares its package
   ```haxe
   package com.example.model;
   class User { }
   ```

2. **Import Statements**: Files import types from other packages
   ```haxe
   package com.example.service;
   import com.example.model.User;

   class UserService {
       var users:Array<User>;
   }
   ```

3. **Symbol Resolution**: The compiler:
   - Registers all types with their full package names
   - Resolves imports to fully-qualified names
   - Checks package visibility and access control

### Symbol Namespaces

- **Stdlib symbols**: Prefixed with `haxe.*`
  - Example: `haxe.String`, `haxe.Array`

- **User symbols**: Prefixed with their package
  - Example: `com.example.model.User`, `com.example.service.UserService`

### Checking Symbols

```rust
let typed_files = unit.lower_to_tast()?;

// Find all symbols in a package
for symbol in unit.symbol_table.all_symbols() {
    if let Some(qname) = symbol.qualified_name {
        let name = unit.string_interner.get(qname).unwrap_or("");
        if name.starts_with("com.example.model.") {
            println!("Model symbol: {}", name);
        }
    }
}
```

## Configuration Options

```rust
use compiler::compilation::CompilationConfig;

let config = CompilationConfig {
    // Custom stdlib paths (overrides auto-discovery)
    stdlib_paths: vec![PathBuf::from("/custom/stdlib")],

    // Which stdlib files to load by default
    default_stdlib_imports: vec![
        "StdTypes.hx".to_string(),
        "String.hx".to_string(),
        "Array.hx".to_string(),
    ],

    // Whether to load stdlib at all
    load_stdlib: true,

    // Package prefix for stdlib symbols
    stdlib_root_package: Some("haxe".to_string()),

    // Global import.hx files (applied to all user files)
    global_import_hx_files: vec![],
};

let unit = CompilationUnit::new(config);
```

## Advanced Features

### Global import.hx Files

Load global imports that apply to all user files:

```rust
let mut config = CompilationConfig::default();
config.global_import_hx_files = vec![
    PathBuf::from("config/import.hx"),
];

let mut unit = CompilationUnit::new(config);
unit.load_stdlib()?;
unit.load_global_imports()?;  // Load global imports
// ... add user files ...
```

The global import.hx file might contain:

```haxe
// config/import.hx
import haxe.String;
import haxe.Array;
using StringTools;
```

### Custom Stdlib Files

Load only specific stdlib files:

```rust
let mut config = CompilationConfig::default();
config.default_stdlib_imports = vec![
    "StdTypes.hx".to_string(),
    "String.hx".to_string(),
    // Don't load Array, Iterator, etc.
];

let unit = CompilationUnit::new(config);
```

### Disable Stdlib Loading

For testing or minimal builds:

```rust
let mut config = CompilationConfig::default();
config.load_stdlib = false;

let unit = CompilationUnit::new(config);
// Only user files will be compiled
```

## Complete Example

Here's a complete example demonstrating all features:

```rust
use compiler::compilation::{CompilationUnit, CompilationConfig};
use std::path::PathBuf;

fn main() -> Result<(), String> {
    println!("Compiling multi-file Haxe project...\n");

    // 1. Create compilation unit with custom configuration
    let mut config = CompilationConfig::default();
    config.default_stdlib_imports = vec![
        "StdTypes.hx".to_string(),
        "String.hx".to_string(),
        "Array.hx".to_string(),
        "Iterator.hx".to_string(),
    ];

    let mut unit = CompilationUnit::new(config);

    // 2. Load standard library
    println!("Loading standard library...");
    unit.load_stdlib()?;
    println!("✓ Loaded {} stdlib files\n", unit.stdlib_files.len());

    // 3. Add source files from filesystem
    println!("Loading project files...");
    let source_paths = vec![
        PathBuf::from("src"),
        PathBuf::from("lib"),
    ];

    // Load by import path
    unit.add_file_by_import("com.example.model.User", &source_paths)?;
    unit.add_file_by_import("com.example.service.UserService", &source_paths)?;
    unit.add_file_by_import("com.example.Main", &source_paths)?;

    // Or load entire directories
    // let count = unit.add_directory(&PathBuf::from("src"), true)?;

    println!("✓ Loaded {} user files\n", unit.user_files.len());

    // 4. Compile to TAST
    println!("Compiling...");
    let typed_files = unit.lower_to_tast()?;
    println!("✓ Successfully compiled {} files\n", typed_files.len());

    // 5. Inspect results
    println!("Compilation results:");
    println!("  Total files: {}", typed_files.len());
    println!("  Stdlib files: {}", unit.stdlib_files.len());
    println!("  User files: {}", unit.user_files.len());

    // Count symbols by package
    let mut stdlib_count = 0;
    let mut user_count = 0;

    for symbol in unit.symbol_table.all_symbols() {
        if let Some(qname) = symbol.qualified_name {
            let name = unit.string_interner.get(qname).unwrap_or("");
            if name.starts_with("haxe.") {
                stdlib_count += 1;
            } else if name.starts_with("com.example.") {
                user_count += 1;
            }
        }
    }

    println!("  Stdlib symbols: {}", stdlib_count);
    println!("  User symbols: {}", user_count);

    println!("\n✓ Compilation successful!");
    Ok(())
}
```

## Troubleshooting

### Stdlib Not Found

If you see "Warning: No standard library found":

1. Install Haxe to a standard location, or
2. Set `HAXE_STD_PATH` environment variable:
   ```bash
   export HAXE_STD_PATH=/path/to/haxe/std
   ```
3. Or provide a custom path in code:
   ```rust
   config.stdlib_paths = vec![PathBuf::from("/custom/path")];
   ```

### Import Resolution Failed

If imports aren't resolving:

1. Check that files are in the correct directory structure matching their package:
   ```
   package com.example.model;  // Must be in com/example/model/
   ```

2. Ensure source paths are provided:
   ```rust
   let source_paths = vec![PathBuf::from("src")];
   unit.add_file_by_import("com.example.User", &source_paths)?;
   ```

3. Verify the file exists:
   ```rust
   if let Some(path) = unit.resolve_import_path("com.example.User", &source_paths) {
       println!("Found: {:?}", path);
   }
   ```

### Symbol Not Found

If symbols aren't resolving between files:

1. Ensure files are added in dependency order (dependencies first)
2. Check that imports are correctly spelled
3. Verify package names match directory structure
4. Use qualified names to debug:
   ```rust
   for symbol in unit.symbol_table.all_symbols() {
       if let Some(qname) = symbol.qualified_name {
           println!("{}", unit.string_interner.get(qname).unwrap_or(""));
       }
   }
   ```

## API Reference

### CompilationUnit

| Method | Description |
|--------|-------------|
| `new(config)` | Create a new compilation unit |
| `load_stdlib()` | Load standard library files |
| `load_global_imports()` | Load global import.hx files |
| `add_file(source, path)` | Add file from string source |
| `add_file_from_path(path)` | Add file from filesystem |
| `add_file_by_import(import, paths)` | Add file by import path |
| `add_directory(path, recursive)` | Add all .hx files from directory |
| `resolve_import_path(import, paths)` | Convert import to file path |
| `lower_to_tast()` | Compile all files to TAST |

### CompilationConfig

| Field | Type | Description |
|-------|------|-------------|
| `stdlib_paths` | `Vec<PathBuf>` | Paths to search for stdlib |
| `default_stdlib_imports` | `Vec<String>` | Stdlib files to load |
| `load_stdlib` | `bool` | Whether to load stdlib |
| `stdlib_root_package` | `Option<String>` | Package prefix for stdlib |
| `global_import_hx_files` | `Vec<PathBuf>` | Global import files |

| Static Method | Description |
|---------------|-------------|
| `discover_stdlib_paths()` | Auto-discover stdlib locations |

## Next Steps

- See [IMPLEMENTATION_ROADMAP.md](IMPLEMENTATION_ROADMAP.md) for upcoming features
- Check [test_multifile_compilation.rs](examples/test_multifile_compilation.rs) for examples
- Read [test_filesystem_compilation.rs](examples/test_filesystem_compilation.rs) for filesystem loading
