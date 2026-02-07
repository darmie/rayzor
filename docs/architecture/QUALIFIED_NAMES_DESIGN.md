# Qualified Name System Design

## Problem Statement

Currently, the compiler uses `InternedString` for function names throughout the pipeline, which creates several issues:

1. **Ambiguity**: Functions with the same name in different classes/packages are indistinguishable
2. **Inefficient Lookups**: String matching (`contains("compute")`) instead of direct ID lookup
3. **Non-Determinism**: HashMap iteration order causes test flakiness
4. **Poor Debugging**: Function names display as "InternedString(98)" instead of human-readable names

## Current State

### Symbol System (TAST)
- ✅ `qualified_name: Option<InternedString>` field exists in `Symbol` struct
- ❌ Always set to `None` - never populated
- ✅ `package_id: Option<PackageId>` field exists
- ✅ `scope_id: ScopeId` tracks lexical scope

### HIR/MIR
- Uses `name: String` for functions
- No package/class context preserved
- Function lookup by iterator position or string matching

## Proposed Solution

### 1. Qualified Path Structure

```rust
/// A fully-qualified path to a symbol (e.g., "com.example.MyClass.myMethod")
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct QualifiedPath {
    /// Package path (e.g., ["com", "example"])
    pub package: Vec<InternedString>,

    /// Class/module path (e.g., ["MyClass", "InnerClass"])
    pub class_path: Vec<InternedString>,

    /// Symbol name (e.g., "myMethod")
    pub name: InternedString,

    /// Cached full path for display and hashing
    full_path: InternedString,
}

impl QualifiedPath {
    /// Create from package, class, and symbol name
    pub fn new(
        package: Vec<InternedString>,
        class_path: Vec<InternedString>,
        name: InternedString,
        interner: &mut StringInterner
    ) -> Self;

    /// Get the full qualified name (e.g., "com.example.MyClass.myMethod")
    pub fn full_name(&self) -> InternedString;

    /// Get just the local name
    pub fn local_name(&self) -> InternedString;

    /// Check if this path is in a given package
    pub fn in_package(&self, package: &[InternedString]) -> bool;

    /// Check if this path is in a given class
    pub fn in_class(&self, class: &[InternedString]) -> bool;
}
```

### 2. Symbol Table Enhancement

```rust
impl SymbolTable {
    /// Build qualified name for a symbol
    fn build_qualified_name(&mut self, symbol_id: SymbolId) -> QualifiedPath {
        // Walk up scope chain to build package.class.name
        // Use existing package_id and scope_id
    }

    /// Lookup symbol by qualified path
    pub fn resolve_path(&self, path: &QualifiedPath) -> Option<SymbolId>;

    /// Get all symbols in a package
    pub fn symbols_in_package(&self, package: &[InternedString]) -> Vec<SymbolId>;

    /// Get all methods in a class
    pub fn class_methods(&self, class_symbol: SymbolId) -> Vec<SymbolId>;
}
```

### 3. HIR/MIR Integration

```rust
pub struct HirFunction {
    pub symbol_id: SymbolId,
    pub qualified_path: QualifiedPath,  // NEW
    pub name: InternedString,            // Keep for local use
    // ...
}

pub struct IrFunction {
    pub symbol_id: SymbolId,
    pub qualified_path: QualifiedPath,  // NEW
    pub name: String,                    // Keep for display
    // ...
}
```

### 4. Function Resolution

Instead of:
```rust
// ❌ Bad: Non-deterministic, fragile
let func = module.functions.values().nth(2)?;
let func = module.functions.values().find(|f| f.name.contains("compute"))?;
```

Use:
```rust
// ✅ Good: Direct lookup by qualified path
let path = QualifiedPath::parse("test.Math.compute", interner)?;
let func = module.get_function_by_path(&path)?;

// Or by symbol ID (even better)
let func = module.get_function(symbol_id)?;
```

## Implementation Plan

### Phase 1: Populate qualified_name in Symbol (Week 1)
1. ✅ Field already exists
2. Implement `build_qualified_name()` in SymbolTable
3. Populate during symbol creation in ast_lowering
4. Add `resolve_path()` lookup method

### Phase 2: Add QualifiedPath to HIR (Week 2)
1. Add `qualified_path` field to `HirFunction`, `HirClass`, etc.
2. Preserve from TAST during lowering
3. Update HIR→MIR to use qualified paths
4. Add `IrModule::get_function_by_path()`

### Phase 3: Add QualifiedPath to MIR (Week 3)
1. Add to `IrFunction`
2. Use for function lookups in call resolution
3. Update Cranelift backend to use paths for debugging/profiling
4. Display improvements

### Phase 4: Enhanced Function Registry (Week 4)
1. Create `FunctionRegistry` with:
   - Path-based lookup
   - Package/class filtering
   - Method overload resolution
2. Replace HashMap iteration with registry queries
3. Add caching for common lookups

## Benefits

### Correctness
- ✅ No name collisions between packages/classes
- ✅ Deterministic function resolution
- ✅ Proper scoping and visibility

### Performance
- ✅ O(1) lookup by path/ID instead of O(n) iteration
- ✅ Cached qualified names (computed once)
- ✅ Efficient filtering by package/class

### Developer Experience
- ✅ Human-readable function names in IR dumps
- ✅ Better error messages with full paths
- ✅ Easier debugging and profiling
- ✅ Reliable test behavior

### Future Features
- ✅ Module system support
- ✅ Method overloading (distinguish by signature + path)
- ✅ Cross-package optimization
- ✅ Incremental compilation (path-based invalidation)

## Example Usage

```rust
// Define functions
let math_add = QualifiedPath::new(
    vec![interner.intern("test")],
    vec![interner.intern("Math")],
    interner.intern("add"),
    interner
);

// Lookup in MIR
let add_func = mir_module.get_function_by_path(&math_add)?;

// Call resolution in HIR→MIR
if let Some(func_id) = self.function_map.get(&func_symbol) {
    let callee_path = self.get_qualified_path(func_symbol);
    eprintln!("Calling: {}", callee_path); // "test.Math.add"
    self.builder.build_call_direct(func_id, args, ret_type)
}

// Testing
let test_func = test_module
    .get_function_by_path(&QualifiedPath::parse("test.Math.compute", interner)?)
    .expect("compute function not found");
```

## Migration Strategy

1. **Backward Compatible**: Keep existing `name` fields during transition
2. **Opt-in**: Add qualified paths alongside existing lookups
3. **Gradual**: Migrate one subsystem at a time (TAST → HIR → MIR → Cranelift)
4. **Deprecate**: Remove string-based lookups once all code uses paths

## Testing

- Add tests for qualified path parsing and construction
- Test symbol resolution by path
- Test name collision detection
- Verify deterministic test execution
- Benchmark lookup performance

## Related Systems

- **Type System**: Could use similar qualified paths for types
- **Import System**: Already uses package paths, can share infrastructure
- **Module System**: Foundation for proper module boundaries
- **Namespacing**: Enable Java-style package namespaces

## References

- Java: Fully qualified class names (e.g., `java.util.ArrayList`)
- Rust: Module paths (e.g., `std::collections::HashMap`)
- Haxe: Package-qualified types (e.g., `haxe.ds.StringMap`)
