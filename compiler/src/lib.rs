pub mod tast;
pub mod semantic_graph;
pub mod pipeline;
pub mod error_codes;
pub mod ir;
pub mod codegen;
pub mod compilation;
pub mod dependency_graph;
pub mod hxml;
pub mod stdlib;  // MIR-based standard library

// Re-export plugin system from separate crate (avoids cyclic dependency)
pub use rayzor_plugin as plugin;

// #[cfg(test)]
// mod pipeline_test;

pub mod pipeline_validation;
