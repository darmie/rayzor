use compiler::pipeline::*;

#[test]
fn test_qualified_type_names() {
    // Test 1: Same package type reference
    let source1 = r#"
package com.example;

class User {
    public var name:String;
    public var id:Int;
    
    public function new(name:String, id:Int) {
        this.name = name;
        this.id = id;
    }
}

class Main {
    static function main() {
        // Test simple type reference
        var user:User = new User("Alice", 1);
        
        // Test fully qualified type reference
        var user2:com.example.User = new com.example.User("Bob", 2);
        
        trace(user.name);
        trace(user2.name);
    }
}
"#;

    let ast1 = parser::parse_haxe(source1, "test1.hx").expect("Failed to parse test1.hx");
    
    // Create a compilation pipeline
    let mut pipeline = HaxeCompilationPipeline::new();
    
    // Run type checking
    let result = pipeline.process_files(vec![("test1.hx".to_string(), ast1)]);
    
    match result {
        Ok(_) => {
            println!("Test 1 passed: Same package type references work!");
        }
        Err(errors) => {
            println!("Test 1 failed with errors:");
            for error in errors {
                println!("  - {}", error);
            }
            panic!("Type checking failed for same package references");
        }
    }
}

#[test]
fn test_cross_package_imports() {
    // Test 2: Cross-package imports
    let source1 = r#"
package com.example;

class User {
    public var name:String;
    public var id:Int;
    
    public function new(name:String, id:Int) {
        this.name = name;
        this.id = id;
    }
}
"#;

    let source2 = r#"
package test;

import com.example.User;

class TestImports {
    static function main() {
        // Using imported type
        var user:User = new User("Charlie", 3);
        trace(user.name);
        
        // Using fully qualified name even with import
        var user2:com.example.User = new com.example.User("David", 4);
        trace(user2.name);
    }
}
"#;

    let ast1 = parse_haxe(source1, "User.hx").expect("Failed to parse User.hx");
    let ast2 = parser::parse_haxe(source2, "TestImports.hx").expect("Failed to parse TestImports.hx");
    
    // Create a compilation pipeline
    let mut pipeline = HaxeCompilationPipeline::new();
    
    // Run type checking with both files
    let result = pipeline.process_files(vec![
        ("User.hx".to_string(), ast1),
        ("TestImports.hx".to_string(), ast2)
    ]);
    
    match result {
        Ok(_) => {
            println!("Test 2 passed: Cross-package imports work!");
        }
        Err(errors) => {
            println!("Test 2 failed with errors:");
            for error in errors {
                println!("  - {}", error);
            }
            panic!("Type checking failed for cross-package imports");
        }
    }
}

#[test]
fn test_type_path_resolution_edge_cases() {
    // Test 3: Edge cases
    let source = r#"
package com.example.models;

class Product {
    public var name:String;
    public var price:Float;
}

class Order {
    // Reference type in same package
    public var product:Product;
    
    // Fully qualified reference
    public var product2:com.example.models.Product;
    
    // Array of qualified types
    public var products:Array<com.example.models.Product>;
    
    public function new() {
        this.product = new Product();
        this.product2 = new com.example.models.Product();
        this.products = [];
    }
}
"#;

    let ast = parser::parse_haxe(source, "test3.hx").expect("Failed to parse test3.hx");
    
    let mut pipeline = HaxeCompilationPipeline::new();
    let result = pipeline.process_files(vec![("test3.hx".to_string(), ast)]);
    
    match result {
        Ok(_) => {
            println!("Test 3 passed: Edge cases work!");
        }
        Err(errors) => {
            println!("Test 3 failed with errors:");
            for error in errors {
                println!("  - {}", error);
            }
            panic!("Type checking failed for edge cases");
        }
    }
}