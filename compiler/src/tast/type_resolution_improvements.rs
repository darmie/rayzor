//! Type Resolution Improvements for AST Lowering
//!
//! This module provides specific improvements to replace dynamic_type placeholders
//! with proper type resolution in the AST lowering phase.

use super::{InternedString, StringInterner, SymbolId, SymbolTable, TypeId, TypeKind, TypeTable};
use std::cell::RefCell;

/// Helper functions to improve type resolution in AST lowering
pub struct TypeResolutionImprovements;

impl TypeResolutionImprovements {
    /// Resolve type alias to its target type
    pub fn resolve_type_alias(
        type_table: &RefCell<TypeTable>,
        symbol_table: &SymbolTable,
        alias_symbol: SymbolId,
    ) -> TypeId {
        if let Some(symbol) = symbol_table.get_symbol(alias_symbol) {
            // For type aliases, the symbol's type_id is the alias type itself
            // We need to follow the alias chain
            let type_table_ref = type_table.borrow();
            if let Some(alias_type) = type_table_ref.get(symbol.type_id) {
                if let TypeKind::TypeAlias { target_type, .. } = &alias_type.kind {
                    return *target_type;
                }
            }
        }

        // Fallback to dynamic
        type_table.borrow().dynamic_type()
    }

    /// Resolve abstract type to its underlying type
    pub fn resolve_abstract_type(
        type_table: &RefCell<TypeTable>,
        symbol_table: &SymbolTable,
        abstract_symbol: SymbolId,
    ) -> TypeId {
        if let Some(symbol) = symbol_table.get_symbol(abstract_symbol) {
            let type_table_ref = type_table.borrow();
            if let Some(abstract_type) = type_table_ref.get(symbol.type_id) {
                if let TypeKind::Abstract { underlying, .. } = &abstract_type.kind {
                    if let Some(underlying_type) = underlying {
                        return *underlying_type;
                    }
                }
            }
        }

        // Fallback to dynamic
        type_table.borrow().dynamic_type()
    }

    /// Resolve 'this' type in class context
    pub fn resolve_this_type(
        type_table: &RefCell<TypeTable>,
        symbol_table: &SymbolTable,
        current_class_symbol: Option<SymbolId>,
    ) -> TypeId {
        if let Some(class_symbol) = current_class_symbol {
            if let Some(symbol) = symbol_table.get_symbol(class_symbol) {
                return symbol.type_id;
            }
        }

        // Fallback to dynamic
        type_table.borrow().dynamic_type()
    }

    /// Resolve 'super' type in class context
    pub fn resolve_super_type(
        type_table: &RefCell<TypeTable>,
        symbol_table: &SymbolTable,
        current_class_symbol: Option<SymbolId>,
    ) -> TypeId {
        if let Some(class_symbol) = current_class_symbol {
            if let Some(symbol) = symbol_table.get_symbol(class_symbol) {
                let type_table_ref = type_table.borrow();
                if let Some(class_type) = type_table_ref.get(symbol.type_id) {
                    if let TypeKind::Class { .. } = &class_type.kind {
                        // Get the class's base class
                        // This requires looking up the class metadata
                        // For now, we'll need to store this information separately
                        return type_table_ref.dynamic_type();
                    }
                }
            }
        }

        // Fallback to dynamic
        type_table.borrow().dynamic_type()
    }

    /// Get or create the null type
    pub fn get_null_type(type_table: &RefCell<TypeTable>) -> TypeId {
        // There is no TypeKind::Null in the current type system
        // Use dynamic type as a placeholder for null values
        type_table.borrow().dynamic_type()
    }

    /// Get or create regex type (EReg)
    pub fn get_regex_type(
        type_table: &RefCell<TypeTable>,
        _string_interner: &StringInterner,
    ) -> TypeId {
        // TODO: Implement proper EReg type creation
        // For now, return dynamic as placeholder
        type_table.borrow().dynamic_type()
    }

    /// Create map type Map<K, V>
    pub fn create_map_type(
        type_table: &RefCell<TypeTable>,
        _key_type: TypeId,
        _value_type: TypeId,
    ) -> TypeId {
        // Map types require generic instantiation
        // For now, return dynamic as placeholder
        type_table.borrow().dynamic_type()
    }

    /// Create anonymous object type
    pub fn create_anonymous_object_type(
        type_table: &RefCell<TypeTable>,
        fields: Vec<(InternedString, TypeId)>,
    ) -> TypeId {
        let anonymous_fields: Vec<_> = fields
            .into_iter()
            .map(|(name, type_id)| super::core::AnonymousField {
                name,
                type_id,
                is_public: true,
                optional: false,
            })
            .collect();

        type_table
            .borrow_mut()
            .create_type(super::core::TypeKind::Anonymous {
                fields: anonymous_fields,
            })
    }

    /// Infer object literal type from fields
    pub fn infer_object_literal_type(
        type_table: &RefCell<TypeTable>,
        fields: &[(InternedString, TypeId)],
    ) -> TypeId {
        if fields.is_empty() {
            // Empty object - could be a special type
            type_table.borrow().dynamic_type()
        } else {
            // Would create anonymous type, but for now use dynamic
            type_table.borrow().dynamic_type()
        }
    }

    /// Create union type for conditional/switch expressions
    pub fn create_union_type(type_table: &RefCell<TypeTable>, branch_types: Vec<TypeId>) -> TypeId {
        if branch_types.is_empty() {
            return type_table.borrow().void_type();
        }

        if branch_types.len() == 1 {
            return branch_types[0];
        }

        // Remove duplicates
        let mut unique_types = Vec::new();
        for t in branch_types {
            if !unique_types.contains(&t) {
                unique_types.push(t);
            }
        }

        // Create union type
        type_table.borrow_mut().create_union_type(unique_types)
    }

    /// Resolve field type from class
    pub fn resolve_field_type(
        symbol_table: &SymbolTable,
        type_table: &RefCell<TypeTable>,
        class_fields: &[(InternedString, SymbolId, bool)],
        field_name: InternedString,
    ) -> TypeId {
        // Look for the field in the class fields
        for (name, symbol_id, _is_static) in class_fields {
            if *name == field_name {
                if let Some(field_symbol) = symbol_table.get_symbol(*symbol_id) {
                    return field_symbol.type_id;
                }
            }
        }

        // Not found, return dynamic
        type_table.borrow().dynamic_type()
    }
}
