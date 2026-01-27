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
// Test to isolate if-guard parsing issue
use parser::haxe_parser::{parse_haxe_file, parse_haxe_file_with_diagnostics};

fn main() {
    // Test cases for switch guard parsing
    let test_cases = [
        //         ("Valid guard with parentheses", r#"
        // class Test {
        //     function test(value:Int):String {
        //         return switch(value) {
        //             case n if (n > 100):
        //                 "large";
        //             case _:
        //                 "small";
        //         };
        //     }
        // }
        // "#),
        (
            "Invalid guard without parentheses",
            r#"
class Test {
    function test(value:Int):String {
        return switch(value) {
            case n if n > 100:
                "large";
            case _:
                "small";
        };
    }
}
"#,
        ),
        //         ("Simple case without guard", r#"
        // class Test {
        //     function test(value:Int):String {
        //         return switch(value) {
        //             case 42:
        //                 "answer";
        //             case _:
        //                 "other";
        //         };
        //     }
        // }
        // "#),
    ];

    for (name, source) in test_cases.iter() {
        println!("\n=== Testing: {} ===", name);
        println!("Source: {}", source);

        // Try with enhanced diagnostics
        match parse_haxe_file_with_diagnostics("test.hx", source) {
            Ok(result) => {
                println!("✓ Parse successful");
                println!("Declarations: {}", result.file.declarations.len());
                println!("Diagnostics: {}", result.diagnostics.len());

                // Print diagnostics if any
                if !result.diagnostics.is_empty() {
                    println!("📋 Diagnostics found:");

                    // Try to format diagnostics
                    let formatter = diagnostics::ErrorFormatter::with_colors();
                    let formatted =
                        formatter.format_diagnostics(&result.diagnostics, &result.source_map);
                    println!("📋 Formatted diagnostics:\n{}", formatted);
                }

                for decl in &result.file.declarations {
                    use parser::haxe_ast::TypeDeclaration;
                    let decl_type = match decl {
                        TypeDeclaration::Class(c) => format!("Class({})", c.name),
                        TypeDeclaration::Interface(i) => format!("Interface({})", i.name),
                        TypeDeclaration::Enum(e) => format!("Enum({})", e.name),
                        TypeDeclaration::Abstract(a) => format!("Abstract({})", a.name),
                        TypeDeclaration::Typedef(t) => format!("Typedef({})", t.name),
                        TypeDeclaration::Conditional(_) => "Conditional".to_string(),
                    };
                    println!("  • {}", decl_type);
                }
            }
            Err(error_msg) => {
                println!("✗ Parse failed: {}", error_msg);
            }
        }
    }
}
