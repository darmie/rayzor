//! Analysis Engine and Passes for Semantic Graphs
//!
//! This module provides the unified analysis infrastructure that orchestrates
//! lifetime checking, ownership analysis, escape analysis, and dead code detection
//! across your semantic graph representations.

pub mod analysis_engine;
pub mod deadcode_analyzer;
pub mod escape_analyzer;
pub mod global_lifetime_constraints;
pub mod lifetime_analyzer;
pub mod lifetime_solver;
pub mod ownership_analyzer;

mod analysis_engine_integration_test;
mod deadcode_analyzer_test;
mod escape_analyzer_test;
mod field_constraint_test;
mod lifetime_analysis_test;
mod ownership_analyzer_test;
