//! Simple working tests for enhanced type checking components
//!
//! These tests verify that the enhanced type checking modules can be instantiated
//! and perform basic operations without full integration.

use crate::tast::{
    enhanced_type_checker::EnhancedTypeChecker,
    control_flow_analysis::{ControlFlowAnalyzer, ControlFlowGraph},
    null_safety_analysis::{NullSafetyAnalyzer, NullState},
    effect_analysis::EffectAnalyzer,
    core::TypeTable,
    SymbolTable, StringInterner,
};
use std::cell::RefCell;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enhanced_type_checker_creation() {
        let symbol_table = SymbolTable::new();
        let type_table = std::rc::Rc::new(RefCell::new(TypeTable::new()));

        let _checker = EnhancedTypeChecker::new(&symbol_table, &type_table);

        println!("✅ Enhanced type checker can be created");
    }

    #[test]
    fn test_control_flow_analyzer_creation() {
        let _analyzer = ControlFlowAnalyzer::new();

        println!("✅ Control flow analyzer can be created");
    }

    #[test]
    fn test_control_flow_graph_creation() {
        let mut cfg = ControlFlowGraph::new();

        // Test basic operations
        cfg.entry_block = 0;
        cfg.exit_blocks.push(1);

        println!("✅ Control flow graph can be created and modified");
    }

    #[test]
    fn test_null_safety_analyzer_creation() {
        let symbol_table = SymbolTable::new();
        let type_table = RefCell::new(TypeTable::new());
        let cfg = ControlFlowGraph::new();

        let _analyzer = NullSafetyAnalyzer::new(&type_table, &symbol_table, &cfg);

        println!("✅ Null safety analyzer can be created");
    }

    #[test]
    fn test_effect_analyzer_creation() {
        let symbol_table = SymbolTable::new();
        let type_table = std::rc::Rc::new(RefCell::new(TypeTable::new()));

        let _analyzer = EffectAnalyzer::new(&symbol_table, &type_table);

        println!("✅ Effect analyzer can be created");
    }

    #[test]
    fn test_null_state_enum() {
        // Test that NullState enum works as expected
        let state1 = NullState::Null;
        let state2 = NullState::NotNull;
        let state3 = NullState::MaybeNull;
        let state4 = NullState::Uninitialized;

        assert_eq!(state1, NullState::Null);
        assert_ne!(state1, state2);
        assert_ne!(state2, state3);
        assert_ne!(state3, state4);

        println!("✅ Null state enum works correctly");
    }

    #[test]
    fn test_enhanced_type_checker_with_empty_file() {
        let symbol_table = SymbolTable::new();
        let type_table = std::rc::Rc::new(RefCell::new(TypeTable::new()));
        let string_interner = std::rc::Rc::new(RefCell::new(StringInterner::new()));

        let mut checker = EnhancedTypeChecker::new(&symbol_table, &type_table);
        let file = crate::tast::node::TypedFile::new(string_interner);

        // This should not crash
        let results = checker.check_file(&file);

        // Basic sanity checks
        assert_eq!(results.metrics.functions_analyzed, 0);
        assert!(results.metrics.control_flow_time_us >= 0);

        println!("✅ Enhanced type checker can analyze empty file");
    }

    #[test]
    fn test_basic_integration_exists() {
        // This test verifies that all the pieces can be instantiated together
        let symbol_table = SymbolTable::new();
        let type_table = std::rc::Rc::new(RefCell::new(TypeTable::new()));

        let _enhanced_checker = EnhancedTypeChecker::new(&symbol_table, &type_table);
        let _cfg_analyzer = ControlFlowAnalyzer::new();
        let _effect_analyzer = EffectAnalyzer::new(&symbol_table, &type_table);

        let cfg = ControlFlowGraph::new();
        let _null_analyzer = NullSafetyAnalyzer::new(&type_table, &symbol_table, &cfg);

        println!("✅ All enhanced type checking components can be instantiated together");
    }
}