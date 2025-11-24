//! Plugin system for runtime function registration
//!
//! This module provides a trait-based plugin architecture that allows to register runtime
//! functions with the compiler backend.

/// Trait for runtime plugins
///
/// Implement this trait to provide
/// runtime functions that can be called from compiled code.
pub trait RuntimePlugin: Send + Sync {
    /// Returns the name of this plugin (e.g., "haxe", "stdlib")
    fn name(&self) -> &str;

    /// Returns the runtime symbols this plugin provides
    ///
    /// Each symbol is a tuple of (symbol_name, function_pointer).
    /// The function pointer must point to a valid function with C calling convention.
    fn runtime_symbols(&self) -> Vec<(&'static str, *const u8)>;

    /// Called when the plugin is loaded (optional)
    fn on_load(&self) -> Result<(), String> {
        Ok(())
    }

    /// Called when the plugin is unloaded (optional)
    fn on_unload(&self) -> Result<(), String> {
        Ok(())
    }
}

/// Registry for managing runtime plugins
pub struct PluginRegistry {
    plugins: Vec<Box<dyn RuntimePlugin>>,
}

impl PluginRegistry {
    /// Create a new empty plugin registry
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    /// Register a runtime plugin
    pub fn register(&mut self, plugin: Box<dyn RuntimePlugin>) -> Result<(), String> {
        let name = plugin.name();

        // Check for duplicate registrations
        if self.plugins.iter().any(|p| p.name() == name) {
            return Err(format!("Plugin '{}' is already registered", name));
        }

        // Call the plugin's load hook
        plugin.on_load()?;

        self.plugins.push(plugin);
        Ok(())
    }

    /// Get all runtime symbols from all registered plugins
    pub fn collect_symbols(&self) -> Vec<(&'static str, *const u8)> {
        let mut symbols = Vec::new();
        for plugin in &self.plugins {
            symbols.extend(plugin.runtime_symbols());
        }
        symbols
    }

    /// List all registered plugin names
    pub fn list_plugins(&self) -> Vec<&str> {
        self.plugins.iter().map(|p| p.name()).collect()
    }

    /// Get a specific plugin by name
    pub fn get_plugin(&self, name: &str) -> Option<&dyn RuntimePlugin> {
        self.plugins.iter()
            .find(|p| p.name() == name)
            .map(|p| &**p as &dyn RuntimePlugin)
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}
