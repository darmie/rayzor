# DFG Builder Completion Guide

## Executive Summary

The Data Flow Graph (DFG) builder in `compiler/src/semantic_graph/dfg_builder.rs` is approximately **60% complete** and requires significant work to become production-ready. This document provides a comprehensive analysis of missing components and a detailed implementation roadmap.

**Current State**: Basic SSA construction framework exists with placeholder implementations
**Required Work**: Core SSA algorithm completion, CFG integration, and phi operand handling
**Complexity**: High - requires deep understanding of SSA construction and compiler internals

---

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Current Implementation Analysis](#current-implementation-analysis)
3. [Critical Missing Dependencies](#critical-missing-dependencies)
4. [Detailed Gap Analysis](#detailed-gap-analysis)
5. [Implementation Roadmap](#implementation-roadmap)
6. [Code Examples and Patterns](#code-examples-and-patterns)
7. [Testing Strategy](#testing-strategy)
8. [Performance Considerations](#performance-considerations)
9. [Error Handling and Validation](#error-handling-and-validation)
10. [Integration Points](#integration-points)

---

## Architecture Overview

### Data Flow in the Semantic Graph System

```
TAST → CFG Construction → Dominance Analysis → DFG Construction → Analysis
  ↓         ↓                     ↓                  ↓             ↓
TypedAST   BasicBlocks      DominanceTree    DataFlowNodes   Optimizations
           Terminators      PhiPlacement     SSA Variables   Ownership Analysis
           Predecessors     DominanceFront   DefUseChains    Lifetime Checking
```

### DFG Builder Integration Points

```
DfgBuilder
├── Input: ControlFlowGraph + TypedFunction
├── Dependencies:
│   ├── DominanceTree (for phi placement)
│   ├── TastCfgMapping (statement-to-block mapping)
│   └── DataFlowGraph (output structure)
├── Algorithm Phases:
│   ├── 1. Parameter initialization
│   ├── 2. Phi function placement
│   ├── 3. Variable renaming (SSA conversion)
│   ├── 4. Phi operand completion
│   └── 5. Graph finalization
└── Output: Complete SSA-form DataFlowGraph
```

---

## Current Implementation Analysis

### ✅ **What's Working**

1. **Solid Foundation**
   - Proper struct definitions with SSA state management
   - Correct use of existing type system (DataFlowNodeKind, etc.)
   - Basic expression handling for most TAST node types
   - Statistics tracking framework

2. **Expression Builders**
   - Most `TypedExpressionKind` variants handled correctly
   - Proper operand dependency tracking
   - Correct use of DFG node types (Constant, Variable, BinaryOp, etc.)

3. **Framework Components**
   - SSA variable allocation system
   - Node ID management
   - Basic phi node creation structure

### ❌ **What's Broken or Missing**

1. **Core SSA Algorithm**
   - Phi operand completion is empty
   - Variable renaming is incomplete
   - CFG block processing is linear, not dominance-based

2. **Integration Issues**
   - No real CFG-to-TAST mapping usage
   - Placeholder dominance tree navigation
   - Missing variable definition analysis

3. **Critical Helper Functions**
   - Block predecessor/successor navigation
   - Variable collection from function bodies
   - Statement-to-block mapping

---

## Critical Missing Dependencies

### 1. Import Issues

**Problem**: Missing essential imports prevent compilation

```rust
// Currently missing in dfg_builder.rs:
use crate::tast::collections::{new_id_set, new_id_map, IdMap, IdSet}; // ❌ Compilation fails
use crate::semantic_graph::tast_cfg_mapping::TastCfgMapping;           // ❌ Not imported
```

**Solution**: Add proper imports and handle any API differences

### 2. Type Mismatches

**Problem**: Some function signatures don't match actual APIs

```rust
// Current code assumes:
fn process_block_recursive(..., statements: &[TypedStatement], ...) 

// But should use TastCfgMapping:
fn process_block_recursive(..., mapping: &TastCfgMapping, ...)
```

### 3. Missing Core Algorithms

**Problem**: Key SSA construction steps are empty or placeholder implementations

```rust
// These methods are critical but incomplete:
fn complete_phi_operands() { Ok(()) }                    // ❌ Empty!
fn find_variable_definition_blocks() { HashSet::new() }  // ❌ Empty!
fn get_block_predecessors() { vec![] }                   // ❌ Empty!
```

---

## Detailed Gap Analysis

### Phase 1: Compilation and Basic Setup

| Component | Status | Issue | Priority |
|-----------|--------|-------|----------|
| Imports | ❌ Broken | Missing `collections` module imports | Critical |
| Type compatibility | ❌ Broken | Some function signatures wrong | Critical |
| Basic compilation | ❌ Fails | Multiple compilation errors | Critical |

### Phase 2: CFG Integration

| Component | Status | Issue | Priority |
|-----------|--------|-------|----------|
| TAST-CFG mapping | ❌ Missing | No integration with `TastCfgMapping` | High |
| Block traversal | ❌ Placeholder | Linear processing instead of CFG-based | High |
| Statement mapping | ❌ Missing | No statement-to-block mapping usage | High |
| Dominance integration | ⚠️ Partial | Basic structure exists but not fully used | Medium |

### Phase 3: SSA Algorithm Core

| Component | Status | Issue | Priority |
|-----------|--------|-------|----------|
| Phi operand completion | ❌ Empty | Core SSA requirement completely missing | Critical |
| Variable renaming | ⚠️ Partial | Basic framework but missing key logic | Critical |
| Variable collection | ❌ Incomplete | Only handles parameters, not local vars | High |
| Definition analysis | ❌ Empty | No analysis of where variables are defined | High |

### Phase 4: Expression and Statement Processing

| Component | Status | Issue | Priority |
|-----------|--------|-------|----------|
| Expression builders | ✅ Mostly complete | Some placeholders but good coverage | Low |
| Statement builders | ⚠️ Partial | Basic cases work, complex control flow needs work | Medium |
| Error handling | ⚠️ Basic | Some error cases but needs expansion | Medium |

---

## Implementation Roadmap

### Stage 1: Fix Compilation (Estimated: 2-4 hours)

**Objective**: Get the code to compile and run basic tests

**Tasks**:
1. **Fix Imports**
   ```rust
   // Add to top of dfg_builder.rs:
   use crate::tast::collections::{new_id_map, new_id_set, IdMap, IdSet};
   use crate::semantic_graph::tast_cfg_mapping::{TastCfgMapping, StatementLocation};
   ```

2. **Fix Function Signatures**
   ```rust
   // Change from:
   fn process_block_recursive(&mut self, statements: &[TypedStatement]) 
   
   // To:
   fn process_block_recursive(&mut self, block_id: BlockId, mapping: &TastCfgMapping)
   ```

3. **Fix Basic Type Issues**
   - Resolve any remaining compilation errors
   - Ensure all existing tests pass

**Acceptance Criteria**: Code compiles without errors

### Stage 2: CFG Integration (Estimated: 1-2 days)

**Objective**: Properly integrate with CFG and TAST mapping

**Tasks**:
1. **Implement Real Block Processing**
   ```rust
   fn build_dfg(&mut self, cfg: &ControlFlowGraph, function: &TypedFunction) -> Result<DataFlowGraph, GraphConstructionError> {
       // Phase 1-3: Same as current
       
       // Phase 4: Use real CFG integration
       let mapping = TastCfgMapping::build(cfg, function)?;
       self.process_function_body_with_cfg(cfg, function, &dominance_tree, &mapping)?;
       
       // Continue...
   }
   
   fn process_function_body_with_cfg(&mut self, cfg: &ControlFlowGraph, function: &TypedFunction, dominance_tree: &DominanceTree, mapping: &TastCfgMapping) -> Result<(), GraphConstructionError> {
       // Process blocks in dominance tree order
       for &block_id in &dominance_tree.reverse_postorder {
           self.process_block_with_mapping(block_id, cfg, function, dominance_tree, mapping)?;
       }
       Ok(())
   }
   ```

2. **Fix Helper Functions**
   ```rust
   fn get_block_predecessors(&self, block_id: BlockId, cfg: &ControlFlowGraph) -> Vec<BlockId> {
       cfg.get_block(block_id)
           .map(|block| block.predecessors.iter().copied().collect())
           .unwrap_or_default()
   }
   ```

3. **Statement-to-Block Mapping**
   ```rust
   fn process_block_statements(&mut self, block_id: BlockId, mapping: &TastCfgMapping, function: &TypedFunction) -> Result<(), GraphConstructionError> {
       let statements = mapping.get_statements_in_block(block_id);
       for &stmt_location in statements {
           let statement = self.get_statement_from_location(stmt_location, function)?;
           self.build_statement(statement)?;
       }
       Ok(())
   }
   ```

**Acceptance Criteria**: DFG builder can process simple functions with basic blocks

### Stage 3: Core SSA Algorithm (Estimated: 3-5 days)

**Objective**: Implement complete SSA construction with proper phi handling

**Tasks**:
1. **Variable Collection and Analysis**
   ```rust
   fn collect_all_variables(&self, function: &TypedFunction) -> Vec<SymbolId> {
       let mut variables = Vec::new();
       
       // Add parameters
       for param in &function.parameters {
           variables.push(param.symbol_id);
       }
       
       // Traverse function body for local variables
       self.collect_variables_from_statements(&function.body, &mut variables);
       
       variables
   }
   
   fn collect_variables_from_statements(&self, statements: &[TypedStatement], variables: &mut Vec<SymbolId>) {
       for statement in statements {
           match statement {
               TypedStatement::VarDeclaration { symbol_id, .. } => {
                   variables.push(*symbol_id);
               }
               TypedStatement::Block { statements, .. } => {
                   self.collect_variables_from_statements(statements, variables);
               }
               // Handle other statement types...
               _ => {}
           }
       }
   }
   ```

2. **Definition Block Analysis**
   ```rust
   fn find_variable_definition_blocks(&self, cfg: &ControlFlowGraph, function: &TypedFunction, variable: SymbolId, mapping: &TastCfgMapping) -> HashSet<BlockId> {
       let mut def_blocks = HashSet::new();
       
       // Check each block for definitions of this variable
       for &block_id in cfg.blocks.keys() {
           if self.block_defines_variable(block_id, variable, mapping, function) {
               def_blocks.insert(block_id);
           }
       }
       
       def_blocks
   }
   
   fn block_defines_variable(&self, block_id: BlockId, variable: SymbolId, mapping: &TastCfgMapping, function: &TypedFunction) -> bool {
       let statements = mapping.get_statements_in_block(block_id);
       for &stmt_location in statements {
           if let Ok(statement) = self.get_statement_from_location(stmt_location, function) {
               if self.statement_defines_variable(statement, variable) {
                   return true;
               }
           }
       }
       false
   }
   ```

3. **Phi Operand Completion** (Most Critical)
   ```rust
   fn complete_phi_operands(&mut self, cfg: &ControlFlowGraph) -> Result<(), GraphConstructionError> {
       // Get all incomplete phi nodes
       let incomplete_phis: Vec<_> = self.ssa_state.incomplete_phis.clone().into_iter().collect();
       
       for (phi_node_id, incomplete_phi) in incomplete_phis {
           let mut phi_operands = Vec::new();
           
           // For each predecessor of this phi's block
           for &pred_block in &incomplete_phi.predecessor_blocks {
               // Get the SSA variable for this symbol at the end of the predecessor
               let ssa_var = self.get_variable_at_block_exit(incomplete_phi.symbol_id, pred_block)?;
               
               phi_operands.push(PhiIncoming {
                   value: self.get_node_defining_variable(ssa_var)?,
                   predecessor: pred_block,
               });
           }
           
           // Update the phi node with complete operands
           if let Some(phi_node) = self.dfg.nodes.get_mut(&phi_node_id) {
               if let DataFlowNodeKind::Phi { incoming } = &mut phi_node.kind {
                   *incoming = phi_operands;
               }
           }
       }
       
       // Clear incomplete phi tracking
       self.ssa_state.incomplete_phis.clear();
       Ok(())
   }
   ```

4. **Variable Renaming Algorithm**
   ```rust
   fn rename_variables_in_block(&mut self, block_id: BlockId, dominance_tree: &DominanceTree, mapping: &TastCfgMapping, function: &TypedFunction) -> Result<(), GraphConstructionError> {
       // Save current variable stacks
       let saved_stacks = self.save_variable_stacks();
       
       // Set current block
       let old_block = self.ssa_state.current_block;
       self.ssa_state.current_block = block_id;
       
       // 1. Process phi nodes in this block (update variable stacks)
       self.process_phi_nodes_in_block(block_id)?;
       
       // 2. Process regular statements in this block
       let statements = mapping.get_statements_in_block(block_id);
       for &stmt_location in statements {
           let statement = self.get_statement_from_location(stmt_location, function)?;
           self.build_statement(statement)?;
       }
       
       // 3. Fill phi operands in successor blocks
       self.fill_successor_phi_operands(block_id, cfg)?;
       
       // 4. Recursively process dominated children
       if let Some(children) = dominance_tree.dom_tree_children.get(&block_id) {
           for &child_block in children.clone() {
               self.rename_variables_in_block(child_block, dominance_tree, mapping, function)?;
           }
       }
       
       // 5. Restore variable stacks
       self.restore_variable_stacks(saved_stacks);
       self.ssa_state.current_block = old_block;
       
       Ok(())
   }
   ```

**Acceptance Criteria**: Can build complete SSA form for functions with control flow

### Stage 4: Enhanced Statement Processing (Estimated: 2-3 days)

**Objective**: Handle complex control flow statements properly

**Tasks**:
1. **Control Flow Statement Integration**
2. **Exception Handling Support**
3. **Loop Handling Improvements**

**Acceptance Criteria**: All statement types process correctly

### Stage 5: Validation and Testing (Estimated: 1-2 days)

**Objective**: Comprehensive testing and validation

**Tasks**:
1. **Unit Tests for Each Phase**
2. **Integration Tests with Real Functions**
3. **Performance Testing**
4. **Error Case Handling**

**Acceptance Criteria**: Full test coverage and performance benchmarks

---

## Code Examples and Patterns

### Pattern 1: Safe Variable Stack Management

```rust
fn process_with_stack_management<F>(&mut self, f: F) -> Result<(), GraphConstructionError>
where
    F: FnOnce(&mut Self) -> Result<(), GraphConstructionError>,
{
    let saved_stacks = self.save_variable_stacks();
    let result = f(self);
    self.restore_variable_stacks(saved_stacks);
    result
}
```

### Pattern 2: Statement Location Handling

```rust
fn get_statement_from_location(&self, location: StatementLocation, function: &TypedFunction) -> Result<&TypedStatement, GraphConstructionError> {
    if location.statement_index < function.body.len() {
        Ok(&function.body[location.statement_index])
    } else {
        Err(GraphConstructionError::InternalError {
            message: format!("Invalid statement location: {:?}", location),
        })
    }
}
```

### Pattern 3: Error Propagation

```rust
fn build_with_error_context<T, F>(&mut self, operation: &str, f: F) -> Result<T, GraphConstructionError>
where
    F: FnOnce(&mut Self) -> Result<T, GraphConstructionError>,
{
    f(self).map_err(|e| match e {
        GraphConstructionError::InternalError { message } => {
            GraphConstructionError::InternalError {
                message: format!("{}: {}", operation, message),
            }
        }
        other => other,
    })
}
```

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_simple_function_dfg_construction() {
        // Test basic function with no control flow
    }
    
    #[test]
    fn test_phi_node_placement() {
        // Test phi nodes are placed correctly at dominance frontiers
    }
    
    #[test]
    fn test_ssa_variable_renaming() {
        // Test variables are properly renamed in SSA form
    }
    
    #[test]
    fn test_complex_control_flow() {
        // Test if/while/for statements with multiple blocks
    }
    
    #[test]
    fn test_error_handling() {
        // Test error cases and recovery
    }
}
```

### Integration Tests

```rust
#[test]
fn test_real_haxe_function() {
    let haxe_code = r#"
        function test(x: Int): Int {
            var y = x + 1;
            if (y > 10) {
                y = y * 2;
            }
            return y;
        }
    "#;
    
    // Parse to TAST, build CFG, then DFG
    // Verify SSA form is correct
}
```

### Property-Based Tests

```rust
#[test]
fn test_ssa_properties() {
    // Property: Each SSA variable has exactly one definition
    // Property: All uses have corresponding definitions
    // Property: Phi nodes are only at dominance frontiers
}
```

---

## Performance Considerations

### Memory Usage

| Component | Expected Size | Optimization |
|-----------|---------------|--------------|
| SSA Variables | O(V) where V = variables * avg_definitions | Use arena allocation |
| Phi Nodes | O(φ) where φ = variables * dominance_frontiers | Pre-calculate frontiers |
| Variable Stacks | O(V * max_nesting_depth) | Use persistent data structures |
| Def-Use Chains | O(E) where E = data flow edges | Use compressed representations |

### Time Complexity

| Phase | Complexity | Notes |
|-------|------------|-------|
| Dominance Analysis | O(E α(E,V)) | Already implemented |
| Phi Placement | O(V * DF) | DF = dominance frontier size |
| Variable Renaming | O(V + E) | Linear in nodes and edges |
| Total | O(E α(E,V)) | Dominated by dominance computation |

### Optimization Opportunities

1. **Phi Node Pruning**: Remove unnecessary phi nodes
2. **Copy Propagation**: Eliminate redundant copies during construction
3. **Dead Code Elimination**: Remove unused variables early
4. **Memory Pooling**: Use arena allocators for temporary data

---

## Error Handling and Validation

### Error Categories

1. **Input Validation Errors**
   - Malformed CFG
   - Invalid TAST structure
   - Missing type information

2. **Algorithm Errors**
   - Dominance analysis failures
   - SSA construction errors
   - Internal consistency violations

3. **Resource Errors**
   - Out of memory
   - Stack overflow in recursive algorithms
   - Timeout for large functions

### Validation Checks

```rust
impl DataFlowGraph {
    /// Validate SSA form properties
    pub fn validate_ssa_form(&self) -> Result<(), DfgValidationError> {
        // Check 1: Each SSA variable has exactly one definition
        for ssa_var in self.ssa_variables.values() {
            let def_count = self.count_definitions(ssa_var.id);
            if def_count != 1 {
                return Err(DfgValidationError::MultipleDefinitions {
                    variable: ssa_var.id,
                    count: def_count,
                });
            }
        }
        
        // Check 2: All uses have corresponding definitions
        for node in self.nodes.values() {
            for &operand in &node.operands {
                if !self.nodes.contains_key(&operand) {
                    return Err(DfgValidationError::UndefinedOperand {
                        use_node: node.id,
                        operand,
                    });
                }
            }
        }
        
        // Check 3: Phi nodes are properly formed
        for node in self.nodes.values() {
            if let DataFlowNodeKind::Phi { incoming } = &node.kind {
                if incoming.is_empty() {
                    return Err(DfgValidationError::EmptyPhiNode {
                        node_id: node.id,
                    });
                }
            }
        }
        
        Ok(())
    }
}
```

---

## Integration Points

### With CFG System

```rust
// DFG builder must integrate with:
use crate::semantic_graph::cfg::{ControlFlowGraph, BasicBlock, Terminator};
use crate::semantic_graph::dominance::DominanceTree;
use crate::semantic_graph::tast_cfg_mapping::TastCfgMapping;
```

### With Type System

```rust
// Must properly handle:
use crate::tast::node::{TypedFunction, TypedExpression, TypedStatement};
use crate::tast::{TypeId, SymbolId, SourceLocation};
```

### With Analysis Framework

```rust
// DFG output used by:
// - Ownership analysis
// - Lifetime analysis
// - Optimization passes
// - Dead code elimination
```

---

## Risk Assessment

### High-Risk Areas

1. **Phi Operand Completion** - Most complex part, easy to get wrong
2. **Variable Scoping** - Nested scopes and shadowing
3. **Control Flow Integration** - Exception handling, loops
4. **Memory Management** - Large functions may cause memory issues

### Mitigation Strategies

1. **Incremental Development** - Implement one phase at a time
2. **Comprehensive Testing** - Test each component independently
3. **Reference Implementation** - Study existing SSA construction algorithms
4. **Code Review** - Have SSA experts review the implementation

---

## Conclusion

The DFG builder requires substantial work to become production-ready, but the foundation is solid. The key challenges are:

1. **Core SSA Algorithm Completion** - Especially phi operand handling
2. **CFG Integration** - Proper use of existing CFG and mapping APIs
3. **Error Handling** - Robust error detection and recovery
4. **Performance** - Efficient algorithms for large functions

**Estimated Total Effort**: 2-3 weeks for a single experienced developer

**Success Criteria**: 
- All tests pass
- Can handle real Haxe functions
- Performance acceptable for large codebases
- Integrates cleanly with analysis framework

The implementation should be done incrementally, with each stage building on the previous one and maintaining a working system throughout the development process.
