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
// pub mod type_resolution;
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
pub use node::{TypedFile, TypedClass, TypedInterface, TypedEnum, TypedExpression, TypedStatement};
pub use namespace::{NamespaceResolver, ImportResolver, PackageId, PackageInfo, QualifiedPath, ImportEntry};
// pub use type_resolution::*;
// pub use type_inference::*;
