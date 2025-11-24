//! Test using parse recovery mode for better error reporting

#[cfg(test)]
mod tests {
    use parser::parse_haxe_file;
    
    #[test]
    fn test_parse_with_recovery() {
        // Test content with switch that fails
        let test_content = r#"package com.example;

class TestClass {
    static function test() {
        var x = 1;
        var result = switch (x) {
            case 0: "zero";
            case 1: "one"; 
            case _: "other";
        }
    }
}"#;
        
        println!("=== Testing with recovery mode ===");
        match parse_haxe_file("test.hx", test_content, true) {
            Ok(_) => println!("✅ Parsed successfully with recovery"),
            Err(e) => {
                println!("❌ Parse failed with recovery mode:");
                println!("{}", e);
            }
        }
        
        println!("\n=== Testing without recovery mode ===");
        match parse_haxe_file("test.hx", test_content, false) {
            Ok(_) => println!("✅ Parsed successfully without recovery"),
            Err(e) => {
                println!("❌ Parse failed without recovery mode:");
                println!("{}", e);
            }
        }
    }
    
    #[test]
    fn test_comprehensive_parse_errors() {
        let test_content = r#"package com.example;

// This should parse fine
class Container<T> {
    public var items:Array<T>;
    
    public function new() {
        items = [];
    }
}

// This has problematic syntax
class TestProblems {
    static function problemFunction() {
        // Switch with missing semicolon
        var result = switch (x) {
            case 0: "zero";
            case 1: "one";
            case _: "other";
        }
        
        // Arrow function (not supported)
        var add = (a, b) -> a + b;
        
        // Nested function (not supported)
        function inner() {
            return 42;
        }
        
        // Pattern match with null (not supported)
        var y = switch (nullable) {
            case null: "null";
            case _: "not null";
        }
    }
}"#;
        
        println!("=== Testing comprehensive parse with recovery ===");
        match parse_haxe_file("test.hx", test_content, true) {
            Ok(_) => println!("✅ Parsed successfully with recovery"),
            Err(e) => {
                println!("❌ Parse failed:");
                // Print first 500 chars of error
                let error_str = format!("{}", e);
                println!("{}", &error_str[..500.min(error_str.len())]);
                if error_str.len() > 500 {
                    println!("... (truncated)");
                }
            }
        }
    }
}