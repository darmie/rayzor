# AST Lowering Design Document
## From Syntax Tree to Semantic Analysis Ready Representation

### Table of Contents
1. [Executive Summary](#executive-summary)
2. [Current State Analysis](#current-state-analysis)
3. [Proposed Architecture](#proposed-architecture)
4. [Stage 1: Typed AST (TAST)](#stage-1-typed-ast-tast)
5. [Stage 2: Semantic Graph (SemGraph)](#stage-2-semantic-graph-semgraph)
6. [Stage 3: Analysis Infrastructure](#stage-3-analysis-infrastructure)
7. [Data Structures Design](#data-structures-design)
8. [Implementation Phases](#implementation-phases)
9. [Performance Strategy](#performance-strategy)
10. [Integration Points](#integration-points)

---

## Executive Summary

This document outlines the design for transforming the current syntax-focused AST into a semantically-aware representation suitable for advanced static analysis, ownership checking, and lifetime verification. The transformation occurs in three distinct stages, each adding semantic information while maintaining performance.

**Goals:**
- Enable Rust-style memory safety analysis
- Support static analysis for optimization
- Maintain high performance through arena allocation
- Provide foundation for HIR/MIR lowering
- Support incremental compilation

**Non-Goals:**
- Complete HIR/MIR implementation (separate phase)
- Code generation (handled by later stages)
- Runtime optimization (handled by JIT/AOT backends)

---

## Current State Analysis

### Existing AST Strengths
- **Comprehensive Coverage**: Handles full Haxe syntax including modern features
- **Rich Error Reporting**: Rust-like error formatting with source locations
- **Well-Structured**: Clean separation between syntax elements
- **Extensible**: Easy to add new language features

### Gaps for Semantic Analysis
- **No Type Information**: AST nodes lack resolved type information
- **No Symbol Resolution**: Identifiers not linked to their declarations
- **No Scope Tracking**: No representation of lexical scoping
- **No Ownership Semantics**: Cannot track move/borrow semantics
- **No Lifetime Information**: Cannot perform lifetime analysis
- **Tree Structure Limitations**: Some analyses need graph representation

### Requirements for Next Stage
1. Full type resolution with generic instantiation
2. Symbol table with cross-references
3. Scope hierarchy with lifetime tracking
4. Ownership and borrowing semantics
5. Control and data flow representation
6. Foundation for static analysis passes

---

## Proposed Architecture

### Three-Stage Transformation Pipeline

```
Parsed AST (Current)
    ↓ Stage 1: Type Resolution & Symbol Binding
Typed AST (TAST)
    ↓ Stage 2: Graph Construction & Flow Analysis  
Semantic Graph (SemGraph)
    ↓ Stage 3: Static Analysis Passes
Analysis Results → HIR Lowering
```

### Key Design Principles

1. **Incremental Transformation**: Each stage adds information without losing previous data
2. **Performance First**: Arena allocation, efficient data structures, cache-friendly layouts
3. **Analysis Friendly**: Representations optimized for common analysis patterns
4. **Maintainable**: Clear separation of concerns, well-documented interfaces
5. **Extensible**: Easy to add new analysis passes or language features

---

## Stage 1: Typed AST (TAST)

### Purpose
Transform syntax tree into semantically-aware tree with full type information, symbol resolution, and basic ownership tracking.

### Core Components

#### 1. Symbol System
```
SymbolId → Symbol
  ├── name: String
  ├── symbol_type: ResolvedType  
  ├── kind: SymbolKind (Variable, Function, Class, etc.)
  ├── scope_id: ScopeId
  ├── lifetime_id: LifetimeId
  ├── mutability: Mutability
  └── visibility: Visibility
```

#### 2. Type System
```
ResolvedType (enum)
  ├── Primitive(PrimitiveType)
  ├── Named { symbol_id, type_args, ownership }
  ├── Function { params, return_type, effects }
  ├── Array { element_type, ownership }
  ├── Optional(inner_type)
  └── TypeVar { id, bounds }
```

#### 3. Scope System
```
ScopeTree
  ├── scopes: HashMap<ScopeId, Scope>
  └── parent_child_relationships
  
Scope
  ├── id: ScopeId
  ├── parent: Option<ScopeId>
  ├── children: Vec<ScopeId>
  ├── symbols: Vec<SymbolId>
  ├── lifetime: LifetimeId
  └── kind: ScopeKind (Function, Block, Class, etc.)
```

#### 4. Ownership Tracking
```
OwnershipKind (enum)
  ├── Owned        // move semantics
  ├── Borrowed     // immutable borrow
  ├── BorrowedMut  // mutable borrow
  └── Shared       // reference counted
  
VariableUsage (enum)
  ├── Move         // transfer ownership
  ├── Borrow       // immutable reference
  ├── BorrowMut    // mutable reference
  └── Copy         // copy for Copy types
```

### Transformation Process

#### Phase 1: Declaration Collection
- Scan all top-level declarations
- Create forward references for all types and functions
- Build initial symbol table
- Resolve import dependencies

#### Phase 2: Type Resolution
- Resolve all type annotations
- Perform type inference where needed
- Check generic constraints
- Build complete type hierarchy

#### Phase 3: Expression Typing
- Type check all expressions
- Insert implicit conversions
- Resolve method calls and overloads
- Track ownership at expression level

#### Phase 4: Lifetime Assignment
- Assign lifetime IDs to all expressions
- Track scope-based lifetimes
- Identify borrowing relationships
- Prepare for lifetime analysis

### TAST Node Examples

```rust
// Original AST
Expression::FieldAccess { 
    object: Box<Expression>, 
    field: String 
}

// Becomes TAST
TypedExpression {
    expr_type: ResolvedType,
    lifetime: LifetimeId,
    kind: FieldAccess {
        object: Box<TypedExpression>,
        field_symbol: SymbolId,  // Resolved!
        usage: VariableUsage,    // How we're using the field
    }
}
```

---

## Stage 2: Semantic Graph (SemGraph)

### Purpose
Convert tree-based TAST into graph-based representation optimized for flow analysis and optimization.

### Graph Types

#### 1. Control Flow Graph (CFG)
```
BasicBlock
  ├── id: BlockId
  ├── statements: Vec<StatementId>
  ├── terminator: Terminator
  └── predecessors: Vec<BlockId>

Terminator (enum)
  ├── Goto(BlockId)
  ├── Return(Option<ExpressionId>)
  ├── If { condition, then_block, else_block }
  ├── Switch { value, cases, default }
  └── Unreachable
```

#### 2. Data Flow Graph (DFG)
```
DataFlowNode
  ├── id: NodeId
  ├── kind: NodeKind
  ├── inputs: Vec<NodeId>
  ├── outputs: Vec<NodeId>
  └── type_info: ResolvedType

NodeKind (enum)
  ├── Variable(SymbolId)
  ├── Constant(Value)
  ├── Operation { op, operands }
  ├── Call { function, args }
  └── Phi { inputs } // For SSA form
```

#### 3. Call Graph
```
CallGraphNode
  ├── function_id: SymbolId
  ├── callers: Vec<CallSite>
  ├── callees: Vec<CallSite>
  └── lifetime_effects: Vec<LifetimeConstraint>

CallSite
  ├── location: SourceLocation
  ├── arguments: Vec<ExpressionId>
  └── lifetime_propagation: LifetimePropagation
```

#### 4. Ownership Graph
```
OwnershipNode
  ├── variable: SymbolId
  ├── lifetime: LifetimeId
  ├── ownership_kind: OwnershipKind
  ├── borrows: Vec<BorrowEdge>
  └── moves: Vec<MoveEdge>

BorrowEdge
  ├── borrower: SymbolId
  ├── borrowed: SymbolId
  ├── borrow_type: BorrowType
  └── scope: ScopeId
```

### Graph Construction Algorithm

#### 1. CFG Construction
1. Create basic blocks for each statement sequence
2. Identify control flow changes (if, while, return, etc.)
3. Connect blocks with appropriate edges
4. Optimize: merge single-predecessor blocks

#### 2. DFG Construction  
1. Convert expressions to data flow nodes
2. Build SSA form (Single Static Assignment)
3. Insert φ (phi) nodes at control flow joins
4. Track variable definitions and uses

#### 3. Call Graph Construction
1. Identify all function calls
2. Resolve dynamic dispatch where possible
3. Build call relationship graph
4. Analyze lifetime propagation through calls

#### 4. Ownership Graph Construction
1. Track variable declarations and their ownership
2. Identify ownership transfers (moves)
3. Track borrowing relationships
4. Build constraint system for ownership checking

---

## Stage 3: Analysis Infrastructure

### Analysis Passes

#### 1. Lifetime Analysis
**Purpose**: Ensure all borrows are valid and no use-after-free occurs

**Algorithm**:
1. Assign lifetime variables to all references
2. Generate lifetime constraints from code structure
3. Solve constraint system
4. Report lifetime violations

**Key Checks**:
- Borrowed data outlives all borrows
- No dangling pointers
- Proper ownership transfer

#### 2. Ownership Analysis  
**Purpose**: Verify ownership rules and optimize memory usage

**Algorithm**:
1. Track ownership state of each variable
2. Verify moves don't use moved-from values
3. Check borrowing doesn't violate exclusivity
4. Optimize: suggest stack allocation opportunities

**Key Checks**:
- No use after move
- Exclusive mutable borrows
- Proper initialization

#### 3. Escape Analysis
**Purpose**: Determine which allocations can use stack instead of heap

**Algorithm**:
1. Build escape graph for all allocations
2. Track which allocations escape their scope
3. Mark non-escaping allocations for stack allocation
4. Optimize: reduce garbage collector pressure

#### 4. Dead Code Analysis
**Purpose**: Identify unreachable code and unused variables

**Algorithm**:
1. Mark all reachable code from entry points
2. Identify unused variables
3. Report dead code
4. Prepare for elimination in later stages

### Analysis Results Format

```rust
AnalysisResults {
    lifetime_constraints: Vec<LifetimeConstraint>,
    ownership_violations: Vec<OwnershipError>,
    escape_analysis: HashMap<AllocationId, EscapeInfo>,
    dead_code: Vec<DeadCodeWarning>,
    optimization_hints: Vec<OptimizationHint>,
}

OptimizationHint (enum)
  ├── StackAllocatable(AllocationId)
  ├── InlineCandidate(FunctionId)
  ├── DeadCode(NodeId)
  └── OwnershipOptimization(OwnershipOptimization)
```

---

## Data Structures Design

### Memory Layout Strategy

#### Arena-Based Allocation
```rust
CompilerArenas {
    symbols: TypedArena<Symbol>,           // ~10K symbols typical
    types: TypedArena<ResolvedType>,       // ~5K types typical  
    expressions: TypedArena<TypedExpression>, // ~50K expressions typical
    statements: TypedArena<TypedStatement>,   // ~20K statements typical
    scopes: TypedArena<Scope>,             // ~1K scopes typical
    strings: StringInterner,               // Deduplicated strings
    
    // Graph arenas
    cfg_blocks: TypedArena<BasicBlock>,
    dfg_nodes: TypedArena<DataFlowNode>,
    call_sites: TypedArena<CallSite>,
    ownership_nodes: TypedArena<OwnershipNode>,
}
```

#### ID-Based References
```rust
// Instead of Box<T> or &T, use IDs for cross-references
SymbolId(u32)    // Index into symbols arena
TypeId(u32)      // Index into types arena  
ExpressionId(u32) // Index into expressions arena
ScopeId(u32)     // Index into scopes arena
LifetimeId(u32)  // Lifetime identifier
```

#### Cache-Friendly Layouts
```rust
// Group related data for better cache locality
struct SymbolTable {
    // Hot data: accessed frequently during analysis
    symbols: Vec<Symbol>,
    symbol_types: Vec<TypeId>,
    symbol_scopes: Vec<ScopeId>,
    
    // Cold data: accessed less frequently
    symbol_metadata: Vec<SymbolMetadata>,
    debug_info: Vec<DebugInfo>,
}
```

### String Interning Strategy

```rust
StringInterner {
    strings: Arena<str>,                    // Actual string storage
    map: FxHashMap<&str, InternedString>,  // Fast hash map
    reverse_map: Vec<&str>,                // ID → string lookup
}

// Usage: intern commonly used strings
let class_name = interner.intern("String");  // Returns InternedString(42)
```

---

## Implementation Phases

### Phase 1: Foundation (Weeks 1-2)
**Goal**: Basic infrastructure and TAST transformation

**Deliverables**:
- [ ] Arena allocation system
- [ ] Core data structures (Symbol, ResolvedType, etc.)
- [ ] Basic symbol table
- [ ] Scope tree implementation
- [ ] String interning

**Success Criteria**:
- Can parse simple class and create TAST
- Symbol resolution works for basic cases
- Arena allocation benchmarks show performance improvement

### Phase 2: Type System (Weeks 3-4)  
**Goal**: Complete type resolution and checking

**Deliverables**:
- [ ] Full type checker implementation
- [ ] Generic type handling
- [ ] Type inference engine
- [ ] Ownership kind assignment
- [ ] Basic lifetime tracking

**Success Criteria**:
- Type checks all parser test cases
- Handles complex generic scenarios
- Reports meaningful type errors

### Phase 3: Graph Construction (Weeks 5-6)
**Goal**: Convert TAST to semantic graphs

**Deliverables**:
- [ ] Control Flow Graph builder
- [ ] Data Flow Graph builder  
- [ ] SSA form conversion
- [ ] Call graph construction
- [ ] Ownership graph construction

**Success Criteria**:
- Generates correct CFG for control flow constructs
- DFG correctly represents data dependencies
- Call graph handles method dispatch

### Phase 4: Analysis Passes (Weeks 7-8)
**Goal**: Implement core static analysis

**Deliverables**:
- [ ] Lifetime analysis engine
- [ ] Ownership checking
- [ ] Basic escape analysis
- [ ] Dead code detection
- [ ] Analysis result reporting

**Success Criteria**:
- Catches common memory safety violations
- Provides helpful error messages
- Suggests optimization opportunities

### Phase 5: Integration & Optimization (Weeks 9-10)
**Goal**: Performance optimization and HIR integration prep

**Deliverables**:
- [ ] Performance profiling and optimization
- [ ] Integration with existing parser
- [ ] Incremental compilation support
- [ ] HIR lowering interface
- [ ] Comprehensive testing

**Success Criteria**:
- Handles large codebases efficiently
- Memory usage within reasonable bounds
- Ready for HIR integration

---

## Performance Strategy

### Allocation Patterns
- **Arena Allocation**: Eliminates fragmentation and individual deallocations
- **Bump Pointer**: O(1) allocation for most cases
- **Batch Deallocation**: Drop entire compilation phase at once

### Cache Optimization
- **Data Locality**: Group frequently accessed data
- **Hot/Cold Separation**: Keep hot data compact
- **Predictable Access Patterns**: Linear scans where possible

### Memory Usage Targets
- **Symbol Table**: ~400 bytes per symbol (reasonable for 10K symbols = 4MB)
- **Type Table**: ~200 bytes per type (5K types = 1MB)
- **Expression Nodes**: ~150 bytes per expression (50K expressions = 7.5MB)
- **Total Working Set**: Target <100MB for large codebases

### Benchmarking Strategy
```rust
// Performance test cases
benchmark_cases = [
    "small_class.hx",        // 100 LOC
    "medium_project.hx",     // 1K LOC  
    "large_codebase.hx",     // 10K LOC
    "stdlib_subset.hx",      // Complex generics
    "deep_inheritance.hx",   // Deep class hierarchies
]

// Metrics to track
performance_metrics = [
    "parse_time",
    "tast_conversion_time", 
    "graph_construction_time",
    "analysis_time",
    "peak_memory_usage",
    "allocation_count",
]
```

---

## Integration Points

### With Existing Parser
```rust
// Clean interface between parser and TAST
pub fn lower_to_tast(
    ast: HaxeFile,
    arenas: &CompilerArenas,
    options: LoweringOptions,
) -> Result<TypedFile, Vec<TypeError>> {
    let mut lowering = AstToTastLowering::new(arenas);
    lowering.lower_file(ast)
}
```

### With Future HIR  
```rust
// Prepare for HIR lowering
pub fn prepare_for_hir(
    tast: TypedFile,
    graphs: SemanticGraphs,
    analysis: AnalysisResults,
) -> HIRLoweringContext {
    HIRLoweringContext {
        typed_ast: tast,
        cfg: graphs.control_flow,
        dfg: graphs.data_flow,
        lifetime_constraints: analysis.lifetime_constraints,
        optimization_hints: analysis.optimization_hints,
    }
}
```

### Error Reporting Integration
```rust
// Leverage existing error formatting system
impl From<TypeError> for ParseError {
    fn from(type_error: TypeError) -> Self {
        // Convert semantic errors to parser error format
        // Reuse existing error formatting infrastructure
    }
}
```

### Incremental Compilation
```rust
// Design for incremental compilation from start
pub struct IncrementalContext {
    symbol_cache: HashMap<FileId, SymbolTable>,
    type_cache: HashMap<FileId, TypeTable>,
    dependency_graph: DependencyGraph,
    invalidation_tracker: InvalidationTracker,
}
```

---

## Next Steps

1. **Review & Approval**: Review this design document
2. **Architecture Validation**: Validate approach with small prototype
3. **Implementation Planning**: Break down Phase 1 into daily tasks
4. **Performance Baseline**: Establish current parser performance metrics
5. **Begin Implementation**: Start with arena allocation and core data structures

This design provides a solid foundation for achieving your memory safety and static analysis goals while maintaining the high performance requirements of your hybrid VM/compiler system.