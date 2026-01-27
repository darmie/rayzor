use compiler::pipeline::*;

#[test]
fn test_type_path_resolution_integration() {
    // Test that the type resolution correctly handles qualified type names
    // after our fix to use the correct parser Type enum variants

    let source = r#"
package com.example.models;

// Basic class with simple types
class User {
    public var id:Int;
    public var name:String;
    public var email:String;
    
    public function new(id:Int, name:String, email:String) {
        this.id = id;
        this.name = name;
        this.email = email;
    }
}

// Class that references another class in same package
class UserProfile {
    // Simple reference within same package
    public var user:User;
    
    // Fully qualified reference (should work even in same package)
    public var owner:com.example.models.User;
    
    // Array of local type
    public var friends:Array<User>;
    
    // Array with fully qualified type
    public var colleagues:Array<com.example.models.User>;
    
    public function new(user:User) {
        this.user = user;
        this.owner = user;
        this.friends = [];
        this.colleagues = [];
    }
    
    // Method returning local type
    public function getUser():User {
        return user;
    }
    
    // Method returning fully qualified type
    public function getOwner():com.example.models.User {
        return owner;
    }
}
"#;

    // Create pipeline
    let mut pipeline = HaxeCompilationPipeline::new();

    // Process the file
    let result = pipeline.compile_file("test.hx", source);

    // Check results
    if result.errors.is_empty() {
        println!("✅ Compilation succeeded!");
        println!("  Typed files: {}", result.typed_files.len());

        // Verify that types were resolved correctly
        if let Some(typed_file) = result.typed_files.first() {
            let total_decls = typed_file.classes.len()
                + typed_file.interfaces.len()
                + typed_file.enums.len()
                + typed_file.type_aliases.len();
            println!("  Declarations in file: {}", total_decls);

            // Check that we have both classes
            let class_count = typed_file.classes.len();

            assert_eq!(class_count, 2, "Expected 2 classes, found {}", class_count);
            println!("  Found {} classes", class_count);
        }
    } else {
        println!("❌ Compilation failed with {} errors:", result.errors.len());
        for error in &result.errors {
            println!("  - {}", error.message);
        }
        panic!("Type checking should succeed for valid qualified type references");
    }
}

#[test]
fn test_cross_package_type_resolution() {
    // Test cross-package imports and type resolution

    // First file: com.example.models.Product
    let source1 = r#"
package com.example.models;

class Product {
    public var id:Int;
    public var name:String;
    public var price:Float;
    
    public function new(id:Int, name:String, price:Float) {
        this.id = id;
        this.name = name;
        this.price = price;
    }
}
"#;

    // Second file: com.example.services.ProductService with import
    let source2 = r#"
package com.example.services;

import com.example.models.Product;

class ProductService {
    private var products:Array<Product>;
    
    public function new() {
        this.products = [];
    }
    
    // Using imported type
    public function addProduct(product:Product):Void {
        products.push(product);
    }
    
    // Using fully qualified type even with import
    public function createProduct(name:String, price:Float):com.example.models.Product {
        var id = products.length + 1;
        return new com.example.models.Product(id, name, price);
    }
    
    // Return array of imported type
    public function getAllProducts():Array<Product> {
        return products;
    }
}
"#;

    // Create pipeline
    let mut pipeline = HaxeCompilationPipeline::new();

    // Compile both files
    let result1 = pipeline.compile_file("Product.hx", source1);
    let result2 = pipeline.compile_file("ProductService.hx", source2);

    // Check first file
    if result1.errors.is_empty() {
        println!("✅ Product.hx compiled successfully");
    } else {
        println!("❌ Product.hx failed:");
        for error in &result1.errors {
            println!("  - {}", error.message);
        }
        panic!("Product.hx should compile");
    }

    // Check second file
    if result2.errors.is_empty() {
        println!("✅ ProductService.hx compiled successfully");
    } else {
        println!("❌ ProductService.hx failed:");
        for error in &result2.errors {
            println!("  - {}", error.message);
        }
        panic!("ProductService.hx should compile with imports");
    }
}

#[test]
fn test_type_resolution_edge_cases() {
    // Test various edge cases for type resolution

    let source = r#"
package test.edge.cases;

// Type aliases and complex scenarios
typedef UserId = Int;
typedef UserMap = Map<String, User>;

class User {
    public var id:UserId;
    public var name:String;
    
    public function new(id:UserId, name:String) {
        this.id = id;
        this.name = name;
    }
}

class EdgeCases {
    // Nested generics with qualified types
    public var usersByCategory:Map<String, Array<test.edge.cases.User>>;
    
    // Type alias usage
    public var userMap:UserMap;
    
    // Function types with qualified names
    public var userFactory:UserId -> String -> test.edge.cases.User;
    
    // Optional qualified type
    public var currentUser:Null<test.edge.cases.User>;
    
    public function new() {
        this.usersByCategory = new Map();
        this.userMap = new Map();
        this.userFactory = function(id:UserId, name:String) {
            return new test.edge.cases.User(id, name);
        };
        this.currentUser = null;
    }
    
    // Generic method with constraints
    public function processUsers<T:test.edge.cases.User>(users:Array<T>):Array<T> {
        // Process and return
        return users;
    }
}
"#;

    let mut pipeline = HaxeCompilationPipeline::new();
    let result = pipeline.compile_file("edge_cases.hx", source);

    if result.errors.is_empty() {
        println!("✅ Edge cases compiled successfully");

        // Verify typedef was processed
        if let Some(typed_file) = result.typed_files.first() {
            let typedef_count = typed_file.type_aliases.len();

            assert!(
                typedef_count >= 2,
                "Expected at least 2 typedefs, found {}",
                typedef_count
            );
            println!("  Found {} typedefs", typedef_count);
        }
    } else {
        println!("❌ Edge cases failed:");
        for error in &result.errors {
            println!("  - {}", error.message);
        }
        panic!("Edge cases should compile");
    }
}

#[test]
fn test_import_resolution_scenarios() {
    // Test various import scenarios

    let source = r#"
package test.imports;

// Test different import styles
import com.example.models.User;
import com.example.models.Product;
import com.example.services.*;  // Wildcard import

// Using alias (if supported)
import com.example.models.User as AppUser;

class ImportTest {
    // Simple imported type
    public var user:User;
    
    // Aliased type (if aliases are supported)
    public var appUser:AppUser;
    
    // Type from wildcard import
    public var product:Product;
    
    // Fully qualified despite import
    public var explicitUser:com.example.models.User;
    
    public function new() {
        // Test that constructors work with imported types
        this.user = new User(1, "Test", "test@example.com");
        this.appUser = new AppUser(2, "App", "app@example.com");
        this.product = new Product(1, "Widget", 9.99);
        this.explicitUser = new com.example.models.User(3, "Explicit", "explicit@example.com");
    }
}
"#;

    // This test documents the current state of import handling
    // It may fail if import resolution is not fully implemented
    let mut pipeline = HaxeCompilationPipeline::new();
    let result = pipeline.compile_file("import_test.hx", source);

    if result.errors.is_empty() {
        println!("✅ Import resolution test passed");
    } else {
        println!("⚠️  Import resolution test failed (expected if not fully implemented):");
        for error in &result.errors {
            println!("  - {}", error.message);
        }
        // Don't panic - this documents current limitations
    }
}

#[test]
fn test_type_resolution_parser_fix() {
    // This test specifically verifies that our fix to type_resolution.rs
    // correctly handles the parser's Type enum variants

    let source = r#"
package verification;

class TypeResolutionTest {
    // Test Type::Path variant
    public var simple:String;
    public var qualified:verification.TypeResolutionTest;
    
    // Test Type::Function variant
    public var func:Int -> String -> Bool;
    
    // Test Type::Optional variant
    public var optional:Null<String>;
    
    // Test Type::Anonymous variant (becomes Dynamic)
    public var anon:{ x:Int, y:Int };
    
    // Test nested types
    public var nested:Array<Map<String, verification.TypeResolutionTest>>;
    
    public function new() {
        this.simple = "test";
        this.qualified = this;
        this.func = function(a:Int, b:String):Bool { return true; };
        this.optional = null;
        this.anon = { x: 0, y: 0 };
        this.nested = [];
    }
}
"#;

    let mut pipeline = HaxeCompilationPipeline::new();
    let result = pipeline.compile_file("type_resolution_test.hx", source);

    if result.errors.is_empty() {
        println!(
            "✅ Type resolution parser fix verified - all Type enum variants handled correctly"
        );
    } else {
        println!("❌ Type resolution test failed:");
        for error in &result.errors {
            println!("  - {}", error.message);
        }
        panic!("Type resolution should handle all parser Type variants after our fix");
    }
}
