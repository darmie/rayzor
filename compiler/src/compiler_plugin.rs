//! Compiler Plugin Architecture
//!
//! This module provides a trait-based plugin system for extending the compiler's
//! stdlib method mapping system. This is separate from the `rayzor_plugin` crate
//! which handles JIT runtime symbol registration.
//!
//! **Compiler plugins** (this module):
//! - Register method mappings (Haxe method -> runtime function)
//! - Declare extern function signatures in MIR
//! - Build MIR wrapper functions
//!
//! **Runtime plugins** (`rayzor_plugin` crate):
//! - Provide function pointers for JIT linking
//! - Handle symbol resolution at runtime
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────────────┐
//! │ CompilerCompilerPluginRegistry│ ── manages compiler-level plugins
//! └──────────┬───────────┘
//!            │
//!     ┌──────┴──────┐
//!     │             │
//! ┌───▼────┐  ┌────▼────┐
//! │ Builtin │  │  HDLL   │ ── implements CompilerPlugin trait
//! │ Plugin  │  │ Plugin  │
//! └─────────┘  └─────────┘
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use compiler::compiler_plugin::{CompilerPlugin, CompilerCompilerPluginRegistry};
//!
//! // Create a registry with plugins
//! let mut registry = CompilerCompilerPluginRegistry::new();
//! registry.register(Box::new(BuiltinPlugin));
//!
//! // Optionally add external plugins (e.g., HDLL)
//! // registry.register(Box::new(HdllPlugin::load("ssl.hdll")?));
//!
//! // Build combined stdlib mapping from all plugins
//! let mapping = registry.build_combined_mapping();
//! ```

use crate::ir::mir_builder::MirBuilder;
use crate::stdlib::{MethodSignature, RuntimeFunctionCall, StdlibMapping};

/// Trait for compiler plugins that provide stdlib method mappings.
///
/// Plugins can be:
/// - **BuiltinPlugin**: The default rayzor stdlib (Rust runtime)
/// - **HdllPlugin**: Hashlink dynamic libraries (.hdll files)
/// - **Custom**: User-defined plugins for specialized libraries
///
/// # Lifecycle
///
/// 1. Plugin is registered with `CompilerPluginRegistry`
/// 2. During compilation, `method_mappings()` provides Haxe → runtime function mapping
/// 3. During MIR building, `declare_externs()` registers extern function signatures
/// 4. Optionally, `build_mir_wrappers()` creates MIR wrapper functions
pub trait CompilerPlugin: Send + Sync {
    /// Returns the plugin name for debugging and identification
    fn name(&self) -> &str;

    /// Returns method mappings from Haxe stdlib methods to runtime functions.
    ///
    /// These mappings tell the compiler how to translate Haxe method calls
    /// (like `str.charAt(0)`) to runtime function calls (like `haxe_string_char_at`).
    fn method_mappings(&self) -> Vec<(MethodSignature, RuntimeFunctionCall)>;

    /// Declare extern function signatures in the MIR builder.
    ///
    /// This is called during stdlib MIR construction to register extern functions
    /// that will be linked at JIT compilation time.
    fn declare_externs(&self, builder: &mut MirBuilder);

    /// Build MIR wrapper functions that forward to extern implementations.
    ///
    /// MIR wrappers are useful when the Haxe calling convention differs from
    /// the C calling convention of the runtime function.
    fn build_mir_wrappers(&self, builder: &mut MirBuilder);

    /// Returns the priority of this plugin (higher = loaded later, can override).
    ///
    /// Default priority is 0. Built-in plugins should use 0, while user plugins
    /// can use higher values to override built-in mappings.
    fn priority(&self) -> i32 {
        0
    }
}

/// Registry for managing multiple runtime plugins.
///
/// The registry aggregates mappings from all registered plugins and provides
/// a unified view for the compiler.
pub struct CompilerPluginRegistry {
    plugins: Vec<Box<dyn CompilerPlugin>>,
}

impl CompilerPluginRegistry {
    /// Create a new empty plugin registry.
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    /// Register a plugin with the registry.
    ///
    /// Plugins are stored in registration order. When building mappings,
    /// later plugins can override earlier ones based on priority.
    pub fn register(&mut self, plugin: Box<dyn CompilerPlugin>) {
        self.plugins.push(plugin);
    }

    /// Build a combined `StdlibMapping` from all registered plugins.
    ///
    /// Mappings are combined in priority order (lower priority first),
    /// so higher-priority plugins can override lower-priority ones.
    pub fn build_combined_mapping(&self) -> StdlibMapping {
        let mut mapping = StdlibMapping::new();

        // Sort plugins by priority (stable sort preserves registration order for equal priorities)
        let mut sorted_plugins: Vec<&Box<dyn CompilerPlugin>> = self.plugins.iter().collect();
        sorted_plugins.sort_by_key(|p| p.priority());

        // Collect all mappings from plugins
        for plugin in &sorted_plugins {
            for (sig, call) in plugin.method_mappings() {
                mapping.register_mapping(sig, call);
            }
        }

        mapping
    }

    /// Declare all extern functions from all plugins.
    ///
    /// This should be called during stdlib MIR construction.
    pub fn declare_all_externs(&self, builder: &mut MirBuilder) {
        for plugin in &self.plugins {
            plugin.declare_externs(builder);
        }
    }

    /// Build all MIR wrappers from all plugins.
    ///
    /// This should be called during stdlib MIR construction.
    pub fn build_all_mir_wrappers(&self, builder: &mut MirBuilder) {
        for plugin in &self.plugins {
            plugin.build_mir_wrappers(builder);
        }
    }

    /// Get the names of all registered plugins.
    pub fn plugin_names(&self) -> Vec<&str> {
        self.plugins.iter().map(|p| p.name()).collect()
    }

    /// Get the number of registered plugins.
    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }
}

impl Default for CompilerPluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestPlugin {
        name: String,
        priority: i32,
    }

    impl CompilerPlugin for TestPlugin {
        fn name(&self) -> &str {
            &self.name
        }

        fn method_mappings(&self) -> Vec<(MethodSignature, RuntimeFunctionCall)> {
            vec![]
        }

        fn declare_externs(&self, _builder: &mut MirBuilder) {
            // No-op for test
        }

        fn build_mir_wrappers(&self, _builder: &mut MirBuilder) {
            // No-op for test
        }

        fn priority(&self) -> i32 {
            self.priority
        }
    }

    #[test]
    fn test_plugin_registry() {
        let mut registry = CompilerPluginRegistry::new();
        assert!(registry.is_empty());

        registry.register(Box::new(TestPlugin {
            name: "test1".to_string(),
            priority: 0,
        }));
        registry.register(Box::new(TestPlugin {
            name: "test2".to_string(),
            priority: 10,
        }));

        assert_eq!(registry.len(), 2);
        assert_eq!(registry.plugin_names(), vec!["test1", "test2"]);
    }
}
