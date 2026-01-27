#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    unreachable_patterns,
    unused_mut,
    unused_assignments,
    unused_parens
)]
#![allow(
    clippy::single_component_path_imports,
    clippy::for_kv_map,
    clippy::explicit_auto_deref
)]
#![allow(
    clippy::println_empty_string,
    clippy::len_zero,
    clippy::useless_vec,
    clippy::field_reassign_with_default
)]
#![allow(
    clippy::needless_borrow,
    clippy::redundant_closure,
    clippy::bool_assert_comparison
)]
#![allow(
    clippy::empty_line_after_doc_comments,
    clippy::useless_format,
    clippy::clone_on_copy
)]
// Direct test of guard parsing with context error capture
use parser::{parse_haxe_file, parse_haxe_file_with_diagnostics};

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
            println!(
                "Parse result: {} declarations found",
                result.file.declarations.len()
            );
            println!("Diagnostics count: {}", result.diagnostics.len());

            if result.diagnostics.has_errors() {
                println!("\n=== Raw Diagnostics ===");
                println!("Total diagnostics: {}", result.diagnostics.len());

                println!("\n=== Formatted Diagnostics ===");
                let formatter = diagnostics::ErrorFormatter::with_colors();
                let formatted =
                    formatter.format_diagnostics(&result.diagnostics, &result.source_map);
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
            println!(
                "Basic parse succeeded with {} declarations",
                file.declarations.len()
            );
        }
        Err(e) => {
            println!("Basic parse failed: {:?}", e);
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
            println!(
                "Parse result: {} declarations found",
                result.file.declarations.len()
            );
            println!("Diagnostics count: {}", result.diagnostics.len());

            if !result.file.declarations.is_empty() {
                println!("Successfully parsed!");
            }
        }
        Err(e) => {
            println!("Parse failed: {:?}", e);
        }
    }
}
