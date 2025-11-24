//! Integration tests for Ownership Graph construction and memory safety analysis
//!
//! These tests verify that the ownership graph correctly handles real-world Haxe
//! memory safety scenarios including borrowing patterns, move semantics,
//! lifetime constraints, and memory safety violations.

use crate::semantic_graph::{ analysis::lifetime_analyzer::LifetimeConstraint, OwnershipGraph};

#[cfg(test)]
mod ownership_graph_integration_tests {
    use super::super::*;
    use crate::semantic_graph::analysis::lifetime_analyzer::{EqualityReason, LifetimeConstraint, OutlivesReason};
    use crate::tast::{collections::*, CallSiteId, DataFlowNodeId};
    use crate::tast::{ScopeId, TypeId};

    /// Test basic variable ownership and lifetime tracking
    #[test]
    fn test_basic_ownership_tracking() {
        let mut graph = OwnershipGraph::new();
        
        // Create variables in different scopes
        let main_scope = ScopeId::from_raw(1);
        let inner_scope = ScopeId::from_raw(2);
        
        let var_a = SymbolId::from_raw(1); // String
        let var_b = SymbolId::from_raw(2); // Int
        let var_c = SymbolId::from_raw(3); // Array<String>
        
        let string_type = TypeId::from_raw(1);
        let int_type = TypeId::from_raw(2);
        let array_type = TypeId::from_raw(3);
        
        // Add variables to graph
        graph.add_variable(var_a, string_type, main_scope);
        graph.add_variable(var_b, int_type, main_scope);
        graph.add_variable(var_c, array_type, inner_scope);
        
        // Verify initial state
        assert_eq!(graph.get_ownership_kind(var_a), Some(OwnershipKind::Owned));
        assert_eq!(graph.get_ownership_kind(var_b), Some(OwnershipKind::Owned));
        assert_eq!(graph.get_ownership_kind(var_c), Some(OwnershipKind::Owned));
        
        assert!(!graph.is_moved(var_a));
        assert!(!graph.is_moved(var_b));
        assert!(!graph.is_moved(var_c));
        
        // Check lifetimes are properly assigned
        assert!(graph.get_variable_lifetime(var_a).is_some());
        assert!(graph.get_variable_lifetime(var_b).is_some());
        assert!(graph.get_variable_lifetime(var_c).is_some());
        
        // Variables in different scopes should have different lifetimes
        let lifetime_a = graph.get_variable_lifetime(var_a).unwrap();
        let lifetime_c = graph.get_variable_lifetime(var_c).unwrap();
        assert_ne!(lifetime_a, lifetime_c);
        
        graph.update_statistics();
        assert_eq!(graph.statistics.variable_count, 3);
        assert_eq!(graph.statistics.borrow_count, 0);
        assert_eq!(graph.statistics.move_count, 0);
    }
    
    /// Test borrowing relationships and aliasing detection
    #[test]
    fn test_borrowing_and_aliasing_violations() {
        let mut graph = OwnershipGraph::new();
        
        let scope = ScopeId::from_raw(1);
        let string_type = TypeId::from_raw(1);
        
        // Create variables
        let original = SymbolId::from_raw(1);     // let data = "hello"
        let borrow1 = SymbolId::from_raw(2);      // let ref1 = &data
        let borrow2 = SymbolId::from_raw(3);      // let ref2 = &data
        let mut_borrow = SymbolId::from_raw(4);   // let mut_ref = &mut data
        
        graph.add_variable(original, string_type, scope);
        graph.add_variable(borrow1, string_type, scope);
        graph.add_variable(borrow2, string_type, scope);
        graph.add_variable(mut_borrow, string_type, scope);
        
        // Create borrowing relationships
        let location = SourceLocation::unknown();
        
        // Two immutable borrows - should be allowed
        graph.add_borrow(borrow1, original, BorrowType::Immutable, scope, location.clone());
        graph.add_borrow(borrow2, original, BorrowType::Immutable, scope, location.clone());
        
        // Add mutable borrow - should create aliasing violation
        graph.add_borrow(mut_borrow, original, BorrowType::Mutable, scope, location.clone());
        
        // Check ownership states
        assert_eq!(graph.get_ownership_kind(borrow1), Some(OwnershipKind::Borrowed));
        assert_eq!(graph.get_ownership_kind(borrow2), Some(OwnershipKind::Borrowed));
        assert_eq!(graph.get_ownership_kind(mut_borrow), Some(OwnershipKind::BorrowedMut));
        
        // Check borrowing relationships
        let borrowers = graph.get_borrowers(original);
        assert_eq!(borrowers.len(), 3);
        assert!(borrowers.contains(&borrow1));
        assert!(borrowers.contains(&borrow2));
        assert!(borrowers.contains(&mut_borrow));
        
        // Check for aliasing violations
        let violations = graph.has_aliasing_violations();
        assert_eq!(violations.len(), 1);
        
        match &violations[0] {
            OwnershipViolation::AliasingViolation { variable, .. } => {
                assert_eq!(*variable, original);
            }
            _ => panic!("Expected aliasing violation"),
        }
        
        graph.update_statistics();
        assert_eq!(graph.statistics.borrow_count, 3);
        assert!(graph.statistics.constraint_count > 0); // Lifetime constraints added
    }
    
    /// Test move semantics and use-after-move detection
    #[test]
    fn test_move_semantics() {
        let mut graph = OwnershipGraph::new();
        
        let scope = ScopeId::from_raw(1);
        let vec_type = TypeId::from_raw(1);
        
        // Create variables
        let source = SymbolId::from_raw(1);      // let vec1 = Vec::new()
        let destination = SymbolId::from_raw(2); // let vec2 = vec1  (move)
        let param = SymbolId::from_raw(3);       // function(vec2)   (move into call)
        
        graph.add_variable(source, vec_type, scope);
        graph.add_variable(destination, vec_type, scope);
        graph.add_variable(param, vec_type, scope);
        
        let location = SourceLocation::unknown();
        
        // Move from source to destination
        graph.add_move(source, Some(destination), location.clone(), MoveType::Explicit);
        
        // Move destination into function call
        graph.add_move(destination, None, location.clone(), MoveType::FunctionCall);
        
        // Check move states
        assert!(graph.is_moved(source));
        assert!(graph.is_moved(destination));
        assert!(!graph.is_moved(param));
        
        assert_eq!(graph.get_ownership_kind(source), Some(OwnershipKind::Moved));
        assert_eq!(graph.get_ownership_kind(destination), Some(OwnershipKind::Moved));
        
        // Check use-after-move detection
        let violations = graph.check_use_after_move();
        assert_eq!(violations.len(), 2); // Both source and destination moved
        
        for violation in &violations {
            match violation {
                OwnershipViolation::UseAfterMove { variable, move_type, .. } => {
                    if *variable == source {
                        assert_eq!(*move_type, MoveType::Explicit);
                    } else if *variable == destination {
                        assert_eq!(*move_type, MoveType::FunctionCall);
                    } else {
                        panic!("Unexpected variable in use-after-move: {:?}", variable);
                    }
                }
                _ => panic!("Expected use-after-move violation"),
            }
        }
        
        graph.update_statistics();
        assert_eq!(graph.statistics.move_count, 2);
    }
    
    /// Test complex lifetime constraints and relationships
    #[test]
    fn test_lifetime_constraints() {
        let mut graph = OwnershipGraph::new();
        
        let global_scope = ScopeId::from_raw(0);
        let function_scope = ScopeId::from_raw(1);
        let block_scope = ScopeId::from_raw(2);
        let inner_scope = ScopeId::from_raw(3);
        
        let string_type = TypeId::from_raw(1);
        
        // Create variables in nested scopes
        let global_var = SymbolId::from_raw(1);   // Global string
        let func_param = SymbolId::from_raw(2);   // Function parameter
        let local_var = SymbolId::from_raw(3);    // Local variable
        let inner_var = SymbolId::from_raw(4);    // Inner scope variable
        
        graph.add_variable(global_var, string_type, global_scope);
        graph.add_variable(func_param, string_type, function_scope);
        graph.add_variable(local_var, string_type, block_scope);
        graph.add_variable(inner_var, string_type, inner_scope);
        
        let location = SourceLocation::unknown();
        
        // Create borrowing relationships that establish lifetime constraints
        // inner_var borrows from local_var
        graph.add_borrow(inner_var, local_var, BorrowType::Immutable, inner_scope, location.clone());
        
        // local_var borrows from func_param
        graph.add_borrow(local_var, func_param, BorrowType::Immutable, block_scope, location.clone());
        
        // func_param borrows from global_var
        graph.add_borrow(func_param, global_var, BorrowType::Immutable, function_scope, location.clone());
        
        // Add explicit lifetime constraints
        let global_lifetime = graph.get_variable_lifetime(global_var).unwrap();
        let func_lifetime = graph.get_variable_lifetime(func_param).unwrap();
        let local_lifetime = graph.get_variable_lifetime(local_var).unwrap();
        let inner_lifetime = graph.get_variable_lifetime(inner_var).unwrap();
        
        // Add explicit constraints for function return lifetimes
        graph.add_lifetime_constraint(LifetimeConstraint::Outlives {
            longer: global_lifetime,
            shorter: func_lifetime,
            reason: OutlivesReason::Return,
            location: SourceLocation::unknown() // Todo!
        });
        
        graph.add_lifetime_constraint(LifetimeConstraint::Outlives {
            longer: func_lifetime,
            shorter: local_lifetime,
            reason:OutlivesReason::Parameter,
            location: SourceLocation::unknown() // Todo!
        });
        
        graph.add_lifetime_constraint(LifetimeConstraint::Equal {
            left: local_lifetime,
            right: inner_lifetime,
            reason: EqualityReason::GenericConstraint,
            location: SourceLocation::unknown() // Todo!
        });
        
        // Verify constraint relationships
        assert!(graph.lifetime_constraints.len() >= 6); // 3 from borrows + 3 explicit
        
        // Check that all lifetimes are properly tracked
        let global_lt = graph.lifetimes.get(&global_lifetime).unwrap();
        let func_lt = graph.lifetimes.get(&func_lifetime).unwrap();
        let local_lt = graph.lifetimes.get(&local_lifetime).unwrap();
        let inner_lt = graph.lifetimes.get(&inner_lifetime).unwrap();
        
        assert_eq!(global_lt.scope, global_scope);
        assert_eq!(func_lt.scope, function_scope);
        assert_eq!(local_lt.scope, block_scope);
        assert_eq!(inner_lt.scope, inner_scope);
        
        graph.update_statistics();
        assert!(graph.statistics.constraint_count >= 6);
    }
    
    /// Test ownership in generic containers and collections
    #[test]
    fn test_generic_container_ownership() {
        let mut graph = OwnershipGraph::new();
        
        let scope = ScopeId::from_raw(1);
        
        // Types for generic containers
        let array_type = TypeId::from_raw(1);      // Array<String>
        let map_type = TypeId::from_raw(2);        // Map<String, Int>
        let string_type = TypeId::from_raw(3);
        let int_type = TypeId::from_raw(4);
        
        // Container variables
        let array_var = SymbolId::from_raw(1);     // let arr: Array<String>
        let map_var = SymbolId::from_raw(2);       // let map: Map<String, Int>
        
        // Element variables
        let elem1 = SymbolId::from_raw(3);         // let s1 = arr[0]
        let elem2 = SymbolId::from_raw(4);         // let s2 = arr[1]
        let key_var = SymbolId::from_raw(5);       // let key = "hello"
        let value_var = SymbolId::from_raw(6);     // let value = map[key]
        
        graph.add_variable(array_var, array_type, scope);
        graph.add_variable(map_var, map_type, scope);
        graph.add_variable(elem1, string_type, scope);
        graph.add_variable(elem2, string_type, scope);
        graph.add_variable(key_var, string_type, scope);
        graph.add_variable(value_var, int_type, scope);
        
        let location = SourceLocation::unknown();
        
        // Model borrowing from container elements
        // elem1 and elem2 borrow from array
        graph.add_borrow(elem1, array_var, BorrowType::Immutable, scope, location.clone());
        graph.add_borrow(elem2, array_var, BorrowType::Immutable, scope, location.clone());
        
        // value_var borrows from map
        graph.add_borrow(value_var, map_var, BorrowType::Immutable, scope, location.clone());
        
        // Test moving container while elements are borrowed
        graph.add_move(array_var, None, location.clone(), MoveType::FunctionCall);
        
        // This should create violations since elements still reference moved container
        assert!(graph.is_moved(array_var));
        assert_eq!(graph.get_ownership_kind(elem1), Some(OwnershipKind::Borrowed));
        assert_eq!(graph.get_ownership_kind(elem2), Some(OwnershipKind::Borrowed));
        
        // The borrowers should still be tracking the moved container
        let borrowers = graph.get_borrowers(array_var);
        assert_eq!(borrowers.len(), 2);
        assert!(borrowers.contains(&elem1));
        assert!(borrowers.contains(&elem2));
        
        graph.update_statistics();
        assert_eq!(graph.statistics.variable_count, 6);
        assert_eq!(graph.statistics.borrow_count, 3);
        assert_eq!(graph.statistics.move_count, 1);
    }
    
    /// Test ownership across function calls and parameter passing
    #[test]
    fn test_function_call_ownership() {
        let mut graph = OwnershipGraph::new();
        
        let caller_scope = ScopeId::from_raw(1);
        let callee_scope = ScopeId::from_raw(2);
        
        let string_type = TypeId::from_raw(1);
        let int_type = TypeId::from_raw(2);
        
        // Caller variables
        let caller_var = SymbolId::from_raw(1);    // let data = "hello"
        let result_var = SymbolId::from_raw(2);    // let result = process(data)
        
        // Callee parameters and locals
        let param_var = SymbolId::from_raw(3);     // fn process(input: String)
        let local_var = SymbolId::from_raw(4);     // let processed = input.toUpperCase()
        let return_var = SymbolId::from_raw(5);    // return processed
        
        graph.add_variable(caller_var, string_type, caller_scope);
        graph.add_variable(result_var, string_type, caller_scope);
        graph.add_variable(param_var, string_type, callee_scope);
        graph.add_variable(local_var, string_type, callee_scope);
        graph.add_variable(return_var, string_type, callee_scope);
        
        let location = SourceLocation::unknown();
        let call_site = CallSiteId::from_raw(1);
        
        // Model parameter passing as move (for owned types)
        graph.add_move(caller_var, Some(param_var), location.clone(), MoveType::FunctionCall);
        
        // Model local computation as move from parameter (e.g., input.toUpperCase())
        graph.add_move(param_var, Some(local_var), location.clone(), MoveType::Explicit);
        
        // Model return as move
        graph.add_move(local_var, Some(return_var), location.clone(), MoveType::Explicit);
        graph.add_move(return_var, Some(result_var), location.clone(), MoveType::Implicit);
        
        // Add function call lifetime constraints
        let param_lifetime = graph.get_variable_lifetime(param_var).unwrap();
        let return_lifetime = graph.get_variable_lifetime(return_var).unwrap();
        
        graph.add_lifetime_constraint(LifetimeConstraint::CallConstraint {
            call_site,
            caller_lifetimes: vec![param_lifetime],
            callee_lifetimes: vec![return_lifetime],
            location: SourceLocation::unknown() // Todo!
        });
        
        // Verify ownership flow
        assert!(graph.is_moved(caller_var));      // Moved into function
        assert!(graph.is_moved(local_var));       // Moved to return
        assert!(graph.is_moved(return_var));      // Moved to result
        assert!(!graph.is_moved(result_var));     // Final destination
        
        assert_eq!(graph.get_ownership_kind(result_var), Some(OwnershipKind::Owned));
        
        // Check that lifetime constraints were properly set up
        let call_constraints: Vec<_> = graph.lifetime_constraints.iter()
            .filter(|c| matches!(c, LifetimeConstraint::CallConstraint { .. }))
            .collect();
        assert_eq!(call_constraints.len(), 1);
        
        graph.update_statistics();
        assert_eq!(graph.statistics.move_count, 4);
        assert!(graph.statistics.constraint_count > 0);
    }
    
    /// Test error detection and validation in complex scenarios
    #[test]
    fn test_ownership_validation_and_errors() {
        let mut graph = OwnershipGraph::new();
        
        let scope = ScopeId::from_raw(1);
        let string_type = TypeId::from_raw(1);
        
        let var_a = SymbolId::from_raw(1);
        let var_b = SymbolId::from_raw(2);
        let var_c = SymbolId::from_raw(3);
        
        graph.add_variable(var_a, string_type, scope);
        graph.add_variable(var_b, string_type, scope);
        graph.add_variable(var_c, string_type, scope);
        
        let location = SourceLocation::unknown();
        
        // Create complex borrowing and moving pattern
        // var_b borrows from var_a
        graph.add_borrow(var_b, var_a, BorrowType::Immutable, scope, location.clone());
        
        // var_c mutably borrows from var_a (creates aliasing violation)
        graph.add_borrow(var_c, var_a, BorrowType::Mutable, scope, location.clone());
        
        // Move var_a while it's borrowed (creates use-after-move potential)
        graph.add_move(var_a, None, location.clone(), MoveType::FunctionCall);
        
        // Validation should pass (graph structure is valid)
        assert!(graph.validate().is_ok());
        
        // But should detect ownership violations
        let aliasing_violations = graph.has_aliasing_violations();
        assert_eq!(aliasing_violations.len(), 1);
        
        let use_after_move_violations = graph.check_use_after_move();
        assert_eq!(use_after_move_violations.len(), 1);
        
        // Test statistics accuracy
        graph.update_statistics();
        assert_eq!(graph.statistics.variable_count, 3);
        assert_eq!(graph.statistics.borrow_count, 2);
        assert_eq!(graph.statistics.move_count, 1);
        assert_eq!(graph.statistics.constraint_count, 2); // From borrows
    }
    
    /// Test performance with large ownership graphs
    #[test]
    fn test_large_ownership_graph_performance() {
        let mut graph = OwnershipGraph::new();
        
        let start_time = std::time::Instant::now();
        
        // Create a large number of variables with complex ownership patterns
        let variable_count = 1000;
        let scope_count = 50;
        let string_type = TypeId::from_raw(1);
        
        let variables: Vec<SymbolId> = (0..variable_count)
            .map(|i| SymbolId::from_raw(i as u32 + 1))
            .collect();
        
        let scopes: Vec<ScopeId> = (0..scope_count)
            .map(|i| ScopeId::from_raw(i as u32 + 1))
            .collect();
        
        // Add all variables distributed across scopes
        for (i, &var) in variables.iter().enumerate() {
            let scope = scopes[i % scope_count];
            graph.add_variable(var, string_type, scope);
        }
        
        let construction_time = start_time.elapsed();
        
        // Create complex ownership patterns
        let patterns_start = std::time::Instant::now();
        let location = SourceLocation::unknown();
        
        for i in 0..variable_count {
            let var = variables[i];
            
            // Every 10th variable creates borrowing chains
            if i % 10 == 0 && i + 3 < variable_count {
                let scope = scopes[i % scope_count];
                
                // Create borrowing chain: var -> var+1 -> var+2
                graph.add_borrow(variables[i + 1], var, BorrowType::Immutable, scope, location.clone());
                graph.add_borrow(variables[i + 2], variables[i + 1], BorrowType::Immutable, scope, location.clone());
                
                // Some mutable borrows for aliasing testing
                if i % 20 == 0 && i + 3 < variable_count {
                    graph.add_borrow(variables[i + 3], var, BorrowType::Mutable, scope, location.clone());
                }
            }
            
            // Every 15th variable gets moved
            if i % 15 == 0 && i + 1 < variable_count {
                graph.add_move(var, Some(variables[i + 1]), location.clone(), MoveType::Explicit);
            }
        }
        
        let patterns_time = patterns_start.elapsed();
        
        // Perform analysis operations
        let analysis_start = std::time::Instant::now();
        
        // Check for violations
        let aliasing_violations = graph.has_aliasing_violations();
        let move_violations = graph.check_use_after_move();
        
        // Validate graph integrity
        let validation_result = graph.validate();
        assert!(validation_result.is_ok());
        
        // Update statistics
        graph.update_statistics();
        
        let analysis_time = analysis_start.elapsed();
        let total_time = start_time.elapsed();
        
        // Performance assertions
        assert!(construction_time.as_millis() < 500, 
               "Variable creation took too long: {}ms", 
               construction_time.as_millis());
        
        assert!(patterns_time.as_millis() < 1000,
               "Ownership pattern creation took too long: {}ms",
               patterns_time.as_millis());
        
        assert!(analysis_time.as_millis() < 200,
               "Ownership analysis took too long: {}ms",
               analysis_time.as_millis());
        
        // Correctness assertions
        assert_eq!(graph.statistics.variable_count, variable_count);
        assert!(graph.statistics.borrow_count > 0);
        assert!(graph.statistics.move_count > 0);
        assert!(graph.statistics.constraint_count > 0);
        assert!(aliasing_violations.len() > 0); // Should find some aliasing violations
        assert!(move_violations.len() > 0);     // Should find some use-after-move
        
        println!("✅ Large ownership graph performance test passed:");
        println!("   📊 {} variables, {} borrows, {} moves", 
                graph.statistics.variable_count, 
                graph.statistics.borrow_count,
                graph.statistics.move_count);
        println!("   ⏱️  Construction: {}ms, Patterns: {}ms, Analysis: {}ms", 
                construction_time.as_millis(), 
                patterns_time.as_millis(),
                analysis_time.as_millis());
        println!("   🚨 {} aliasing violations, {} use-after-move violations", 
                aliasing_violations.len(), move_violations.len());
        println!("   📏 {} lifetime constraints", graph.statistics.constraint_count);
    }
    
    /// Test ownership interaction with shared and reference-counted types
    #[test]
    fn test_shared_ownership_patterns() {
        let mut graph = OwnershipGraph::new();
        
        let scope = ScopeId::from_raw(1);
        
        // Types for different ownership models
        let rc_type = TypeId::from_raw(1);         // Reference counted type
        let weak_type = TypeId::from_raw(2);       // Weak reference type
        let owned_type = TypeId::from_raw(3);      // Exclusively owned type
        
        // Variables
        let rc_var = SymbolId::from_raw(1);        // let rc_data = Rc::new(data)
        let rc_clone1 = SymbolId::from_raw(2);     // let clone1 = rc_data.clone()
        let rc_clone2 = SymbolId::from_raw(3);     // let clone2 = rc_data.clone()
        let weak_ref = SymbolId::from_raw(4);      // let weak = Rc::downgrade(&rc_data)
        let owned_var = SymbolId::from_raw(5);     // let owned = String::new()
        
        graph.add_variable(rc_var, rc_type, scope);
        graph.add_variable(rc_clone1, rc_type, scope);
        graph.add_variable(rc_clone2, rc_type, scope);
        graph.add_variable(weak_ref, weak_type, scope);
        graph.add_variable(owned_var, owned_type, scope);
        
        // Set appropriate ownership kinds
        if let Some(node) = graph.variables.get_mut(&rc_var) {
            node.ownership_kind = OwnershipKind::Shared;
        }
        if let Some(node) = graph.variables.get_mut(&rc_clone1) {
            node.ownership_kind = OwnershipKind::Shared;
        }
        if let Some(node) = graph.variables.get_mut(&rc_clone2) {
            node.ownership_kind = OwnershipKind::Shared;
        }
        
        let location = SourceLocation::unknown();
        
        // Model shared ownership relationships
        // Clones share ownership with original
        graph.add_borrow(rc_clone1, rc_var, BorrowType::Immutable, scope, location.clone());
        graph.add_borrow(rc_clone2, rc_var, BorrowType::Immutable, scope, location.clone());
        
        // Weak reference doesn't affect ownership
        graph.add_borrow(weak_ref, rc_var, BorrowType::Weak, scope, location.clone());
        
        // Test that shared ownership doesn't create aliasing violations
        let aliasing_violations = graph.has_aliasing_violations();
        // Weak borrows shouldn't count toward aliasing violations
        assert_eq!(aliasing_violations.len(), 0);
        
        // Test moving owned vs shared types
        graph.add_move(owned_var, None, location.clone(), MoveType::FunctionCall);
        
        // Owned variable should be moved, shared variables should not
        assert!(graph.is_moved(owned_var));
        assert!(!graph.is_moved(rc_var));
        assert!(!graph.is_moved(rc_clone1));
        assert!(!graph.is_moved(rc_clone2));
        
        // Check ownership kinds
        assert_eq!(graph.get_ownership_kind(rc_var), Some(OwnershipKind::Shared));
        assert_eq!(graph.get_ownership_kind(rc_clone1), Some(OwnershipKind::Shared));
        assert_eq!(graph.get_ownership_kind(weak_ref), Some(OwnershipKind::Borrowed));
        assert_eq!(graph.get_ownership_kind(owned_var), Some(OwnershipKind::Moved));
        
        graph.update_statistics();
        assert_eq!(graph.statistics.borrow_count, 3);
        assert_eq!(graph.statistics.move_count, 1);
    }
}

/// Helper trait to extend OwnershipGraph for testing
trait OwnershipGraphTestExtensions {
    fn get_lifetime_count(&self) -> usize;
    fn get_constraint_count(&self) -> usize;
    fn has_lifetime_constraint(&self, constraint_type: &str) -> bool;
}

impl OwnershipGraphTestExtensions for OwnershipGraph {
    fn get_lifetime_count(&self) -> usize {
        self.lifetimes.len()
    }
    
    fn get_constraint_count(&self) -> usize {
        self.lifetime_constraints.len()
    }
    
    fn has_lifetime_constraint(&self, constraint_type: &str) -> bool {
        self.lifetime_constraints.iter().any(|constraint| {
            match constraint {
                LifetimeConstraint::Outlives { .. } => constraint_type == "outlives",
                LifetimeConstraint::Equal { .. } => constraint_type == "equal",
                LifetimeConstraint::CallConstraint { .. } => constraint_type == "call",
                LifetimeConstraint::FieldConstraint { .. } => constraint_type == "field",
                LifetimeConstraint::BorrowConstraint { .. } => todo!(),
                LifetimeConstraint::ReturnConstraint { .. } => todo!(),
                LifetimeConstraint::TypeConstraint { variable, required_type, context } => todo!(),
            }
        })
    }
}
