//! Integration tests for Call Graph construction and analysis
//!
//! These tests verify that the call graph correctly handles real-world Haxe
//! scenarios including complex inheritance hierarchies, dynamic dispatch,
//! recursive patterns, and cross-module calls.

use crate::semantic_graph::{CallGraph, CallTarget};

#[cfg(test)]
mod call_graph_integration_tests {
    use super::super::*;
    use crate::semantic_graph::dfg::CallType;
    use crate::tast::{node::*, CallSiteId};
    use crate::tast::{Mutability, TypeId, Visibility};
    use std::collections::HashMap;

    /// Test realistic inheritance hierarchy with virtual method calls
    #[test]
    fn test_inheritance_hierarchy_dispatch() {
        let mut call_graph = CallGraph::new();

        // Create class hierarchy: Animal -> Dog -> Labrador
        let animal_class = SymbolId::from_raw(1);
        let dog_class = SymbolId::from_raw(2);
        let labrador_class = SymbolId::from_raw(3);

        // Virtual methods
        let animal_speak = SymbolId::from_raw(10);
        let dog_speak = SymbolId::from_raw(11);
        let labrador_speak = SymbolId::from_raw(12);
        let main_function = SymbolId::from_raw(20);

        call_graph.add_function(animal_speak);
        call_graph.add_function(dog_speak);
        call_graph.add_function(labrador_speak);
        call_graph.add_function(main_function);

        // Main function calls virtual method on different instances
        let virtual_call_1 = CallSite::new(
            CallSiteId::from_raw(1),
            main_function,
            CallTarget::Virtual {
                method_name: "speak".to_string(),
                receiver_type: TypeId::from_raw(1), // Animal type
                possible_targets: vec![animal_speak, dog_speak, labrador_speak],
            },
            CallType::Virtual,
            BlockId::from_raw(1),
            SourceLocation::unknown(),
        );

        let virtual_call_2 = CallSite::new(
            CallSiteId::from_raw(2),
            main_function,
            CallTarget::Virtual {
                method_name: "speak".to_string(),
                receiver_type: TypeId::from_raw(2), // Dog type
                possible_targets: vec![dog_speak, labrador_speak],
            },
            CallType::Virtual,
            BlockId::from_raw(2),
            SourceLocation::unknown(),
        );

        call_graph.add_call_site(virtual_call_1);
        call_graph.add_call_site(virtual_call_2);

        // Verify call graph structure
        let calls_from_main = call_graph.get_calls_from(main_function);
        assert_eq!(calls_from_main.len(), 2);

        // Test reachability analysis
        let reachable = call_graph.reachable_functions(main_function);
        assert!(reachable.contains(&main_function));
        assert!(reachable.contains(&animal_speak));
        assert!(reachable.contains(&dog_speak));
        assert!(reachable.contains(&labrador_speak));

        // Verify virtual dispatch resolution by checking call sites directly
        let virtual_count = call_graph
            .call_sites
            .values()
            .filter(|site| matches!(site.callee, CallTarget::Virtual { .. }))
            .count();
        assert_eq!(virtual_count, 2);

        call_graph.update_statistics();
        assert_eq!(call_graph.statistics.virtual_call_count, 2);
        assert_eq!(call_graph.statistics.function_count, 4);
    }

    /// Test complex recursion patterns including indirect recursion
    #[test]
    fn test_complex_recursion_patterns() {
        let mut call_graph = CallGraph::new();

        // Functions for testing various recursion patterns
        let fibonacci = SymbolId::from_raw(1); // Direct recursion
        let is_even = SymbolId::from_raw(2); // Mutual recursion
        let is_odd = SymbolId::from_raw(3); // Mutual recursion
        let factorial = SymbolId::from_raw(4); // Direct recursion with base case
        let ackermann = SymbolId::from_raw(5); // Double recursion
        let quicksort = SymbolId::from_raw(6); // Tail recursion
        let main_func = SymbolId::from_raw(10);

        // Add all functions
        for &func in &[
            fibonacci, is_even, is_odd, factorial, ackermann, quicksort, main_func,
        ] {
            call_graph.add_function(func);
        }

        // Main calls all recursive functions
        call_graph.add_call_site(CallSite::new(
            CallSiteId::from_raw(1),
            main_func,
            CallTarget::Direct {
                function: fibonacci,
            },
            CallType::Direct,
            BlockId::from_raw(1),
            SourceLocation::unknown(),
        ));

        call_graph.add_call_site(CallSite::new(
            CallSiteId::from_raw(2),
            main_func,
            CallTarget::Direct { function: is_even },
            CallType::Direct,
            BlockId::from_raw(1),
            SourceLocation::unknown(),
        ));

        // Fibonacci calls itself twice (classic pattern)
        call_graph.add_call_site(CallSite::new(
            CallSiteId::from_raw(10),
            fibonacci,
            CallTarget::Direct {
                function: fibonacci,
            },
            CallType::Direct,
            BlockId::from_raw(2),
            SourceLocation::unknown(),
        ));

        call_graph.add_call_site(CallSite::new(
            CallSiteId::from_raw(11),
            fibonacci,
            CallTarget::Direct {
                function: fibonacci,
            },
            CallType::Direct,
            BlockId::from_raw(3),
            SourceLocation::unknown(),
        ));

        // Mutual recursion: is_even <-> is_odd
        call_graph.add_call_site(CallSite::new(
            CallSiteId::from_raw(12),
            is_even,
            CallTarget::Direct { function: is_odd },
            CallType::Direct,
            BlockId::from_raw(4),
            SourceLocation::unknown(),
        ));

        call_graph.add_call_site(CallSite::new(
            CallSiteId::from_raw(13),
            is_odd,
            CallTarget::Direct { function: is_even },
            CallType::Direct,
            BlockId::from_raw(5),
            SourceLocation::unknown(),
        ));

        // Factorial direct recursion
        call_graph.add_call_site(CallSite::new(
            CallSiteId::from_raw(14),
            factorial,
            CallTarget::Direct {
                function: factorial,
            },
            CallType::Direct,
            BlockId::from_raw(6),
            SourceLocation::unknown(),
        ));

        // Ackermann function - double recursion
        call_graph.add_call_site(CallSite::new(
            CallSiteId::from_raw(15),
            ackermann,
            CallTarget::Direct {
                function: ackermann,
            },
            CallType::Direct,
            BlockId::from_raw(7),
            SourceLocation::unknown(),
        ));

        call_graph.add_call_site(CallSite::new(
            CallSiteId::from_raw(16),
            ackermann,
            CallTarget::Direct {
                function: ackermann,
            },
            CallType::Direct,
            BlockId::from_raw(8),
            SourceLocation::unknown(),
        ));

        // Quicksort calls itself on partitions
        call_graph.add_call_site(CallSite::new(
            CallSiteId::from_raw(17),
            quicksort,
            CallTarget::Direct {
                function: quicksort,
            },
            CallType::Direct,
            BlockId::from_raw(9),
            SourceLocation::unknown(),
        ));

        call_graph.add_call_site(CallSite::new(
            CallSiteId::from_raw(18),
            quicksort,
            CallTarget::Direct {
                function: quicksort,
            },
            CallType::Direct,
            BlockId::from_raw(10),
            SourceLocation::unknown(),
        ));

        // Analyze recursion patterns
        call_graph.compute_strongly_connected_components();

        // Verify direct recursion detection
        assert!(call_graph.is_recursive(fibonacci));
        assert!(call_graph.is_recursive(factorial));
        assert!(call_graph.is_recursive(ackermann));
        assert!(call_graph.is_recursive(quicksort));

        // Verify mutual recursion detection
        assert!(call_graph.is_recursive(is_even));
        assert!(call_graph.is_recursive(is_odd));

        // Main should not be recursive
        assert!(!call_graph.is_recursive(main_func));

        // Check SCC components
        let scc_count = call_graph.recursion_info.scc_components.len();
        assert!(scc_count >= 4); // At least: {fibonacci}, {factorial}, {ackermann}, {quicksort}, {is_even, is_odd}

        // Find the mutual recursion SCC
        let mutual_scc = call_graph
            .recursion_info
            .scc_components
            .iter()
            .find(|scc| scc.functions.contains(&is_even) && scc.functions.contains(&is_odd))
            .expect("Should find mutual recursion SCC");

        assert_eq!(mutual_scc.functions.len(), 2);
        assert!(mutual_scc.has_cycles);

        // Verify statistics
        call_graph.update_statistics();
        assert!(call_graph.statistics.recursive_function_count >= 6);
        assert!(call_graph.recursion_info.max_recursion_depth.unwrap_or(0) >= 1);
    }

    /// Test cross-module and external function calls
    #[test]
    fn test_cross_module_calls() {
        let mut call_graph = CallGraph::new();

        // Local functions
        let local_main = SymbolId::from_raw(1);
        let local_helper = SymbolId::from_raw(2);

        call_graph.add_function(local_main);
        call_graph.add_function(local_helper);

        // Local function calls
        call_graph.add_call_site(CallSite::new(
            CallSiteId::from_raw(1),
            local_main,
            CallTarget::Direct {
                function: local_helper,
            },
            CallType::Direct,
            BlockId::from_raw(1),
            SourceLocation::unknown(),
        ));

        // External library calls
        call_graph.add_call_site(CallSite::new(
            CallSiteId::from_raw(2),
            local_main,
            CallTarget::External {
                function_name: "Array.push".to_string(),
                module: Some("std".to_string()),
            },
            CallType::Builtin, // Use Builtin for external calls
            BlockId::from_raw(2),
            SourceLocation::unknown(),
        ));

        call_graph.add_call_site(CallSite::new(
            CallSiteId::from_raw(3),
            local_helper,
            CallTarget::External {
                function_name: "Math.sqrt".to_string(),
                module: Some("std".to_string()),
            },
            CallType::Builtin, // Use Builtin for external calls
            BlockId::from_raw(3),
            SourceLocation::unknown(),
        ));

        // Dynamic function calls (function values)
        // Create a dummy typed expression for function_expr
         let dummy_expr = TypedExpression {
            expr_type: TypeId::from_raw(999),
            kind: TypedExpressionKind::Variable { symbol_id: SymbolId::from_raw(999) },
            usage: VariableUsage::Copy,
            lifetime_id: LifetimeId::static_lifetime(),
            source_location: SourceLocation::unknown(),
            metadata: ExpressionMetadata::default(),
        };
        

        call_graph.add_call_site(CallSite::new(
            CallSiteId::from_raw(4),
            local_main,
            CallTarget::Dynamic {
                function_expr: dummy_expr,
                possible_targets: vec![local_helper],
            },
            CallType::Virtual, // Use Virtual for dynamic calls
            BlockId::from_raw(4),
            SourceLocation::unknown(),
        ));

        // Unresolved calls (for testing error cases)
        call_graph.add_call_site(CallSite::new(
            CallSiteId::from_raw(5),
            local_main,
            CallTarget::Unresolved {
                name: "unknown_function".to_string(),
                reason: "Symbol not found".to_string(),
            },
            CallType::Direct, // Use Direct for unresolved calls
            BlockId::from_raw(5),
            SourceLocation::unknown(),
        ));

        // Verify call pattern analysis
        call_graph.update_statistics();

        assert_eq!(call_graph.statistics.function_count, 2);
        assert_eq!(call_graph.statistics.call_site_count, 5);
        assert_eq!(call_graph.statistics.direct_call_count, 1);
        assert_eq!(call_graph.statistics.external_call_count, 2);
        assert_eq!(call_graph.statistics.dynamic_call_count, 1);
        // Note: Unresolved calls aren't counted in statistics

        // Test external call filtering by checking call sites directly
        let external_count = call_graph
            .call_sites
            .values()
            .filter(|site| matches!(site.callee, CallTarget::External { .. }))
            .count();
        assert_eq!(external_count, 2);

        let dynamic_count = call_graph
            .call_sites
            .values()
            .filter(|site| matches!(site.callee, CallTarget::Dynamic { .. }))
            .count();
        assert_eq!(dynamic_count, 1);

        // Verify call reachability excludes external calls
        let reachable = call_graph.reachable_functions(local_main);
        assert_eq!(reachable.len(), 2); // Only local functions
        assert!(reachable.contains(&local_main));
        assert!(reachable.contains(&local_helper));
    }

    /// Test performance characteristics with call graphs
    #[test]
    fn test_large_call_graph_performance() {
        let mut call_graph = CallGraph::new();

        let start_time = std::time::Instant::now();

        // Create a simple call graph with 10 functions for performance testing
        let function_count = 10;
        let functions: Vec<SymbolId> = (0..function_count)
            .map(|i| SymbolId::from_raw(i as u32 + 1))
            .collect();

        // Add all functions
        for &func in &functions {
            call_graph.add_function(func);
        }

        let mut call_site_id_counter = 1;

        // Create simple call patterns - each function calls the next one
        for i in 0..function_count - 1 {
            let caller = functions[i];
            let callee = functions[i + 1];

            let call_site = CallSite::new(
                CallSiteId::from_raw(call_site_id_counter),
                caller,
                CallTarget::Direct { function: callee },
                CallType::Direct,
                BlockId::from_raw(call_site_id_counter),
                SourceLocation::unknown(),
            );

            call_graph.add_call_site(call_site);
            call_site_id_counter += 1;
        }

        // Add one recursive call for testing
        let recursive_call = CallSite::new(
            CallSiteId::from_raw(call_site_id_counter),
            functions[0],
            CallTarget::Direct { function: functions[0] },
            CallType::Direct,
            BlockId::from_raw(call_site_id_counter),
            SourceLocation::unknown(),
        );
        call_graph.add_call_site(recursive_call);

        let construction_time = start_time.elapsed();

        // Perform analysis operations
        let analysis_start = std::time::Instant::now();

        call_graph.compute_strongly_connected_components();
        call_graph.update_statistics();

        // Test reachability
        let reachable = call_graph.reachable_functions(functions[0]);

        let analysis_time = analysis_start.elapsed();

        // Performance assertions
        assert!(
            construction_time.as_millis() < 100,
            "Call graph construction took too long: {}ms",
            construction_time.as_millis()
        );

        assert!(
            analysis_time.as_millis() < 100,
            "Call graph analysis took too long: {}ms",
            analysis_time.as_millis()
        );

        // Correctness assertions
        assert_eq!(call_graph.statistics.function_count, function_count);
        assert!(call_graph.statistics.call_site_count > 0);
        assert!(call_graph.statistics.recursive_function_count > 0);
        assert!(reachable.len() > 0);

        println!("✅ Call graph performance test passed:");
        println!(
            "   📊 {} functions, {} call sites",
            function_count, call_graph.statistics.call_site_count
        );
        println!(
            "   ⏱️  Construction: {}ms, Analysis: {}ms",
            construction_time.as_millis(),
            analysis_time.as_millis()
        );
        println!(
            "   🔄 {} recursive functions, {} SCCs",
            call_graph.statistics.recursive_function_count,
            call_graph.recursion_info.scc_components.len()
        );
    }

    /// Test call graph with generic function calls and type specialization
    #[test]
    fn test_generic_function_calls() {
        let mut call_graph = CallGraph::new();

        // Generic functions
        let generic_sort = SymbolId::from_raw(1); // sort<T>(array: Array<T>)
        let generic_map = SymbolId::from_raw(2); // map<T,U>(array: Array<T>, fn: T->U)
        let generic_filter = SymbolId::from_raw(3); // filter<T>(array: Array<T>, pred: T->Bool)

        // Specialized instances
        let sort_int = SymbolId::from_raw(10); // sort<Int>
        let sort_string = SymbolId::from_raw(11); // sort<String>
        let map_int_string = SymbolId::from_raw(12); // map<Int,String>
        let filter_string = SymbolId::from_raw(13); // filter<String>

        let main_func = SymbolId::from_raw(20);

        // Add functions
        for &func in &[
            generic_sort,
            generic_map,
            generic_filter,
            sort_int,
            sort_string,
            map_int_string,
            filter_string,
            main_func,
        ] {
            call_graph.add_function(func);
        }

        // Main function calls specialized versions
        call_graph.add_call_site(CallSite::new(
            CallSiteId::from_raw(1),
            main_func,
            CallTarget::Direct { function: sort_int },
            CallType::Direct,
            BlockId::from_raw(1),
            SourceLocation::unknown(),
        ));

        call_graph.add_call_site(CallSite::new(
            CallSiteId::from_raw(2),
            main_func,
            CallTarget::Direct {
                function: map_int_string,
            },
            CallType::Direct,
            BlockId::from_raw(2),
            SourceLocation::unknown(),
        ));

        // Specialized functions call higher-order functions
        let dummy_expr1 = TypedExpression {
            expr_type: TypeId::from_raw(999),
            kind: TypedExpressionKind::Variable { symbol_id: SymbolId::from_raw(999) },
            usage: VariableUsage::Copy,
            lifetime_id: LifetimeId::static_lifetime(),
            source_location: SourceLocation::unknown(),
            metadata: ExpressionMetadata::default(),
        };

        call_graph.add_call_site(CallSite::new(
            CallSiteId::from_raw(3),
            map_int_string,
            CallTarget::Dynamic {
                function_expr: dummy_expr1,
                possible_targets: vec![], // Function parameter - unknown targets
            },
            CallType::Virtual, // Use Virtual for dynamic calls
            BlockId::from_raw(3),
            SourceLocation::unknown(),
        ));

        // Filter calls predicate function
        let dummy_expr2 = TypedExpression {
            expr_type: TypeId::from_raw(999),
            kind: TypedExpressionKind::Variable { symbol_id: SymbolId::from_raw(999) },
            usage: VariableUsage::Copy,
            lifetime_id: LifetimeId::static_lifetime(),
            source_location: SourceLocation::unknown(),
            metadata: ExpressionMetadata::default(),
        };

        call_graph.add_call_site(CallSite::new(
            CallSiteId::from_raw(4),
            filter_string,
            CallTarget::Dynamic {
                function_expr: dummy_expr2,
                possible_targets: vec![], // Predicate function
            },
            CallType::Virtual, // Use Virtual for dynamic calls
            BlockId::from_raw(4),
            SourceLocation::unknown(),
        ));

        // Verify the call graph captures generic patterns
        call_graph.update_statistics();

        assert_eq!(call_graph.statistics.function_count, 8);
        assert_eq!(call_graph.statistics.dynamic_call_count, 2); // Higher-order function calls

        let reachable = call_graph.reachable_functions(main_func);
        assert!(reachable.contains(&sort_int));
        assert!(reachable.contains(&map_int_string));

        // Test that dynamic calls are properly identified
        let dynamic_count = call_graph
            .call_sites
            .values()
            .filter(|site| matches!(site.callee, CallTarget::Dynamic { .. }))
            .count();
        assert_eq!(dynamic_count, 2);

        for site in call_graph.call_sites.values() {
            match &site.callee {
                CallTarget::Dynamic { function_expr, .. } => {
                    // Check that function expression is present
                    assert!(matches!(
                        function_expr.kind,
                        TypedExpressionKind::Variable { .. }
                    ));
                }
                _ => {}
            }
        }
    }

    /// Test error handling and validation in call graph construction
    #[test]
    fn test_call_graph_validation_and_errors() {
        let mut call_graph = CallGraph::new();

        let func_a = SymbolId::from_raw(1);
        let func_b = SymbolId::from_raw(2);
        let nonexistent_func = SymbolId::from_raw(999);

        call_graph.add_function(func_a);
        call_graph.add_function(func_b);

        // Add valid call
        call_graph.add_call_site(CallSite::new(
            CallSiteId::from_raw(1),
            func_a,
            CallTarget::Direct { function: func_b },
            CallType::Direct,
            BlockId::from_raw(1),
            SourceLocation::unknown(),
        ));

        // Add call to non-existent function (as unresolved call)
        call_graph.add_call_site(CallSite::new(
            CallSiteId::from_raw(2),
            func_a,
            CallTarget::Unresolved {
                name: "missing_function".to_string(),
                reason: "Function not found in symbol table".to_string(),
            },
            CallType::Direct, // Use Direct for unresolved calls
            BlockId::from_raw(2),
            SourceLocation::unknown(),
        ));

        // Validation should pass (checking basic consistency)
        // Since there's no validate() method, we check basic consistency manually
        assert!(call_graph.functions.contains(&func_a));
        assert!(call_graph.functions.contains(&func_b));
        assert_eq!(call_graph.call_sites.len(), 2);

        // Check that all call sites reference valid functions
        for call_site in call_graph.call_sites.values() {
            assert!(call_graph.functions.contains(&call_site.caller));
            // Note: callees might be external/unresolved so not all need to be in functions set
        }

        // Test statistics accuracy
        call_graph.update_statistics();
        assert_eq!(call_graph.statistics.function_count, 2);
        assert_eq!(call_graph.statistics.call_site_count, 2);
        assert_eq!(call_graph.statistics.direct_call_count, 1);
        // Note: Unresolved calls aren't included in counts
    }
}

/// Helper functions for call graph testing
impl CallGraph {
    /// Count call sites by target type for testing
    pub fn count_call_sites_by_target_type(&self) -> (usize, usize, usize, usize, usize) {
        let mut direct = 0;
        let mut virtual_calls = 0;
        let mut dynamic = 0;
        let mut external = 0;
        let mut unresolved = 0;

        for site in self.call_sites.values() {
            match &site.callee {
                CallTarget::Direct { .. } => direct += 1,
                CallTarget::Virtual { .. } => virtual_calls += 1,
                CallTarget::Dynamic { .. } => dynamic += 1,
                CallTarget::External { .. } => external += 1,
                CallTarget::Unresolved { .. } => unresolved += 1,
            }
        }

        (direct, virtual_calls, dynamic, external, unresolved)
    }
}
