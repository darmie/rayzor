//! Tests for package-level access control

use crate::tast::{
    ast_lowering::AstLowering,
    enhanced_type_checker::EnhancedTypeChecker,
    namespace::{NamespaceResolver, ImportResolver},
    StringInterner, SymbolTable, ScopeTree, TypeTable, PackageId,
};
use ::parser::Parser;
use diagnostics::{Diagnostics, SourceMap};
use std::rc::Rc;
use std::cell::RefCell;

#[test]
fn test_cross_package_internal_access_denied() {
    let source1 = r#"
package com.example.game;

internal class Player {
    internal var health: Int = 100;
    
    internal function takeDamage(amount: Int): Void {
        health -= amount;
    }
}
"#;

    let source2 = r#"
package com.example.ui;

import com.example.game.Player;

class HealthBar {
    function updateHealth(player: Player): Void {
        // This should fail - accessing internal field from different package
        var currentHealth = player.health;
        
        // This should also fail - calling internal method from different package
        player.takeDamage(10);
    }
}
"#;

    let mut string_interner = StringInterner::new();
    let mut source_map = SourceMap::new();
    let mut diagnostics = Diagnostics::new();
    
    // Parse first file
    let file_id1 = source_map.add_source("Player.hx".to_string(), source1.to_string());
    let mut parser1 = Parser::new(source1, &mut string_interner, file_id1);
    let ast1 = parser1.parse_file().expect("Failed to parse Player.hx");
    
    // Parse second file
    let file_id2 = source_map.add_source("HealthBar.hx".to_string(), source2.to_string());
    let mut parser2 = Parser::new(source2, &mut string_interner, file_id2);
    let ast2 = parser2.parse_file().expect("Failed to parse HealthBar.hx");
    
    // Set up type system
    let type_table = Rc::new(RefCell::new(TypeTable::new(&string_interner)));
    let mut symbol_table = SymbolTable::new();
    let mut scope_tree = ScopeTree::new();
    let mut namespace_resolver = NamespaceResolver::new(&string_interner);
    let mut import_resolver = ImportResolver::new();
    
    // Lower ASTs
    let string_interner_rc = Rc::new(RefCell::new(StringInterner::new()));
    let mut lowerer1 = AstLowering::new(
        &mut string_interner,
        string_interner_rc.clone(),
        &mut symbol_table,
        &type_table,
        &mut scope_tree,
        &mut namespace_resolver,
        &mut import_resolver,
    );
    let typed_file1 = lowerer1.lower_file(&ast1).expect("Failed to lower Player.hx");

    let string_interner_rc2 = Rc::new(RefCell::new(StringInterner::new()));
    let mut lowerer2 = AstLowering::new(
        &mut string_interner,
        string_interner_rc2,
        &mut symbol_table,
        &type_table,
        &mut scope_tree,
        &mut namespace_resolver,
        &mut import_resolver,
    );
    let mut typed_file2 = lowerer2.lower_file(&ast2).expect("Failed to lower HealthBar.hx");
    
    // Type check with enhanced type checker
    let mut type_checker = EnhancedTypeChecker::new(
        &type_table,
        &symbol_table,
        &scope_tree,
        &string_interner,
        &source_map,
        &mut diagnostics,
    );
    
    // Type check second file (should produce errors)
    let result = type_checker.check_file(&mut typed_file2);
    
    // Should succeed but with errors logged
    assert!(result.is_ok());
    
    // Check that we have access errors
    let errors: Vec<_> = diagnostics.errors().collect();
    assert!(errors.len() >= 2, "Expected at least 2 access errors, got {}", errors.len());
    
    // Verify error messages
    let error_messages: Vec<String> = errors.iter()
        .map(|e| e.message.clone())
        .collect();
    
    assert!(error_messages.iter().any(|msg| msg.contains("internal") && msg.contains("health")),
        "Expected error about accessing internal field 'health'");
    assert!(error_messages.iter().any(|msg| msg.contains("internal") && msg.contains("takeDamage")),
        "Expected error about calling internal method 'takeDamage'");
}

#[test]
fn test_same_package_internal_access_allowed() {
    let source = r#"
package com.example.game;

internal class Enemy {
    internal var damage: Int = 10;
}

class Combat {
    function attack(enemy: Enemy): Int {
        // This should work - accessing internal field from same package
        return enemy.damage;
    }
}
"#;

    let mut string_interner = StringInterner::new();
    let mut source_map = SourceMap::new();
    let mut diagnostics = Diagnostics::new();
    
    let file_id = source_map.add_source("Combat.hx".to_string(), source.to_string());
    let mut parser = Parser::new(source, &mut string_interner, file_id);
    let ast = parser.parse_file().expect("Failed to parse");
    
    let type_table = Rc::new(RefCell::new(TypeTable::new(&string_interner)));
    let mut symbol_table = SymbolTable::new();
    let mut scope_tree = ScopeTree::new();
    let mut namespace_resolver = NamespaceResolver::new(&string_interner);
    let mut import_resolver = ImportResolver::new();
    
    let string_interner_rc = Rc::new(RefCell::new(StringInterner::new()));
    let mut lowerer = AstLowering::new(
        &mut string_interner,
        string_interner_rc,
        &mut symbol_table,
        &type_table,
        &mut scope_tree,
        &mut namespace_resolver,
        &mut import_resolver,
    );
    let mut typed_file = lowerer.lower_file(&ast).expect("Failed to lower");
    
    let mut type_checker = EnhancedTypeChecker::new(
        &type_table,
        &symbol_table,
        &scope_tree,
        &string_interner,
        &source_map,
        &mut diagnostics,
    );
    
    let result = type_checker.check_file(&mut typed_file);
    assert!(result.is_ok());
    
    // Should have no errors
    let errors: Vec<_> = diagnostics.errors().collect();
    assert_eq!(errors.len(), 0, "Expected no errors for same-package internal access");
}

#[test]
fn test_sub_package_access() {
    let source1 = r#"
package com.example;

internal class Base {
    internal var value: Int = 42;
}
"#;

    let source2 = r#"
package com.example.sub;

import com.example.Base;

class Derived {
    function getValue(base: Base): Int {
        // This should work or not depending on sub-package rules
        // Currently treating sub-packages as having internal access
        return base.value;
    }
}
"#;

    let mut string_interner = StringInterner::new();
    let mut source_map = SourceMap::new();
    let mut diagnostics = Diagnostics::new();
    
    let file_id1 = source_map.add_source("Base.hx".to_string(), source1.to_string());
    let mut parser1 = Parser::new(source1, &mut string_interner, file_id1);
    let ast1 = parser1.parse_file().expect("Failed to parse Base.hx");
    
    let file_id2 = source_map.add_source("Derived.hx".to_string(), source2.to_string());
    let mut parser2 = Parser::new(source2, &mut string_interner, file_id2);
    let ast2 = parser2.parse_file().expect("Failed to parse Derived.hx");
    
    let type_table = Rc::new(RefCell::new(TypeTable::new(&string_interner)));
    let mut symbol_table = SymbolTable::new();
    let mut scope_tree = ScopeTree::new();
    let mut namespace_resolver = NamespaceResolver::new(&string_interner);
    let mut import_resolver = ImportResolver::new();
    
    let string_interner_rc = Rc::new(RefCell::new(StringInterner::new()));
    let mut lowerer1 = AstLowering::new(
        &mut string_interner,
        string_interner_rc.clone(),
        &mut symbol_table,
        &type_table,
        &mut scope_tree,
        &mut namespace_resolver,
        &mut import_resolver,
    );
    let typed_file1 = lowerer1.lower_file(&ast1).expect("Failed to lower Base.hx");

    let string_interner_rc2 = Rc::new(RefCell::new(StringInterner::new()));
    let mut lowerer2 = AstLowering::new(
        &mut string_interner,
        string_interner_rc2,
        &mut symbol_table,
        &type_table,
        &mut scope_tree,
        &mut namespace_resolver,
        &mut import_resolver,
    );
    let mut typed_file2 = lowerer2.lower_file(&ast2).expect("Failed to lower Derived.hx");
    
    let mut type_checker = EnhancedTypeChecker::new(
        &type_table,
        &symbol_table,
        &scope_tree,
        &string_interner,
        &source_map,
        &mut diagnostics,
    );
    
    let result = type_checker.check_file(&mut typed_file2);
    assert!(result.is_ok());
    
    // Current implementation allows sub-package access to internal members
    let errors: Vec<_> = diagnostics.errors().collect();
    assert_eq!(errors.len(), 0, "Sub-packages should have access to parent package internals");
}