//! Tests for AST helper methods

use parser::parse_haxe_file;
use parser::haxe_ast::TypeDeclaration;

#[test]
fn test_class_constructor_helpers() {
    let input = r#"
class MyClass {
    var field1: String;
    
    public function new() {}
    
    public function new(x: Int) {}  // overloaded constructor
    
    public function method(): Void {}
    
    var field2: Int = 42;
}
"#;
    
    match parse_haxe_file("test.hx", input, false) {
        Ok(haxe_file) => {
            if let TypeDeclaration::Class(class) = &haxe_file.declarations[0] {
                // Test has_constructor
                assert!(class.has_constructor(), "Class should have constructor");
                
                // Test get_constructors
                let constructors: Vec<_> = class.get_constructors().collect();
                assert_eq!(constructors.len(), 2, "Should find 2 constructors");
                
                // Test get_primary_constructor
                let primary = class.get_primary_constructor();
                assert!(primary.is_some(), "Should find primary constructor");
                assert_eq!(primary.unwrap().params.len(), 0, "Primary constructor should have no params");
                
                // Test get_non_constructor_fields
                let non_constructor_fields: Vec<_> = class.get_non_constructor_fields().collect();
                assert_eq!(non_constructor_fields.len(), 3, "Should have 3 non-constructor fields");
                
                // Test get_methods
                let methods: Vec<_> = class.get_methods().collect();
                assert_eq!(methods.len(), 1, "Should have 1 regular method");
                
                // Test get_vars_and_properties
                let vars: Vec<_> = class.get_vars_and_properties().collect();
                assert_eq!(vars.len(), 2, "Should have 2 variable fields");
            } else {
                panic!("Expected class declaration");
            }
        }
        Err(e) => panic!("Parse error: {}", e),
    }
}

#[test]
fn test_abstract_constructor_helpers() {
    let input = r#"
abstract Vec2(Point) {
    public function new(x: Float, y: Float) {
        this = {x: x, y: y};
    }
    
    public function add(other: Vec2): Vec2 {
        return new Vec2(this.x + other.x, this.y + other.y);
    }
}
"#;
    
    match parse_haxe_file("test.hx", input, false) {
        Ok(haxe_file) => {
            if let TypeDeclaration::Abstract(abstract_decl) = &haxe_file.declarations[0] {
                // Test has_constructor
                assert!(abstract_decl.has_constructor(), "Abstract should have constructor");
                
                // Test get_constructors
                let constructors: Vec<_> = abstract_decl.get_constructors().collect();
                assert_eq!(constructors.len(), 1, "Should find 1 constructor");
                
                // Test get_primary_constructor
                let primary = abstract_decl.get_primary_constructor();
                assert!(primary.is_some(), "Should find primary constructor");
                assert_eq!(primary.unwrap().params.len(), 2, "Constructor should have 2 params");
            } else {
                panic!("Expected abstract declaration");
            }
        }
        Err(e) => panic!("Parse error: {}", e),
    }
}

#[test]
fn test_class_without_constructor() {
    let input = r#"
class NoConstructor {
    var field: String;
    
    public function method(): Void {}
}
"#;
    
    match parse_haxe_file("test.hx", input, false) {
        Ok(haxe_file) => {
            if let TypeDeclaration::Class(class) = &haxe_file.declarations[0] {
                // Test has_constructor
                assert!(!class.has_constructor(), "Class should not have constructor");
                
                // Test get_constructors
                let constructors: Vec<_> = class.get_constructors().collect();
                assert_eq!(constructors.len(), 0, "Should find no constructors");
                
                // Test get_primary_constructor
                assert!(class.get_primary_constructor().is_none(), "Should not find primary constructor");
            } else {
                panic!("Expected class declaration");
            }
        }
        Err(e) => panic!("Parse error: {}", e),
    }
}