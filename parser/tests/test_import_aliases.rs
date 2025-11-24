use parser::haxe_parser::parse_haxe_file;
use parser::haxe_ast::{ImportMode, HaxeFile};

#[test]
fn test_import_alias_parsing() {
    let input = r#"
package com.example;

import com.example.VeryLongClassName as Short;
import haxe.macro.Context as Ctx;
import sys.io.File as F;

class Main {
    public static function main() {
        var obj = new Short();
        var file = F.read("test.txt");
        Ctx.typeof(obj);
    }
}
"#;

    let result = parse_haxe_file("test.hx", input, false);
    assert!(result.is_ok(), "Failed to parse: {:?}", result);
    
    let file = result.unwrap();
    
    // Check that we have 3 imports
    assert_eq!(file.imports.len(), 3);
    
    // Check first import alias
    assert_eq!(file.imports[0].path, vec!["com", "example", "VeryLongClassName"]);
    match &file.imports[0].mode {
        ImportMode::Alias(alias) => assert_eq!(alias, "Short"),
        _ => panic!("Expected alias import mode"),
    }
    
    // Check second import alias
    assert_eq!(file.imports[1].path, vec!["haxe", "macro", "Context"]);
    match &file.imports[1].mode {
        ImportMode::Alias(alias) => assert_eq!(alias, "Ctx"),
        _ => panic!("Expected alias import mode"),
    }
    
    // Check third import alias
    assert_eq!(file.imports[2].path, vec!["sys", "io", "File"]);
    match &file.imports[2].mode {
        ImportMode::Alias(alias) => assert_eq!(alias, "F"),
        _ => panic!("Expected alias import mode"),
    }
}

#[test]
fn test_mixed_import_modes() {
    let input = r#"
import com.example.Normal;
import com.example.WithAlias as Alias;
import com.example.Module.*;
import com.example.Type.staticField;
import another.package.LongTypeName as Short;

class Test {}
"#;

    let result = parse_haxe_file("test.hx", input, false);
    assert!(result.is_ok(), "Failed to parse: {:?}", result);
    
    let file = result.unwrap();
    assert_eq!(file.imports.len(), 5);
    
    // Check normal import
    assert_eq!(file.imports[0].path, vec!["com", "example", "Normal"]);
    assert_eq!(file.imports[0].mode, ImportMode::Normal);
    
    // Check alias import
    assert_eq!(file.imports[1].path, vec!["com", "example", "WithAlias"]);
    match &file.imports[1].mode {
        ImportMode::Alias(alias) => assert_eq!(alias, "Alias"),
        _ => panic!("Expected alias import mode"),
    }
    
    // Check wildcard import
    assert_eq!(file.imports[2].path, vec!["com", "example", "Module"]);
    assert_eq!(file.imports[2].mode, ImportMode::Wildcard);
    
    // Check field import
    assert_eq!(file.imports[3].path, vec!["com", "example", "Type"]);
    match &file.imports[3].mode {
        ImportMode::Field(field) => assert_eq!(field, "staticField"),
        _ => panic!("Expected field import mode"),
    }
    
    // Check another alias import
    assert_eq!(file.imports[4].path, vec!["another", "package", "LongTypeName"]);
    match &file.imports[4].mode {
        ImportMode::Alias(alias) => assert_eq!(alias, "Short"),
        _ => panic!("Expected alias import mode"),
    }
}

#[test]
fn test_import_alias_with_metadata() {
    let input = r#"
@:keep
import com.example.MyClass as MC;

@:native("CustomMain")
class Main {
    function new() {
        var c = new MC();
    }
}
"#;

    let result = parse_haxe_file("test.hx", input, false);
    assert!(result.is_ok(), "Failed to parse: {:?}", result);
    
    let file = result.unwrap();
    assert_eq!(file.imports.len(), 1);
    
    assert_eq!(file.imports[0].path, vec!["com", "example", "MyClass"]);
    match &file.imports[0].mode {
        ImportMode::Alias(alias) => assert_eq!(alias, "MC"),
        _ => panic!("Expected alias import mode"),
    }
}

#[test]
fn test_import_alias_edge_cases() {
    // Test that keywords can be used in import paths but not as aliases
    let input1 = r#"
import haxe.macro.Type as T;
import my.package.class as Class;  // 'class' in path is ok
"#;

    let result = parse_haxe_file("test.hx", input1, false);
    assert!(result.is_ok(), "Failed to parse import with keyword in path");
    
    // Test multiple aliases in sequence
    let input2 = r#"
import a.b.C as D;
import e.f.G as H;
import i.j.K as L;
"#;

    let result = parse_haxe_file("test.hx", input2, false);
    assert!(result.is_ok(), "Failed to parse multiple aliases");
    let file = result.unwrap();
    assert_eq!(file.imports.len(), 3);
}