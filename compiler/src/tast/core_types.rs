//! Core type identification for Rayzor stdlib
//!
//! This module provides utilities for identifying Rayzor standard library types
//! by their fully qualified paths. This is essential for applying special
//! validation rules (e.g., Send/Sync constraints on Thread::spawn).

use crate::tast::core::TypeKind;
use crate::tast::{SymbolId, SymbolTable, TypeId, TypeTable};
use std::cell::RefCell;
use std::rc::Rc;

/// Fully qualified paths for Rayzor core types
pub struct CoreTypePaths {
    // Concurrency types
    pub thread: &'static str,
    pub channel: &'static str,
    pub mutex: &'static str,
    pub arc: &'static str,

    // Memory types
    pub rc: &'static str,
    pub box_type: &'static str,

    // Async types
    pub promise: &'static str,
    pub future: &'static str,
}

impl CoreTypePaths {
    /// Get the standard core type paths
    pub fn standard() -> Self {
        Self {
            // Concurrency
            thread: "rayzor.concurrent.Thread",
            channel: "rayzor.concurrent.Channel",
            mutex: "rayzor.concurrent.Mutex",
            arc: "rayzor.concurrent.Arc",

            // Memory
            rc: "rayzor.memory.Rc",
            box_type: "rayzor.memory.Box",

            // Async
            promise: "rayzor.async.Promise",
            future: "rayzor.async.Future",
        }
    }
}

/// Core type identifier for Rayzor stdlib types
pub struct CoreTypeChecker<'a> {
    type_table: &'a Rc<RefCell<TypeTable>>,
    symbol_table: &'a SymbolTable,
    paths: CoreTypePaths,
}

impl<'a> CoreTypeChecker<'a> {
    /// Create a new core type checker
    pub fn new(type_table: &'a Rc<RefCell<TypeTable>>, symbol_table: &'a SymbolTable) -> Self {
        Self {
            type_table,
            symbol_table,
            paths: CoreTypePaths::standard(),
        }
    }

    /// Check if a type is rayzor.concurrent.Thread<T>
    pub fn is_thread(&self, type_id: TypeId) -> bool {
        self.is_core_type(type_id, self.paths.thread)
    }

    /// Check if a type is rayzor.concurrent.Channel<T>
    pub fn is_channel(&self, type_id: TypeId) -> bool {
        self.is_core_type(type_id, self.paths.channel)
    }

    /// Check if a type is rayzor.concurrent.Arc<T>
    pub fn is_arc(&self, type_id: TypeId) -> bool {
        self.is_core_type(type_id, self.paths.arc)
    }

    /// Check if a type is rayzor.concurrent.Mutex<T>
    pub fn is_mutex(&self, type_id: TypeId) -> bool {
        self.is_core_type(type_id, self.paths.mutex)
    }

    /// Check if a type is rayzor.memory.Rc<T>
    pub fn is_rc(&self, type_id: TypeId) -> bool {
        self.is_core_type(type_id, self.paths.rc)
    }

    /// Check if a type is rayzor.memory.Box<T>
    pub fn is_box(&self, type_id: TypeId) -> bool {
        self.is_core_type(type_id, self.paths.box_type)
    }

    /// Check if a type is rayzor.async.Promise<T>
    pub fn is_promise(&self, type_id: TypeId) -> bool {
        self.is_core_type(type_id, self.paths.promise)
    }

    /// Check if a type is rayzor.async.Future<T>
    pub fn is_future(&self, type_id: TypeId) -> bool {
        self.is_core_type(type_id, self.paths.future)
    }

    /// Get the type argument from a generic type (e.g., T from Thread<T>)
    ///
    /// Returns None if the type is not generic or has no type arguments.
    pub fn get_type_argument(&self, type_id: TypeId) -> Option<TypeId> {
        let type_table = self.type_table.borrow();
        let type_info = type_table.get(type_id)?;

        match &type_info.kind {
            TypeKind::Class { type_args, .. }
            | TypeKind::Interface { type_args, .. }
            | TypeKind::Enum { type_args, .. } => type_args.first().copied(),
            TypeKind::GenericInstance { type_args, .. } => type_args.first().copied(),
            _ => None,
        }
    }

    /// Check if a type matches a fully qualified path
    fn is_core_type(&self, type_id: TypeId, expected_path: &str) -> bool {
        let type_table = self.type_table.borrow();
        let type_info = match type_table.get(type_id) {
            Some(t) => t,
            None => return false,
        };

        // Get the symbol ID from the type
        let symbol_id = match &type_info.kind {
            TypeKind::Class { symbol_id, .. }
            | TypeKind::Interface { symbol_id, .. }
            | TypeKind::Enum { symbol_id, .. }
            | TypeKind::Abstract { symbol_id, .. } => *symbol_id,
            TypeKind::GenericInstance { base_type, .. } => {
                // For generic instances, check the base type
                return self.is_core_type(*base_type, expected_path);
            }
            _ => return false,
        };

        // Get the fully qualified name from the symbol
        self.check_symbol_path(symbol_id, expected_path)
    }

    /// Check if a symbol's fully qualified path matches the expected path
    fn check_symbol_path(&self, symbol_id: SymbolId, expected_path: &str) -> bool {
        // Get the symbol
        let symbol = match self.symbol_table.get_symbol(symbol_id) {
            Some(s) => s,
            None => return false,
        };

        // Get the fully qualified name
        let qualified_name_interned = match symbol.qualified_name {
            Some(qn) => qn,
            None => return false,
        };

        // Get the string from the interner
        let type_table = self.type_table.borrow();
        let fqn = match type_table.get_string(qualified_name_interned) {
            Some(s) => s,
            None => return false,
        };

        // Compare with expected path
        // Handle both dot notation (rayzor.concurrent.Thread) and
        // double-colon notation (rayzor::concurrent::Thread)
        let normalized_fqn = fqn.replace("::", ".");
        let normalized_expected = expected_path.replace("::", ".");

        normalized_fqn == normalized_expected
    }

    /// Validate Thread::spawn - all captured variables must be Send
    ///
    /// Returns the closure type ID if this is a Thread::spawn call
    pub fn get_thread_spawn_closure(
        &self,
        call_expr: &crate::tast::node::TypedExpression,
    ) -> Option<TypeId> {
        use crate::tast::node::TypedExpressionKind;

        // Check if this is a static method call
        if let TypedExpressionKind::StaticMethodCall {
            class_symbol,
            method_symbol,
            arguments,
            ..
        } = &call_expr.kind
        {
            // Check if the class is Thread
            if !self.check_symbol_path(*class_symbol, self.paths.thread) {
                return None;
            }

            // Check if the method is "spawn"
            let method_name = self.symbol_table.get_symbol(*method_symbol)?;
            let method_name_str = {
                let type_table = self.type_table.borrow();
                type_table.get_string(method_name.name)?.to_string()
            };
            if method_name_str != "spawn" {
                return None;
            }

            // Get the first argument (the closure)
            let closure_arg = arguments.first()?;
            Some(closure_arg.expr_type)
        } else {
            None
        }
    }

    /// Validate Channel::new - T must be Send
    ///
    /// Returns the channel element type if this is a Channel::new call
    pub fn get_channel_element_type(&self, type_id: TypeId) -> Option<TypeId> {
        if self.is_channel(type_id) {
            self.get_type_argument(type_id)
        } else {
            None
        }
    }

    /// Validate Arc::new - T must be Send + Sync
    ///
    /// Returns the Arc element type if this is an Arc type
    pub fn get_arc_element_type(&self, type_id: TypeId) -> Option<TypeId> {
        if self.is_arc(type_id) {
            self.get_type_argument(type_id)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_core_paths() {
        let paths = CoreTypePaths::standard();
        assert_eq!(paths.thread, "rayzor.concurrent.Thread");
        assert_eq!(paths.channel, "rayzor.concurrent.Channel");
        assert_eq!(paths.arc, "rayzor.concurrent.Arc");
    }

    // TODO: Add integration tests with actual TypeTable and SymbolTable
}
