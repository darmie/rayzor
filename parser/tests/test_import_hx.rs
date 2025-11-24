//! Tests for import.hx special file handling

use parser::parse_haxe_file;

#[test]
fn test_import_hx_basic() {
    let input = r#"
import haxe.macro.Context;
import sys.io.File;
using Lambda;
using StringTools;
"#;

    match parse_haxe_file("import.hx", input, false) {
        Ok(ast) => {
            println!("Successfully parsed import.hx file!");
            
            // Verify no package declaration
            assert!(ast.package.is_none(), "import.hx should not have package declaration");
            
            // Check imports
            assert_eq!(ast.imports.len(), 2);
            assert_eq!(ast.imports[0].path, vec!["haxe", "macro", "Context"]);
            assert_eq!(ast.imports[1].path, vec!["sys", "io", "File"]);
            
            // Check using statements
            assert_eq!(ast.using.len(), 2);
            assert_eq!(ast.using[0].path, vec!["Lambda"]);
            assert_eq!(ast.using[1].path, vec!["StringTools"]);
            
            // Verify no module fields or type declarations
            assert!(ast.module_fields.is_empty(), "import.hx should not have module fields");
            assert!(ast.declarations.is_empty(), "import.hx should not have type declarations");
        }
        Err(e) => {
            panic!("Failed to parse import.hx: {}", e);
        }
    }
}

#[test]
fn test_import_hx_with_conditionals() {
    let input = r#"
#if js
import js.Browser;
import js.html.Window;
#elseif sys
import sys.FileSystem;
import sys.io.File;
#else
import haxe.io.Bytes;
#end

using Lambda;
"#;

    match parse_haxe_file("import.hx", input, false) {
        Ok(ast) => {
            println!("Successfully parsed import.hx with conditionals!");
            
            // The parser should flatten conditional imports for now
            assert!(ast.imports.len() >= 5, "Should have imported all conditional branches");
            assert_eq!(ast.using.len(), 1);
            assert_eq!(ast.using[0].path, vec!["Lambda"]);
        }
        Err(e) => {
            panic!("Failed to parse import.hx with conditionals: {}", e);
        }
    }
}

#[test]
fn test_import_hx_with_wildcard() {
    let input = r#"
import haxe.macro.*;
using StringTools;
"#;

    match parse_haxe_file("import.hx", input, false) {
        Ok(ast) => {
            println!("Successfully parsed import.hx with wildcard import!");
            
            assert_eq!(ast.imports.len(), 1);
            match &ast.imports[0].mode {
                parser::haxe_ast::ImportMode::Wildcard => {
                    assert_eq!(ast.imports[0].path, vec!["haxe", "macro"]);
                }
                _ => panic!("Expected wildcard import mode"),
            }
            
            assert_eq!(ast.using.len(), 1);
        }
        Err(e) => {
            panic!("Failed to parse import.hx with wildcard: {}", e);
        }
    }
}

#[test]
fn test_import_hx_with_alias() {
    let input = r#"
import haxe.macro.Context as Ctx;
import sys.io.File as FileIO;
"#;

    match parse_haxe_file("import.hx", input, false) {
        Ok(ast) => {
            println!("Successfully parsed import.hx with aliases!");
            
            assert_eq!(ast.imports.len(), 2);
            
            match &ast.imports[0].mode {
                parser::haxe_ast::ImportMode::Alias(alias) => {
                    assert_eq!(alias, "Ctx");
                    assert_eq!(ast.imports[0].path, vec!["haxe", "macro", "Context"]);
                }
                _ => panic!("Expected alias import mode"),
            }
            
            match &ast.imports[1].mode {
                parser::haxe_ast::ImportMode::Alias(alias) => {
                    assert_eq!(alias, "FileIO");
                    assert_eq!(ast.imports[1].path, vec!["sys", "io", "File"]);
                }
                _ => panic!("Expected alias import mode"),
            }
        }
        Err(e) => {
            panic!("Failed to parse import.hx with aliases: {}", e);
        }
    }
}

#[test]
fn test_import_hx_empty() {
    let input = r#"
// This is an empty import.hx file
// It should parse successfully
"#;

    match parse_haxe_file("import.hx", input, false) {
        Ok(ast) => {
            println!("Successfully parsed empty import.hx!");
            
            assert!(ast.package.is_none());
            assert!(ast.imports.is_empty());
            assert!(ast.using.is_empty());
            assert!(ast.module_fields.is_empty());
            assert!(ast.declarations.is_empty());
        }
        Err(e) => {
            panic!("Failed to parse empty import.hx: {}", e);
        }
    }
}

#[test]
fn test_import_hx_reject_package() {
    let input = r#"
package com.example;
import haxe.macro.Context;
"#;

    match parse_haxe_file("import.hx", input, false) {
        Ok(_) => {
            panic!("import.hx should not allow package declarations");
        }
        Err(e) => {
            println!("Correctly rejected package in import.hx: {}", e);
        }
    }
}

#[test]
fn test_import_hx_reject_class() {
    let input = r#"
import haxe.macro.Context;
class Test {
    public function new() {}
}
"#;

    match parse_haxe_file("import.hx", input, false) {
        Ok(_) => {
            panic!("import.hx should not allow class declarations");
        }
        Err(e) => {
            println!("Correctly rejected class in import.hx: {}", e);
        }
    }
}

#[test]
fn test_import_hx_reject_module_fields() {
    let input = r#"
import haxe.macro.Context;
var x = 10;
"#;

    match parse_haxe_file("import.hx", input, false) {
        Ok(_) => {
            panic!("import.hx should not allow module fields");
        }
        Err(e) => {
            println!("Correctly rejected module field in import.hx: {}", e);
        }
    }
}

#[test]
fn test_regular_file_not_treated_as_import_hx() {
    let input = r#"
package com.example;

import haxe.macro.Context;

class Test {
    public function new() {}
}
"#;

    match parse_haxe_file("Test.hx", input, false) {
        Ok(ast) => {
            println!("Successfully parsed regular Haxe file!");
            
            // Should have package declaration
            assert!(ast.package.is_some());
            assert_eq!(ast.package.unwrap().path, vec!["com", "example"]);
            
            // Should have imports
            assert_eq!(ast.imports.len(), 1);
            
            // Should have class declaration
            assert_eq!(ast.declarations.len(), 1);
        }
        Err(e) => {
            panic!("Failed to parse regular Haxe file: {}", e);
        }
    }
}

#[test]
fn test_import_hx_detection_various_paths() {
    // Test various path formats that should be detected as import.hx
    let test_cases = vec![
        "import.hx",
        "src/import.hx",
        "com/example/import.hx",
        "/absolute/path/import.hx",
        "./relative/path/import.hx",
        "../parent/import.hx",
    ];
    
    let input = "import haxe.macro.Context;";
    
    for file_name in test_cases {
        match parse_haxe_file(file_name, input, false) {
            Ok(ast) => {
                println!("Successfully parsed {} as import.hx", file_name);
                assert!(ast.package.is_none(), "{} should be treated as import.hx", file_name);
                assert!(ast.declarations.is_empty(), "{} should not allow type declarations", file_name);
            }
            Err(e) => {
                panic!("Failed to parse {} as import.hx: {}", file_name, e);
            }
        }
    }
    
    // Test cases that should NOT be detected as import.hx
    let non_import_cases = vec![
        "Import.hx",  // Capital I
        "import.hx.bak",
        "notimport.hx",
        "import_test.hx",
        "test_import.hx",
    ];
    
    let full_input = r#"
package test;
class Test {}
"#;
    
    for file_name in non_import_cases {
        match parse_haxe_file(file_name, full_input, false) {
            Ok(ast) => {
                println!("Successfully parsed {} as regular file", file_name);
                assert!(ast.package.is_some(), "{} should allow package declaration", file_name);
            }
            Err(e) => {
                panic!("Failed to parse {} as regular file: {}", file_name, e);
            }
        }
    }
}