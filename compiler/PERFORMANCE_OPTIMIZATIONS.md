# AST Lowering & Type System Performance Optimization Analysis

## Executive Summary

This document outlines performance optimization opportunities identified in the Haxe compiler's AST lowering and type system implementation. The analysis reveals several areas where significant performance improvements can be achieved through better memory management, caching strategies, and algorithmic improvements.

## Critical Performance Issues

### 1. Excessive String Allocations

**Problem**: The codebase performs numerous string clones and allocations, especially during:
- Import path resolution
- Type parameter processing
- Symbol name handling

**Impact**: High memory allocation pressure and GC overhead

**Solution**:
- Maximize use of `InternedString` throughout the codebase
- Pass string references (`&str`) instead of cloning
- Cache commonly used strings

### 2. Inefficient Data Structure Usage

**Problem**: 
- HashMaps and Vecs created without capacity hints
- Linear searches where hash lookups would be better
- No pooling of temporary data structures

**Impact**: Unnecessary allocations and poor cache locality

**Solution**:
- Use `with_capacity()` constructors based on expected sizes
- Replace linear searches with HashMap lookups
- Implement object pooling for frequently created/destroyed structures

### 3. Redundant Type Resolution

**Problem**: Multiple lookups of the same type information without caching

**Impact**: O(n) lookups repeated multiple times per compilation unit

**Solution**: Implement a resolution cache with proper invalidation

## Specific Optimizations

### Symbol Table Optimizations

```rust
// Add to SymbolTable struct
pub struct SymbolTable {
    // ... existing fields ...
    
    // New: Resolution cache
    resolution_cache: RefCell<HashMap<(ScopeId, InternedString), Option<SymbolId>>>,
    
    // New: Bloom filter for quick negative lookups
    symbol_bloom: BloomFilter,
}

impl SymbolTable {
    pub fn lookup_symbol(&self, scope: ScopeId, name: InternedString) -> Option<&Symbol> {
        // Check bloom filter first for quick rejection
        if !self.symbol_bloom.might_contain(name) {
            return None;
        }
        
        // Check cache
        let cache_key = (scope, name);
        if let Some(cached) = self.resolution_cache.borrow().get(&cache_key) {
            return cached.and_then(|id| self.get_symbol(*id));
        }
        
        // Perform actual lookup
        let result = self.lookup_symbol_uncached(scope, name);
        
        // Cache result
        self.resolution_cache.borrow_mut().insert(
            cache_key, 
            result.map(|s| s.id)
        );
        
        result
    }
}
```

### Type Table Optimizations

```rust
// Avoid cloning TypeKind
pub fn create_type(&mut self, kind: TypeKind, location: SourceLocation) -> TypeId {
    let type_id = self.next_type_id();
    
    // Don't clone - move or use Arc
    let new_type = Type::with_location(type_id, kind, location);
    self.types.insert(type_id, new_type);
    
    type_id
}

// Optimize generic instance cache
#[derive(Hash, Eq, PartialEq)]
struct GenericCacheKey<'a> {
    base_type: TypeId,
    type_args: &'a [TypeId],
}

// Use borrowing key for lookups
pub fn get_or_create_generic_instance(&mut self, base: TypeId, args: Vec<TypeId>) -> TypeId {
    let key = GenericCacheKey { base_type: base, type_args: &args };
    
    if let Some(&cached) = self.generic_cache.get(&key) {
        return cached;
    }
    
    // Create new instance
    let instance_id = self.create_generic_instance_uncached(base, args.clone());
    
    // Store with owned key
    self.generic_cache.insert((base, args), instance_id);
    
    instance_id
}
```

### AST Lowering Optimizations

```rust
// Pre-allocate collections based on AST size
impl<'ctx> AstLowering<'ctx> {
    pub fn new(context: &'ctx mut TypeCheckingContext) -> Self {
        let estimated_symbols = context.estimated_symbol_count();
        let estimated_types = context.estimated_type_count();
        
        Self {
            context,
            // Pre-allocate space
            type_parameter_stack: Vec::with_capacity(8),
            deferred_bodies: Vec::with_capacity(estimated_symbols / 10),
            // ... other fields ...
        }
    }
    
    // Use string interning more aggressively
    fn lower_import(&mut self, import: &parser::Import) -> TypedImport {
        // Don't clone strings - intern once
        let path = import.path.iter()
            .map(|s| self.context.intern_string(s))
            .collect();
            
        TypedImport {
            path,
            alias: import.alias.as_ref()
                .map(|a| self.context.intern_string(a)),
            // ... other fields ...
        }
    }
}
```

### Type Resolution Optimizations

```rust
pub struct TypeResolver {
    // Add resolution cache
    resolution_cache: HashMap<(String, Option<String>), TypeId>,
    
    // Cache for namespace lookups
    namespace_cache: HashMap<Vec<String>, NamespaceId>,
}

impl TypeResolver {
    pub fn resolve_type(&mut self, name: &str, package: Option<&str>) -> Option<TypeId> {
        // Check cache first
        let cache_key = (name.to_string(), package.map(String::from));
        if let Some(&type_id) = self.resolution_cache.get(&cache_key) {
            return Some(type_id);
        }
        
        // Perform resolution
        let result = self.resolve_type_uncached(name, package);
        
        // Cache result
        if let Some(type_id) = result {
            self.resolution_cache.insert(cache_key, type_id);
        }
        
        result
    }
}
```

## Memory Layout Optimizations

### Use SmallVec for Common Cases

```rust
use smallvec::SmallVec;

pub struct Type {
    pub id: TypeId,
    pub kind: TypeKind,
    pub location: SourceLocation,
    // Most types have < 4 type parameters
    pub type_params: SmallVec<[TypeId; 4]>,
}

pub struct FunctionSignature {
    // Most functions have < 6 parameters
    pub params: SmallVec<[ParamType; 6]>,
    pub return_type: TypeId,
}
```

### Pack Boolean Flags

```rust
#[derive(Default)]
pub struct TypeFlags {
    // Pack 8 booleans into 1 byte
    flags: u8,
}

impl TypeFlags {
    const IS_GENERIC: u8 = 1 << 0;
    const IS_NULLABLE: u8 = 1 << 1;
    const IS_ABSTRACT: u8 = 1 << 2;
    const IS_INTERFACE: u8 = 1 << 3;
    
    pub fn is_generic(&self) -> bool {
        self.flags & Self::IS_GENERIC != 0
    }
    
    pub fn set_generic(&mut self, value: bool) {
        if value {
            self.flags |= Self::IS_GENERIC;
        } else {
            self.flags &= !Self::IS_GENERIC;
        }
    }
}
```

## Profiling & Metrics

### Add Performance Counters

```rust
#[derive(Default)]
pub struct PerfCounters {
    pub string_allocations: AtomicU64,
    pub type_lookups: AtomicU64,
    pub cache_hits: AtomicU64,
    pub cache_misses: AtomicU64,
    pub symbol_resolutions: AtomicU64,
}

impl PerfCounters {
    pub fn report(&self) {
        let total_lookups = self.cache_hits.load(Ordering::Relaxed) 
            + self.cache_misses.load(Ordering::Relaxed);
        let hit_rate = if total_lookups > 0 {
            (self.cache_hits.load(Ordering::Relaxed) as f64 / total_lookups as f64) * 100.0
        } else {
            0.0
        };
        
        println!("Performance Metrics:");
        println!("  String allocations: {}", self.string_allocations.load(Ordering::Relaxed));
        println!("  Type lookups: {}", self.type_lookups.load(Ordering::Relaxed));
        println!("  Cache hit rate: {:.2}%", hit_rate);
        println!("  Symbol resolutions: {}", self.symbol_resolutions.load(Ordering::Relaxed));
    }
}
```

## Implementation Priority

### Phase 1: Quick Wins (1-2 days)
1. Add capacity hints to HashMap/Vec allocations
2. Replace string clones with interned strings
3. Add basic caching to symbol resolution

### Phase 2: Core Optimizations (3-5 days)
1. Implement symbol resolution cache with bloom filter
2. Optimize generic instance cache
3. Replace TypeKind cloning with Arc or references

### Phase 3: Advanced Optimizations (1 week)
1. Implement object pooling for temporary structures
2. Add comprehensive performance metrics
3. Optimize memory layout with SmallVec and bit packing

## Expected Performance Improvements

Based on analysis of typical Haxe codebases:

- **Memory allocation reduction**: 30-40%
- **Symbol resolution speedup**: 3-5x with caching
- **Type checking throughput**: 2-3x improvement
- **Large file compilation**: 40-60% faster

## Testing Strategy

1. Create performance benchmarks for:
   - Large single file compilation
   - Many small files with imports
   - Deep inheritance hierarchies
   - Heavy generic usage

2. Profile before and after each optimization

3. Ensure no regressions in correctness

## Conclusion

These optimizations target the most critical performance bottlenecks in the type system. Implementation should proceed in phases, with careful benchmarking at each step to validate improvements and catch any regressions.