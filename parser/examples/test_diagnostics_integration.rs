//! Test the integrated diagnostics system
//!
//! This example tests that the parser correctly uses the diagnostics crate
//! for rich error reporting.

use parser::{parse_incrementally_enhanced, ErrorFormatter};

fn main() {
    println!("🧪 Testing Diagnostics Integration");
    println!("==================================\n");
    
    // Test 1: Missing semicolon
    test_missing_semicolon();
    
    // Test 2: Invalid function keyword
    test_invalid_function();
    
    // Test 3: Missing braces
    test_missing_braces();
    
    // Test 4: Multiple errors
    test_multiple_errors();
}

fn test_missing_semicolon() {
    println!("📝 Test 1: Missing Semicolon");
    println!("----------------------------\n");
    
    let test_code = r#"
package com.example;

class Test {
    var x = 1
    function test() {
        return x;
    }
}
"#;

    let result = parse_incrementally_enhanced("test.hx", test_code);
    
    if result.has_errors() {
        println!("✅ Successfully detected error:");
        println!("{}", result.format_diagnostics(true));
    } else {
        println!("❌ Failed to detect missing semicolon error");
    }
    
    println!("\n{}\n", "=".repeat(60));
}

fn test_invalid_function() {
    println!("📝 Test 2: Invalid Function Keyword");
    println!("-----------------------------------\n");
    
    let test_code = r#"
class Test {
    fucntion test() {
        return "hello";
    }
}
"#;

    let result = parse_incrementally_enhanced("test.hx", test_code);
    
    if result.has_errors() {
        println!("✅ Successfully detected error:");
        println!("{}", result.format_diagnostics(true));
    } else {
        println!("❌ Failed to detect invalid function keyword");
    }
    
    println!("\n{}\n", "=".repeat(60));
}

fn test_missing_braces() {
    println!("📝 Test 3: Missing Semicolon in Function");
    println!("------------------------------------------\n");
    
    let test_code = r#"
class Test {
    function test() {
        var x = 1;
        var y = 2
        return x + y;
    }
}
"#;

    let result = parse_incrementally_enhanced("test.hx", test_code);
    
    if result.has_errors() {
        println!("✅ Successfully detected error:");
        println!("{}", result.format_diagnostics(true));
    } else {
        println!("❌ Failed to detect missing braces");
    }
    
    println!("\n{}\n", "=".repeat(60));
}

fn test_multiple_errors() {
    println!("📝 Test 4: Multiple Errors");
    println!("--------------------------\n");
    
    let test_code = r#"
package com.example

import haxe.ds.StringMap

calss Calculator {
    var operations: StringMap<String->Float->Float>
    
    public fucntion new() {
        operations = new StringMap();
    }
    
    public function calculate(op: String, a: Float, b: Float): Float 
        var fn = operations.get(op);
        return fn(a, b);
    }
}
"#;

    let result = parse_incrementally_enhanced("calculator.hx", test_code);
    
    if result.has_errors() {
        println!("✅ Successfully detected multiple errors:");
        println!("{}", result.format_diagnostics(true));
        
        let error_count = result.diagnostics.errors().count();
        let warning_count = result.diagnostics.warnings().count();
        
        println!("\n📊 Summary:");
        println!("   Total errors: {}", error_count);
        println!("   Total warnings: {}", warning_count);
    } else {
        println!("❌ Failed to detect errors");
    }
    
    println!("\n{}\n", "=".repeat(60));
}