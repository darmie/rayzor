//! Test module-level fields functionality

use parser::haxe_ast::*;
use parser::parse_haxe_file;

#[test]
fn test_module_level_variables() {
    let input = r#"
package com.example;

// Module-level variable
var moduleVar: String = "hello";

// Module-level final variable
final moduleConst: Int = 42;

// Module-level function
function moduleFunction(): Void {
    trace("Hello from module");
}

// Public module-level function
public function publicModuleFunction(): String {
    return "public";
}

// Static module-level function
static function staticModuleFunction(): Int {
    return 100;
}

// Then a class
class MyClass {
    public function new() {}
}
"#;

    match parse_haxe_file("test.hx", input, false) {
        Ok(ast) => {
            println!("Successfully parsed module-level fields!");

            // Check that we have module fields
            assert_eq!(ast.module_fields.len(), 5, "Expected 5 module fields");

            // Check the first field (var)
            let var_field = &ast.module_fields[0];
            match &var_field.kind {
                ModuleFieldKind::Var {
                    name,
                    type_hint,
                    expr,
                } => {
                    assert_eq!(name, "moduleVar");
                    assert!(type_hint.is_some());
                    assert!(expr.is_some());
                }
                _ => panic!("Expected var field"),
            }

            // Check the second field (final)
            let final_field = &ast.module_fields[1];
            match &final_field.kind {
                ModuleFieldKind::Final {
                    name,
                    type_hint,
                    expr,
                } => {
                    assert_eq!(name, "moduleConst");
                    assert!(type_hint.is_some());
                    assert!(expr.is_some());
                }
                _ => panic!("Expected final field"),
            }

            // Check the third field (function)
            let func_field = &ast.module_fields[2];
            match &func_field.kind {
                ModuleFieldKind::Function(func) => {
                    assert_eq!(func.name, "moduleFunction");
                    assert!(func.return_type.is_some());
                }
                _ => panic!("Expected function field"),
            }

            // Check that we still have the class
            assert_eq!(ast.declarations.len(), 1, "Expected 1 class declaration");
            match &ast.declarations[0] {
                TypeDeclaration::Class(class) => {
                    assert_eq!(class.name, "MyClass");
                }
                _ => panic!("Expected class declaration"),
            }

            println!("All module-level field tests passed!");
        }
        Err(e) => {
            panic!("Failed to parse module-level fields: {}", e);
        }
    }
}

#[test]
fn test_module_fields_with_imports() {
    let input = r#"
package com.example;

import haxe.ds.StringMap;

// Module-level variables after imports
var config: StringMap<String> = new StringMap();

function initConfig(): Void {
    config.set("version", "1.0");
}

class ConfigManager {
    public function new() {}
}
"#;

    match parse_haxe_file("test.hx", input, false) {
        Ok(ast) => {
            println!("Successfully parsed module fields with imports!");

            // Check imports
            assert_eq!(ast.imports.len(), 1, "Expected 1 import");

            // Check module fields
            assert_eq!(ast.module_fields.len(), 2, "Expected 2 module fields");

            // Check that we have the class
            assert_eq!(ast.declarations.len(), 1, "Expected 1 class");

            println!("Module fields with imports test passed!");
        }
        Err(e) => {
            panic!("Failed to parse module fields with imports: {}", e);
        }
    }
}

#[test]
fn test_empty_module_fields() {
    let input = r#"
package com.example;

// Only a class, no module fields
class EmptyModule {
    public function new() {}
}
"#;

    match parse_haxe_file("test.hx", input, false) {
        Ok(ast) => {
            println!("Successfully parsed file with no module fields!");

            // Check that we have no module fields
            assert_eq!(ast.module_fields.len(), 0, "Expected 0 module fields");

            // Check that we have the class
            assert_eq!(ast.declarations.len(), 1, "Expected 1 class");

            println!("Empty module fields test passed!");
        }
        Err(e) => {
            panic!("Failed to parse file with no module fields: {}", e);
        }
    }
}
