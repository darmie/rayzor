use compiler::pipeline::*;

#[test]
fn test_qualified_type_names() {
    // Test 1: Same package type reference (using simple names within same file)
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
        // Test simple type reference within same package
        var user:User = new User("Alice", 1);
        trace(user.name);
    }
}
"#;

    let mut pipeline = HaxeCompilationPipeline::new();
    let result = pipeline.compile_file("test1.hx", source1);

    if result.errors.is_empty() {
        println!("Test 1 passed: Same package type references work!");
    } else {
        println!("Test 1 failed with errors:");
        for error in &result.errors {
            println!("  - {}", error.message);
        }
        panic!("Type checking failed for same package references");
    }
}

#[test]
fn test_cross_package_imports() {
    // Test 2: Multiple classes in same file with cross-references
    let source = r#"
package com.example;

class User {
    public var name:String;
    public var id:Int;

    public function new(name:String, id:Int) {
        this.name = name;
        this.id = id;
    }
}

class UserManager {
    public var activeUser:User;

    public function new() {
        this.activeUser = new User("Default", 0);
    }

    public function createUser(name:String, id:Int):User {
        return new User(name, id);
    }
}
"#;

    let mut pipeline = HaxeCompilationPipeline::new();
    let result = pipeline.compile_file("UserManager.hx", source);

    if result.errors.is_empty() {
        println!("Test 2 passed: Cross-class type references work!");
    } else {
        println!("Test 2 failed with errors:");
        for error in &result.errors {
            println!("  - {}", error.message);
        }
        panic!("Type checking failed for cross-class references");
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

    let mut pipeline = HaxeCompilationPipeline::new();
    let result = pipeline.compile_file("test3.hx", source);

    if result.errors.is_empty() {
        println!("Test 3 passed: Edge cases work!");
    } else {
        println!("Test 3 failed with errors:");
        for error in &result.errors {
            println!("  - {}", error.message);
        }
        panic!("Type checking failed for edge cases");
    }
}
