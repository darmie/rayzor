# BLADE Full Implementation Plan

## Executive Summary

BLADE (Blazing Language Artifact Deployment Environment) is 60-70% complete. The binary format, type extraction, and MIR caching work. The critical missing piece is **full type registration from cache** that properly integrates with the symbol table, type table, and namespace resolver.

**Goal**: Enable cache loading to skip recompilation of unchanged stdlib files, achieving 35-50% compilation speedup for incremental builds.

---

## Current State

### What Works
1. **BLADE binary format** - Efficient postcard serialization
2. **Type info extraction** - `extract_type_info()` extracts classes, enums, aliases from TypedFile
3. **MIR caching** - MIR modules are saved and can be loaded
4. **Metadata validation** - Source hash, timestamp, compiler version checks
5. **Cache directory structure** - `.rayzor/blade/stdlib/` with module naming

### What's Disabled
Cache loading is commented out in two locations:
- `load_stdlib_batch()` line 1044-1053
- `load_and_compile_import_file()` line 1142-1150

Reason: `register_types_from_blade()` creates only top-level symbols, missing:
- Field/method symbols
- Type parameter context
- Scope hierarchy
- Namespace resolver integration

---

## Implementation Plan

### Phase 1: Enhance BladeTypeInfo (1-2 hours)

**Goal**: Store enough information to reconstruct TypedFile without re-parsing.

#### 1.1 Add Symbol Metadata to BladeClassInfo

```rust
// In blade.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BladeFieldInfo {
    pub name: String,
    pub field_type: String,
    pub is_public: bool,
    pub is_static: bool,           // NEW
    pub is_final: bool,            // NEW
    pub has_default: bool,         // NEW
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BladeMethodInfo {
    pub name: String,
    pub params: Vec<BladeParamInfo>,
    pub return_type: String,
    pub is_public: bool,
    pub is_static: bool,           // NEW
    pub is_inline: bool,           // NEW
    pub type_params: Vec<String>,  // NEW - generic methods
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BladeClassInfo {
    // Existing fields...
    pub is_extern: bool,
    pub is_abstract: bool,         // NEW
    pub is_final: bool,            // NEW
    pub constructor: Option<BladeMethodInfo>,  // NEW - explicit constructor
}
```

#### 1.2 Update extract_type_info()

Update the extraction in `compilation.rs` to populate new fields:

```rust
let class_info = BladeClassInfo {
    name: class_name.clone(),
    package: package.clone(),
    extends: class.super_class.map(|tid| self.type_to_string(tid)),
    implements: class.interfaces.iter()
        .map(|tid| self.type_to_string(*tid))
        .collect(),
    type_params: class.type_parameters.iter()
        .map(|tp| self.string_interner.get(tp.name).unwrap_or_default().to_string())
        .collect(),
    is_extern: class.is_extern,
    is_abstract: class.is_abstract,
    is_final: class.is_final,
    fields: class.fields.iter().map(|f| BladeFieldInfo {
        name: self.string_interner.get(f.name).unwrap_or_default().to_string(),
        field_type: self.type_to_string(f.field_type),
        is_public: matches!(f.visibility, Visibility::Public),
        is_static: f.is_static,
        is_final: f.is_final,
        has_default: f.default_value.is_some(),
    }).collect(),
    methods: class.methods.iter().map(|m| BladeMethodInfo {
        name: self.string_interner.get(m.name).unwrap_or_default().to_string(),
        params: m.params.iter().map(|p| BladeParamInfo {
            name: self.string_interner.get(p.name).unwrap_or_default().to_string(),
            param_type: self.type_to_string(p.param_type),
            has_default: p.default_value.is_some(),
        }).collect(),
        return_type: self.type_to_string(m.return_type),
        is_public: matches!(m.visibility, Visibility::Public),
        is_static: m.is_static,
        is_inline: m.is_inline,
        type_params: m.type_params.iter()
            .map(|tp| self.string_interner.get(tp.name).unwrap_or_default().to_string())
            .collect(),
    }).collect(),
    static_fields: // Similar to fields
    static_methods: // Similar to methods
    constructor: class.constructor.as_ref().map(|c| /* extract */),
};
```

---

### Phase 2: Type String Resolution (2-3 hours)

**Goal**: Convert type strings like "Array<Int>" back to TypeIds.

#### 2.1 Create parse_type_string()

```rust
impl CompilationUnit {
    /// Parse a type string and return the corresponding TypeId
    /// Creates the type if it doesn't exist
    fn parse_type_string(&mut self, type_str: &str) -> TypeId {
        let type_str = type_str.trim();

        // Handle primitives
        match type_str {
            "Int" => return self.type_table.borrow().int_type(),
            "Float" => return self.type_table.borrow().float_type(),
            "Bool" => return self.type_table.borrow().bool_type(),
            "String" => return self.type_table.borrow().string_type(),
            "Void" => return self.type_table.borrow().void_type(),
            "Dynamic" => return self.type_table.borrow().dynamic_type(),
            _ => {}
        }

        // Handle Null<T>
        if let Some(inner) = type_str.strip_prefix("Null<").and_then(|s| s.strip_suffix(">")) {
            let inner_type = self.parse_type_string(inner);
            return self.type_table.borrow_mut().create_type(
                TypeKind::Optional { inner_type }
            );
        }

        // Handle Array<T>
        if let Some(inner) = type_str.strip_prefix("Array<").and_then(|s| s.strip_suffix(">")) {
            let element_type = self.parse_type_string(inner);
            return self.type_table.borrow_mut().create_type(
                TypeKind::Array { element_type }
            );
        }

        // Handle function types: (A, B) -> C
        if type_str.starts_with("(") {
            if let Some((params_str, return_str)) = type_str.split_once(") -> ") {
                let params_str = params_str.trim_start_matches('(');
                let params: Vec<TypeId> = if params_str.is_empty() {
                    vec![]
                } else {
                    params_str.split(", ")
                        .map(|s| self.parse_type_string(s))
                        .collect()
                };
                let return_type = self.parse_type_string(return_str);
                return self.type_table.borrow_mut().create_type(
                    TypeKind::Function { params, return_type, effects: FunctionEffects::default() }
                );
            }
        }

        // Handle generic types: ClassName<T, U>
        if let Some(open) = type_str.find('<') {
            let base_name = &type_str[..open];
            let args_str = &type_str[open+1..type_str.len()-1];
            let type_args: Vec<TypeId> = args_str.split(", ")
                .map(|s| self.parse_type_string(s))
                .collect();

            // Look up the base class
            if let Some(symbol_id) = self.lookup_class_symbol(base_name) {
                return self.type_table.borrow_mut().create_type(
                    TypeKind::Class { symbol_id, type_args }
                );
            }
        }

        // Simple class/enum name
        if let Some(symbol_id) = self.lookup_class_symbol(type_str) {
            return self.type_table.borrow_mut().create_type(
                TypeKind::Class { symbol_id, type_args: vec![] }
            );
        }

        // Fallback to placeholder
        let name = self.string_interner.intern(type_str);
        self.type_table.borrow_mut().create_type(TypeKind::Placeholder { name })
    }

    fn lookup_class_symbol(&self, name: &str) -> Option<SymbolId> {
        let interned = self.string_interner.intern(name);
        self.symbol_table.lookup_symbol(ScopeId::first(), interned)
    }
}
```

---

### Phase 3: Full Symbol Registration (3-4 hours)

**Goal**: Replace `register_types_from_blade()` with comprehensive symbol/type setup.

#### 3.1 Create register_class_from_blade()

```rust
fn register_class_from_blade(&mut self, class_info: &BladeClassInfo) -> SymbolId {
    let qualified_name = if class_info.package.is_empty() {
        class_info.name.clone()
    } else {
        format!("{}.{}", class_info.package.join("."), class_info.name)
    };

    let short_name = self.string_interner.intern(&class_info.name);
    let qualified_interned = self.string_interner.intern(&qualified_name);

    // Create class symbol
    let class_sym = self.symbol_table.create_class_in_scope(short_name, ScopeId::first());

    // Set symbol metadata
    if let Some(sym) = self.symbol_table.get_symbol_mut(class_sym) {
        sym.qualified_name = Some(qualified_interned);
        sym.is_extern = class_info.is_extern;
    }

    // Create class type
    let class_type = self.type_table.borrow_mut().create_type(
        TypeKind::Class { symbol_id: class_sym, type_args: vec![] }
    );

    // Link symbol and type
    self.symbol_table.update_symbol_type(class_sym, class_type);
    self.symbol_table.register_type_symbol_mapping(class_type, class_sym);

    // Register in scope with both names
    if let Some(scope) = self.scope_tree.get_scope_mut(ScopeId::first()) {
        scope.add_symbol(class_sym, qualified_interned);
        scope.add_symbol(class_sym, short_name);
    }

    // Register fields
    for field in &class_info.fields {
        self.register_field_symbol(class_sym, field);
    }

    // Register methods
    for method in &class_info.methods {
        self.register_method_symbol(class_sym, method);
    }

    // Update namespace resolver
    self.namespace_resolver.register_type(&qualified_name, class_sym);

    class_sym
}

fn register_field_symbol(&mut self, class_sym: SymbolId, field: &BladeFieldInfo) {
    let field_name = self.string_interner.intern(&field.name);
    let field_type = self.parse_type_string(&field.field_type);

    // Create field symbol
    let field_sym = self.symbol_table.create_field_symbol(
        field_name,
        class_sym,
        field_type,
        field.is_static,
        field.is_public,
    );

    // Register as class member
    self.symbol_table.add_class_member(class_sym, field_sym);
}

fn register_method_symbol(&mut self, class_sym: SymbolId, method: &BladeMethodInfo) {
    let method_name = self.string_interner.intern(&method.name);

    // Parse parameter types
    let param_types: Vec<TypeId> = method.params.iter()
        .map(|p| self.parse_type_string(&p.param_type))
        .collect();

    let return_type = self.parse_type_string(&method.return_type);

    // Create function type
    let func_type = self.type_table.borrow_mut().create_type(
        TypeKind::Function {
            params: param_types,
            return_type,
            effects: FunctionEffects::default(),
        }
    );

    // Create method symbol
    let method_sym = self.symbol_table.create_method_symbol(
        method_name,
        class_sym,
        func_type,
        method.is_static,
        method.is_public,
    );

    // Register as class member
    self.symbol_table.add_class_member(class_sym, method_sym);
}
```

---

### Phase 4: Enable Cache Loading (1-2 hours)

#### 4.1 Uncomment and Update Cache Loading Code

```rust
// In load_stdlib_batch()
if self.config.enable_cache {
    if let Some((cached_mir, cached_type_info)) = self.try_load_blade_cached(&filename, &source) {
        // Full type registration
        self.register_types_from_blade_v2(&cached_type_info);

        // Add cached MIR
        if !cached_mir.functions.is_empty() {
            self.mir_modules.push(std::sync::Arc::new(cached_mir));
        }

        // Create stub TypedFile for compatibility
        let stub_typed_file = self.create_stub_typed_file(&cached_type_info, &filename);
        self.loaded_stdlib_typed_files.push(stub_typed_file);

        debug!("[BLADE] Loaded from cache: {}", name);
        continue;
    }
}
```

#### 4.2 Create Stub TypedFile

```rust
fn create_stub_typed_file(&self, type_info: &BladeTypeInfo, filename: &str) -> TypedFile {
    TypedFile {
        package_name: type_info.classes.first()
            .map(|c| c.package.join("."))
            .unwrap_or_default(),
        imports: vec![],
        using_statements: vec![],
        classes: type_info.classes.iter().map(|c| self.create_stub_typed_class(c)).collect(),
        interfaces: vec![],
        enums: type_info.enums.iter().map(|e| self.create_stub_typed_enum(e)).collect(),
        type_aliases: type_info.type_aliases.iter().map(|t| self.create_stub_typed_alias(t)).collect(),
        functions: vec![],
        file_path: Some(filename.to_string()),
    }
}
```

---

### Phase 5: Testing & Validation (1-2 hours)

#### 5.1 Test Cases

1. **Cache miss**: Clean cache, compile, verify files created
2. **Cache hit**: Second compilation, verify speedup
3. **Cache invalidation**: Modify source, verify recompilation
4. **Multi-file**: File A depends on cached file B
5. **Generic types**: Array<Int>, Map<String, Int>
6. **Extern classes**: sys.io.File, sys.FileSystem
7. **Method resolution**: Call cached class methods

#### 5.2 Performance Benchmarks

```bash
# Benchmark script
echo "Cold cache (first run):"
rm -rf .rayzor/blade/stdlib/*
time cargo run --package compiler --example test_file_streams_simple

echo "Warm cache (second run):"
time cargo run --package compiler --example test_file_streams_simple
```

Expected results:
- Cold cache: ~1-1.5s TAST lowering
- Warm cache: ~200-400ms TAST lowering (50-70% improvement)

---

## Implementation Order

| Phase | Task | Time | Priority |
|-------|------|------|----------|
| 1.1 | Add new fields to BladeTypeInfo | 30min | High |
| 1.2 | Update extract_type_info() | 30min | High |
| 2.1 | Create parse_type_string() | 1h | Critical |
| 3.1 | Create register_class_from_blade() | 2h | Critical |
| 3.2 | Create field/method registration | 1h | Critical |
| 4.1 | Enable cache loading | 30min | High |
| 4.2 | Create stub TypedFile | 30min | Medium |
| 5.1 | Write tests | 1h | Medium |
| 5.2 | Performance benchmarks | 30min | Medium |

**Total estimated time**: 8-10 hours

---

## Success Criteria

1. All existing tests pass (test_file_streams_simple, etc.)
2. Cache files created for all stdlib modules
3. Second compilation shows 50%+ speedup in TAST lowering
4. No type resolution errors when using cached types
5. Generic types work correctly (Array<T>, etc.)

---

## Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Type string parsing edge cases | Type resolution fails | Add comprehensive unit tests |
| Symbol table corruption | Compilation fails | Add validation checks |
| Cache invalidation miss | Stale types used | Add dependency tracking |
| BLADE version incompatibility | Old cache breaks | Bump BLADE_VERSION |

---

## Files to Modify

1. `compiler/src/ir/blade.rs` - Enhance BladeTypeInfo structures
2. `compiler/src/compilation.rs` - Add parsing and registration functions
3. `compiler/src/bin/preblade.rs` - Update to save enhanced type info
4. Tests for cache functionality
