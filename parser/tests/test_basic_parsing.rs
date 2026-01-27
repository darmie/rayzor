//! Basic parsing tests for the new Haxe parser

use parser::parse_haxe_file;

#[test]
fn test_empty_file() {
    let input = "";
    match parse_haxe_file("test.hx", input, false) {
        Ok(haxe_file) => {
            assert!(haxe_file.package.is_none());
            assert!(haxe_file.imports.is_empty());
            assert!(haxe_file.using.is_empty());
            assert!(haxe_file.declarations.is_empty());
            assert_eq!(haxe_file.span.start, 0);
            assert_eq!(haxe_file.span.end, 0);
        }
        Err(e) => panic!("Empty file should parse successfully, got: {}", e),
    }
}

#[test]
fn test_package_only() {
    let input = "package com.example.test;";
    match parse_haxe_file("test.hx", input, false) {
        Ok(haxe_file) => {
            assert!(haxe_file.package.is_some());
            let package = haxe_file.package.unwrap();
            assert_eq!(package.path, vec!["com", "example", "test"]);
            assert!(haxe_file.imports.is_empty());
            assert!(haxe_file.using.is_empty());
            assert!(haxe_file.declarations.is_empty());
        }
        Err(e) => panic!("Package-only file should parse, got: {}", e),
    }
}

#[test]
fn test_imports_only() {
    let input = r#"
import haxe.Json;
import sys.io.File;
import Map.StringMap;
"#;
    match parse_haxe_file("test.hx", input, false) {
        Ok(haxe_file) => {
            assert!(haxe_file.package.is_none());
            assert_eq!(haxe_file.imports.len(), 3);

            assert_eq!(haxe_file.imports[0].path, vec!["haxe", "Json"]);
            assert_eq!(haxe_file.imports[1].path, vec!["sys", "io", "File"]);
            assert_eq!(haxe_file.imports[2].path, vec!["Map", "StringMap"]);
        }
        Err(e) => panic!("Imports-only file should parse, got: {}", e),
    }
}

#[test]
fn test_using_statements() {
    let input = r#"
using Lambda;
using StringTools;
"#;
    match parse_haxe_file("test.hx", input, false) {
        Ok(haxe_file) => {
            assert_eq!(haxe_file.using.len(), 2);
            assert_eq!(haxe_file.using[0].path, vec!["Lambda"]);
            assert_eq!(haxe_file.using[1].path, vec!["StringTools"]);
        }
        Err(e) => panic!("Using statements should parse, got: {}", e),
    }
}

#[test]
fn test_whitespace_and_comments() {
    let input = r#"
// Line comment
package com.example;

/* Block comment */
import haxe.Json;

// Another comment
"#;
    match parse_haxe_file("test.hx", input, false) {
        Ok(haxe_file) => {
            assert!(haxe_file.package.is_some());
            assert_eq!(haxe_file.package.unwrap().path, vec!["com", "example"]);
            assert_eq!(haxe_file.imports.len(), 1);
        }
        Err(e) => panic!("File with comments should parse, got: {}", e),
    }
}
