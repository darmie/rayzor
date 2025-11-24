# IR Module Backlog

## HIR (High-level IR) Outstanding Work

### 1. ✅ ClassHierarchyInfo Extension COMPLETED
**File**: `/compiler/src/tast/type_checker.rs:970-997`
**Status**: COMPLETED - All fields added
**Added Fields**:
- ✅ `is_final: bool` - Whether class cannot be extended
- ✅ `is_abstract: bool` - Whether class cannot be instantiated  
- ✅ `is_extern: bool` - Whether class is defined externally
- ✅ `is_interface: bool` - Whether this is an interface
- ✅ `sealed_to: Option<Vec<TypeId>>` - Types this abstract/enum is sealed to

**Updated Files**:
- `type_checker.rs` - Added fields to struct
- `class_builder.rs` - Updated all instantiation sites
- `symbols.rs` - Updated test instantiations
- `tast_to_hir.rs:165-180` - Using real values from hierarchy

**Remaining Work**:
- Need to populate these fields from actual class metadata during type checking
- Extract `@:final`, `@:abstract`, `@:extern` metadata
- Detect interface vs class from declaration

### 2. Array Comprehension Desugaring
**File**: `tast_to_hir.rs:1114-1279`
**Status**: IN PROGRESS - Basic structure done, needs proper method resolution
**Blockers**:
- Need proper Array.push method symbol resolution (line 1251)
- TypedComprehensionFor doesn't have condition field for filters
- Need to check if array comprehensions support filters in TAST

**Requirements**:
- ✅ Unique temp variable generation
- ✅ Nested loop handling for multiple iterators
- ⚠️ Condition/filter integration (TAST structure unclear)
- ✅ Proper scope management
- ❌ Proper method resolution for array.push()

### 2a. Method Symbol Resolution for Built-in Methods
**File**: `tast_to_hir.rs:1377-1406`
**Status**: PARTIALLY IMPLEMENTED
**Approach**: Using extern classes from haxe-std/
- Array.hx, String.hx loaded by stdlib_loader
- Methods registered during AST lowering
- lookup_array_push_method now searches symbol table

**Current Implementation**:
- ✅ Loads Array.hx from haxe-std/
- ✅ Registers Array as extern class with @:coreType
- ✅ Method lookup via symbol table
- ⚠️ May fail if stdlib not loaded properly
- ⚠️ Generic type parameters not fully handled

**Remaining Issues**:
- Need to ensure stdlib is loaded before HIR lowering
- Generic method signatures (Array<T>.push(T))
- Overloaded methods not handled
**Impact**: Array comprehension partially unblocked

### 2b. Constructor Validation in HIR Lowering
**File**: `tast_to_hir.rs:validate_constructor()`
**Status**: BASIC IMPLEMENTATION - Only checks constructor existence
**Current Implementation**:
- ✅ Validates constructor exists when `new` is called with arguments
- ✅ Reports error for missing constructors
- ❌ Does not validate constructor signatures
- ❌ Does not check accessibility rules
- ❌ Does not handle multiple constructor overloads
- ❌ Does not validate generic type parameters
- ❌ Does not look up constructors in imported modules

**Missing Features**:
1. **Constructor Signature Validation**
   - Check argument types match constructor parameter types
   - Handle type coercion and compatibility
   - Validate generic type parameters match constructor constraints

2. **Constructor Accessibility Checks**
   - Validate visibility rules (private/protected/public)
   - Check if constructor is accessible from current context
   - Handle internal/package visibility

3. **Multiple Constructor Overloads**
   - Support classes with multiple constructors
   - Select best matching constructor based on argument types
   - Handle ambiguous constructor calls with proper error messages

4. **Generic Type Parameter Validation**
   - Validate generic type arguments match constructor constraints
   - Check variance rules for generic parameters
   - Handle bounded type parameters

5. **Cross-Module Constructor Resolution**
   - Look up constructors in imported modules and external libraries
   - Handle extern class constructors
   - Validate imported constructor accessibility

**Priority**: HIGH - Constructor validation is critical for type safety

### 3. Method Abstract Detection
**File**: `tast_to_hir.rs:261`
**Current**: Using `method.body.is_empty()`
**Needed**: 
- Check for `@:abstract` metadata
- Distinguish between abstract and extern methods
- Handle interface methods (always abstract)

### 4. Pattern Matching Desugaring
**File**: `tast_to_hir.rs:1397-1626` - desugar_pattern_match
**Status**: PARTIALLY COMPLETE - Basic desugaring done, complex patterns blocked
**Completed**:
- ✅ Pattern match to if-else chain conversion
- ✅ Temp variable generation for match target
- ✅ Variable pattern with bindings
- ✅ Wildcard pattern (always matches)
- ✅ Literal pattern equality checks
- ✅ Guard pattern support (pattern && guard)

**Blocked on**:
- ❌ Constructor patterns (need enum variant runtime checks)
- ❌ Array patterns (need length checks and element extraction)
- ❌ Object patterns (need field existence checks)
- ❌ Extractor patterns (need method call support)

### 5. Import and Module Field Lowering
**Files**: 
- `tast_to_hir.rs:1040` - lower_import (empty)
- `tast_to_hir.rs:1044` - lower_module_field (empty)
**Impact**: Cannot handle multi-file projects properly

### 6. Type Parameter Lowering
**File**: `tast_to_hir.rs:1048`
**Current**: Returns empty Vec
**Needed**: Convert TypedTypeParameter → HirTypeParam with constraints

## MIR (Mid-level IR) Outstanding Work

### 1. HIR to MIR Lowering Gaps - WEEK 3 IN PROGRESS
**File**: `hir_to_mir.rs`
**Status**: Implementing missing pieces

**Completed Today**:
- ✅ Pattern binding implementation (lines 615-673)
  - Variable, wildcard, tuple patterns working
  - Type/guard patterns delegate to inner pattern
  - Or patterns bind first alternative
  - Complex patterns (constructor, array, object) error gracefully

**Completed in Week 3**:
- ✅ Pattern binding implementation (lines 615-673)
- ✅ Lvalue operations (read/write) - lines 675-737
- ✅ Field access (partial - needs field index mapping) - line 740-752
- ✅ Index access - line 754-764
- ✅ Logical operators (short-circuit AND/OR) - lines 766-822
- ✅ Added build_extract_value to IrBuilder

**Still Missing**:
- ❌ Conditional expressions (ternary) - line 824
- ❌ Do-while loops - line 830
- ❌ For-in loops (iterator desugaring) - line 836
- ❌ Switch statements - line 842
- ❌ Try-catch-finally - line 848
- ❌ Lambda/closure lowering - line 854
- ❌ Array literal - line 860
- ❌ Map literal - line 866
- ❌ Object literal - line 872
- ❌ String interpolation - line 878
- ❌ Inline code - line 884
- ❌ Global variables - line 890
- ❌ Type metadata registration - line 896

**Known Issues**:
- Field access requires proper SymbolId → field index mapping
- Need to track predecessor blocks correctly for phi nodes in logical operators

### 2. MIR Validation
**Status**: Basic validation exists
**Needed**:
- SSA form validation
- Type consistency checking
- Control flow verification
- Debug assertions

## LIR (Low-level IR) Outstanding Work

### 1. Not Yet Implemented
**Status**: LIR structure exists but no lowering from MIR
**Required**: Complete MIR → LIR pipeline

## Optimization Passes

### 1. HIR-specific Optimizations
**File**: `optimizable.rs`
**Current**: HirDeadCodeElimination only
**Needed**:
- Constant folding
- Simple inlining
- Redundant cast elimination

### 2. MIR Optimizations
**Status**: Basic DCE and constant folding
**Needed**:
- Loop optimizations
- Escape analysis
- Devirtualization

## Testing Requirements

Each item needs:
1. Unit tests in module
2. Integration tests with real Haxe code
3. Validation that pipeline still works
4. Performance benchmarks for complex cases

## Priority Order

1. **CRITICAL**: ClassHierarchyInfo extension (blocks proper class handling)
2. **HIGH**: Array comprehension desugaring (needed for correct semantics)
3. **HIGH**: HIR to MIR completion (blocks optimization pipeline)
4. **MEDIUM**: Pattern matching desugaring (advanced feature)
5. **LOW**: Import/module field lowering (can work around for now)