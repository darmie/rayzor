
use std::time::Duration;

use crate::semantic_graph::{
    analysis_engine::AnalysisEngine,
    SemanticGraphs, CallGraph, OwnershipGraph, ControlFlowGraph, DataFlowGraph
};
use crate::tast::{DataFlowNodeId, SourceLocation, SymbolId};


#[test]
fn test_corrected_analysis_pipeline() {
    // Create test semantic graphs using your actual structure
    let graphs = create_test_semantic_graphs();
    
    // Create analysis engine 
    let mut engine = AnalysisEngine::new();
    
    // Run comprehensive analysis
    let results = engine.analyze(&graphs).expect("Analysis should succeed");
    
    // Verify analysis results
    assert!(!results.has_errors(), "Test case should have no errors");
    
    // Extract data we need from results before getting metrics
    let function_count = results.function_lifetime_constraints.len();
    let hir_hints = results.get_hir_hints();
    let dead_code_count = hir_hints.dead_code_regions.len();
    
    // Now we can safely get metrics after using results
    let metrics = engine.metrics();
    assert!(
        metrics.total_time < Duration::from_millis(50),
        "Analysis should complete within 50ms, took {:?}",
        metrics.total_time
    );
    
    println!("✅ Corrected analysis pipeline completed successfully!");
    println!("   Total time: {:?}", metrics.total_time);
    println!("   Functions analyzed: {}", function_count);
    println!("   Dead code regions: {}", dead_code_count);
}

/// Test function-specific analysis 
#[test]
fn test_function_specific_analysis() {
    let graphs = create_test_semantic_graphs();
    let mut engine = AnalysisEngine::new();
    
    // Get a test function ID
    let function_id = SymbolId(1);
    
    // Run analysis on specific function
    let function_results = engine
        .analyze_function(function_id, &graphs)
        .expect("Function analysis should succeed");
    
    assert_eq!(function_results.function_id, function_id);
    assert!(
        function_results.analysis_time < Duration::from_millis(10),
        "Function analysis should be very fast: {:?}",
        function_results.analysis_time
    );
    
    println!("✅ Function-specific analysis test passed!");
    println!("   Function analysis time: {:?}", function_results.analysis_time);
}

/// Test error handling for missing functions
#[test]
fn test_missing_function_error_handling() {
    let graphs = create_empty_semantic_graphs();
    let mut engine = AnalysisEngine::new();
    
    // Try to analyze a function that doesn't exist
    let function_id = SymbolId(999);
    let result = engine.analyze_function(function_id, &graphs);
    
    // Should return FunctionNotFound error
    assert!(result.is_err(), "Should detect missing function");
    
    if let Err(error) = result {
        println!("✅ Error handling test passed: {}", error);
    }
}

// Test data creation functions that work with your actual structures

fn create_test_semantic_graphs() -> SemanticGraphs {
    let mut graphs = SemanticGraphs::new();
    
    // Add a test function
    let function_id = SymbolId(1);
    
    // Create CFG for the function
    let cfg = create_test_cfg();
    graphs.control_flow.insert(function_id, cfg);
    
    // Create DFG for the function
    let dfg = create_test_dfg();
    graphs.data_flow.insert(function_id, dfg);
    
    // Call graph and ownership graph are already initialized
    
    graphs
}

fn create_empty_semantic_graphs() -> SemanticGraphs {
    SemanticGraphs::new()
}

fn create_test_cfg() -> ControlFlowGraph {
    // Create a simple CFG with your actual structure
    let entry_node = crate::tast::BlockId(0);
    let function_id = crate::tast::SymbolId(0);
    let mut cfg = ControlFlowGraph::new(function_id, entry_node);
    
    // Add a basic block
    let basic_block = crate::semantic_graph::cfg::BasicBlock::new(
        entry_node,
        SourceLocation::unknown()
    );
    
    cfg.blocks.insert(entry_node, basic_block);
    cfg
}

fn create_test_dfg() -> DataFlowGraph {
    // Create a simple DFG with your actual structure
    let entry_node = DataFlowNodeId(0);
    DataFlowGraph::new(entry_node)
}

// Additional helper functions for your types
impl SymbolId {
    pub fn new(id: u32) -> Self { SymbolId(id) }
}

impl DataFlowNodeId {
    pub fn new(id: u32) -> Self { DataFlowNodeId(id) }
}

// Import the required types from your codebase

use crate::semantic_graph::cfg::{BasicBlock, Terminator};
