# MIR Safety Enforcement Architecture

## Overview

Memory safety in Rayzor is enforced through a **two-phase approach**:

1. **Semantic Analysis Phase** (TAST level) - Analyzes ownership, borrows, lifetimes
2. **MIR Validation Phase** (IR level) - Enforces constraints before codegen

This architecture **reuses existing analysis** instead of duplicating logic.

---

## Phase 1: Semantic Analysis (TAST Level)

### Location
`compiler/src/semantic_graph/`

### Components

#### OwnershipGraph (`ownership_graph.rs`)
Tracks ownership relationships for all variables:
```rust
pub struct OwnershipGraph {
    pub variables: IdMap<SymbolId, OwnershipNode>,
    pub lifetimes: IdMap<LifetimeId, Lifetime>,
    pub borrow_edges: IdMap<BorrowEdgeId, BorrowEdge>,
    pub move_edges: IdMap<MoveEdgeId, MoveEdge>,
    pub lifetime_constraints: Vec<LifetimeConstraint>,
}
```

**Tracks:**
- Variable ownership (Owned, Borrowed, BorrowedMut, Shared)
- Move operations and their locations
- Borrow relationships (who borrows from whom)
- Whether variables have been moved

#### LifetimeAnalyzer (`analysis/lifetime_analyzer.rs`)
Analyzes lifetimes and detects violations:
```rust
pub struct LifetimeAnalyzer {
    // Analyzes:
    // - Borrow scopes
    // - Lifetime outlive relationships
    // - Dangling references
    // - Use-after-free
}
```

#### OwnershipAnalyzer (`analysis/ownership_analyzer.rs`)
Analyzes ownership patterns:
```rust
pub struct OwnershipAnalyzer {
    // Detects:
    // - Use after move
    // - Double moves
    // - Borrow conflicts
    // - Invalid aliasing
}
```

### Output
`SemanticGraphs` struct containing:
- CFG (Control Flow Graph)
- DFG (Data Flow Graph)
- OwnershipGraph
- Lifetime constraints
- Detected violations

---

## Phase 2: MIR Validation (IR Level)

### Location
`compiler/src/ir/validation.rs`

### Purpose
**Enforce** the constraints discovered by semantic analysis at the MIR level.

### Key Principle
**DO NOT re-analyze** - instead, **consume** the semantic analysis results.

### Architecture

```rust
pub struct MirSafetyValidator<'a> {
    /// Ownership graph from semantic analysis
    ownership_graph: &'a OwnershipGraph,

    /// Lifetime constraints from semantic analysis
    lifetime_constraints: &'a [LifetimeConstraint],

    /// Map TAST symbols to MIR registers
    symbol_to_register: HashMap<SymbolId, IrId>,

    /// Validation errors
    errors: Vec<ValidationError>,
}

impl<'a> MirSafetyValidator<'a> {
    /// Validate a MIR module against semantic analysis results
    pub fn validate(
        mir_module: &IrModule,
        semantic_graphs: &'a SemanticGraphs,
        symbol_table: &SymbolTable,
    ) -> Result<(), Vec<ValidationError>> {
        // 1. Build symbol-to-register mapping
        // 2. Check each MIR instruction against ownership graph
        // 3. Ensure Move/Borrow/Drop operations respect constraints
        // 4. Report violations as MIR validation errors
    }

    /// Check if a MIR Move instruction is valid
    fn validate_move_instruction(&mut self, src: IrId, dest: IrId) {
        let symbol = self.register_to_symbol(src);

        // Check ownership graph: has this symbol been moved?
        if let Some(node) = self.ownership_graph.variables.get(&symbol) {
            if node.is_moved {
                self.errors.push(ValidationError {
                    kind: ValidationErrorKind::UseOfMovedValue { register: src },
                    ...
                });
            }
        }
    }

    /// Check if a MIR Borrow instruction is valid
    fn validate_borrow_instruction(&mut self, src: IrId, dest: IrId, is_mutable: bool) {
        let symbol = self.register_to_symbol(src);

        // Check ownership graph: are there conflicting borrows?
        if let Some(node) = self.ownership_graph.variables.get(&symbol) {
            if is_mutable && !node.borrowed_by.is_empty() {
                self.errors.push(ValidationError {
                    kind: ValidationErrorKind::MutableBorrowConflict {
                        register: src,
                        existing_borrows: ...,
                    },
                    ...
                });
            }
        }
    }

    /// Check if a Drop is safe
    fn validate_drop_instruction(&mut self, value: IrId) {
        let symbol = self.register_to_symbol(value);

        // Check if there are active borrows
        if let Some(node) = self.ownership_graph.variables.get(&symbol) {
            if !node.borrowed_by.is_empty() {
                self.errors.push(ValidationError {
                    kind: ValidationErrorKind::DropWhileBorrowed {
                        register: value,
                        borrows: ...,
                    },
                    ...
                });
            }
        }
    }
}
```

---

## Data Flow

```
Haxe Source Code
    ↓
AST → TAST (Type checking)
    ↓
SEMANTIC ANALYSIS ← Build semantic graphs
    ├─ OwnershipGraph (tracks moves, borrows, ownership)
    ├─ LifetimeAnalyzer (checks lifetimes, detects dangling refs)
    ├─ OwnershipAnalyzer (detects use-after-move, conflicts)
    └─ Produces: SemanticGraphs
    ↓
HIR (High-level IR)
    ↓
MIR LOWERING (with semantic graphs)
    ├─ Convert HIR → MIR
    ├─ Insert Move/Borrow/Clone/Drop instructions
    └─ Pass semantic graphs to MIR
    ↓
MIR VALIDATION ← Enforce semantic constraints
    ├─ MirSafetyValidator
    ├─ Check each MIR instruction against OwnershipGraph
    ├─ Ensure Move/Borrow/Drop respect ownership rules
    └─ Report violations → BLOCK CODEGEN if errors
    ↓
CODEGEN (Cranelift/LLVM) - only if validation passes
```

---

## Why This Architecture?

### ✅ Advantages

1. **No Duplication**: Reuses existing ownership/lifetime analysis
2. **Source-Level Analysis**: Semantic analysis has source context (better errors)
3. **IR-Level Enforcement**: MIR validation ensures constraints hold in IR
4. **Separation of Concerns**:
   - Semantic analysis: WHAT the constraints are
   - MIR validation: ENFORCE constraints before codegen
5. **Layered Safety**:
   - Semantic analysis catches most violations with good error messages
   - MIR validation is a safety net before codegen
   - Codegen can assume MIR is valid

### ❌ What NOT to Do

1. **Don't re-analyze ownership at MIR level** - too late, no source context
2. **Don't duplicate OwnershipGraph logic** - already have it!
3. **Don't try to infer ownership from MIR** - semantic analysis already did this

---

## Implementation Plan

### Step 1: Symbol-to-Register Mapping
Build mapping from TAST SymbolId to MIR IrId during HIR→MIR lowering.

### Step 2: MirSafetyValidator
Create validator that consumes SemanticGraphs and validates MIR.

### Step 3: Integration
Call MirSafetyValidator after MIR lowering, before codegen:
```rust
// In pipeline.rs
let semantic_graphs = build_semantic_graphs(&typed_file);
let hir_module = lower_tast_to_hir(&typed_file);
let mir_module = lower_hir_to_mir(&hir_module);

// VALIDATE MIR AGAINST SEMANTIC CONSTRAINTS
MirSafetyValidator::validate(&mir_module, &semantic_graphs, &symbol_table)?;

// Only proceed to codegen if validation passes
codegen(&mir_module);
```

### Step 4: Error Reporting
Convert semantic violations to MIR validation errors with clear messages.

---

## Current Status

### ✅ Already Have
- OwnershipGraph (tracks ownership, moves, borrows)
- LifetimeAnalyzer (analyzes lifetimes)
- OwnershipAnalyzer (detects violations)
- SemanticGraphs structure
- MIR instructions (Move, Borrow, Clone, Drop)

### 🚧 Need to Implement
- MirSafetyValidator that consumes SemanticGraphs
- Symbol-to-Register mapping during MIR lowering
- Integration into pipeline (validate before codegen)
- Error reporting that maps MIR errors to source locations

### 📝 Next Steps
1. Add symbol tracking to MIR lowering
2. Implement MirSafetyValidator::validate()
3. Integrate into compilation pipeline
4. Test with ownership violations
5. Ensure codegen only runs if MIR is valid

---

## Example

### Haxe Code
```haxe
@:safety(strict)
class Test {
    static function main() {
        var x = new Resource();
        var y = x;  // Move
        trace(x);   // ERROR: use after move
    }
}
```

### Semantic Analysis
```
OwnershipGraph:
  - Variable x: Owned → Moved (at line 4)
  - Variable y: Owned
  - MoveEdge: x → y

Violations detected:
  - Use after move: x used at line 5, moved at line 4
```

### MIR Lowering
```
%0 = new Resource
%1 = move %0      # Move instruction
%2 = trace %0     # ← This will be caught!
```

### MIR Validation
```
MirSafetyValidator:
  - Check instruction: move %0 → %1
  - Mark %0 as Moved in ownership_states
  - Check instruction: trace %0
  - Lookup %0 → SymbolId(x)
  - Check OwnershipGraph: x.is_moved = true
  - ERROR: UseOfMovedValue { register: %0 }
  - BLOCK CODEGEN
```

### Result
Compilation fails with clear error before reaching codegen.

---

## Summary

**Memory safety in Rayzor = Semantic Analysis + MIR Enforcement**

- Semantic analysis (TAST level): Analyze ownership, lifetimes, detect violations
- MIR validation (IR level): Enforce constraints, block unsafe code from codegen
- Codegen (Cranelift/LLVM): Can assume MIR is safe, generate optimized code

This architecture **reuses existing analysis** and provides **layered safety guarantees**.
