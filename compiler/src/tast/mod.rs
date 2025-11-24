pub mod type_arena;
pub mod string_intern;
pub mod id_types;
pub mod symbols;
pub mod scopes;
pub mod core;
pub mod type_checker;
pub mod generic_instantiation;
pub mod constraint_solver;
pub mod generics;
pub mod node;
pub mod node_extensions;
pub mod span_conversion;
pub mod source_extractor;
pub mod class_builder;
pub mod ast_lowering;
pub mod stdlib_loader;
pub mod type_diagnostics;
pub mod type_checking_pipeline;
pub mod namespace;
pub mod package_access;
pub mod type_resolution;
pub mod type_resolution_helpers;
pub mod type_resolution_improvements;
pub mod type_cache;
pub mod symbol_cache;
pub mod effect_analysis;
pub mod control_flow_analysis;
pub mod null_safety_analysis;
pub mod type_flow_guard;
pub mod trait_checker;
pub mod capture_analyzer;
pub mod core_types;
pub mod send_sync_validator;
// pub mod type_inference;

#[cfg(test)]
mod tests;

















pub use type_arena::*;
pub use string_intern::*;
pub use id_types::*;
pub use symbols::*;
pub use scopes::*;
pub use core::{TypeTable, TypeKind, Type, TypeFlags, Variance};
pub use ast_lowering::*;
pub use type_checker::{TypeChecker, TypeCheckError, TypeErrorKind, TypeCheckResult, AccessLevel};
pub use node::{TypedFile, TypedClass, TypedInterface, TypedEnum, TypedExpression, TypedStatement,
               FunctionEffects, AsyncKind, MemoryEffects, ResourceEffects, TypedExpressionKind, MemoryAnnotation, SafetyMode, DerivedTrait,
               PropertyAccessInfo, PropertyAccessor};
pub use namespace::{NamespaceResolver, ImportResolver, PackageId, PackageInfo, QualifiedPath, ImportEntry};
pub use package_access::{PackageAccessValidator, PackageAccessContext, AccessPermission};
pub use type_flow_guard::{TypeFlowGuard, FlowSafetyError, FlowSafetyResults, FlowAnalysisMetrics};
pub use control_flow_analysis::VariableState as FlowVariableState;
// pub use type_resolution::*;
// pub use type_inference::*;
