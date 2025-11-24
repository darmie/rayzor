// Direct test of guard parsing with context error capture
use parser::{parse_haxe_file_with_diagnostics, parse_haxe_file};

fn main() {
    // Test just the guard parsing issue in isolation
    let invalid_guard = r#"
class Test {
    function test(v:Int):String {
        return switch(v) {
            case n if n > 100:
                "large";
            default:
                "small";
        };
    }
}
"#;
    
    println!("=== Testing invalid guard without parentheses ===");
    
    // Try with the enhanced diagnostics parser
    match parse_haxe_file_with_diagnostics("test.hx", invalid_guard) {
        Ok(result) => {
            println!("Parse result: {} declarations found", result.file.declarations.len());
            println!("Diagnostics count: {}", result.diagnostics.len());
            
            if result.diagnostics.has_errors() {
                println!("\n=== Raw Diagnostics ===");
                println!("Total diagnostics: {}", result.diagnostics.len());
                
                println!("\n=== Formatted Diagnostics ===");
                let formatter = diagnostics::ErrorFormatter::with_colors();
                let formatted = formatter.format_diagnostics(&result.diagnostics, &result.source_map);
                println!("{}", formatted);
            }
        }
        Err(e) => {
            println!("Parse failed with error:\n{}", e);
        }
    }
    
    println!("\n=== Testing with basic parser (for comparison) ===");
    
    // Try with the basic parser
    match parse_haxe_file("test.hx", invalid_guard, false) {
        Ok(file) => {
            println!("Basic parse succeeded with {} declarations", file.declarations.len());
        }
        Err(e) => {
            println!("Basic parse failed: {}", e);
        }
    }
    
    // Now test with valid guard syntax
    let valid_guard = r#"
class Test {
    function test(v:Int):String {
        return switch(v) {
            case n if (n > 100):
                "large";
            default:
                "small";
        };
    }
}
"#;
    
    println!("\n=== Testing valid guard with parentheses ===");
    match parse_haxe_file_with_diagnostics("test.hx", valid_guard) {
        Ok(result) => {
            println!("Parse result: {} declarations found", result.file.declarations.len());
            println!("Diagnostics count: {}", result.diagnostics.len());
            
            if !result.file.declarations.is_empty() {
                println!("Successfully parsed!");
            }
        }
        Err(e) => {
            println!("Parse failed: {}", e);
        }
    }
}