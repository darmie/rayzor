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
//! Test Math method detection specifically

use compiler::pipeline::HaxeCompilationPipeline;

fn main() {
    println!("🧪 Testing Math Method Detection\n");
    println!("{}", "=".repeat(70));

    // Test 1: Simple Math.sin call
    test_simple_math();

    // Test 2: Math in expression
    test_math_in_expression();

    // Test 3: Math with variable
    test_math_with_variable();
}

fn test_simple_math() {
    println!("\n📦 Test 1: Simple Math.sin");
    println!("{}", "-".repeat(70));

    let source = r#"
class Test {
    static function main() {
        var x = Math.sin(3.14);
        trace(x);
    }
}
"#;

    compile_source(source);
}

fn test_math_in_expression() {
    println!("\n📦 Test 2: Math in Expression");
    println!("{}", "-".repeat(70));

    let source = r#"
class Test {
    static function main() {
        var result = Math.sqrt(16.0) + Math.sin(3.14);
        trace(result);
    }
}
"#;

    compile_source(source);
}

fn test_math_with_variable() {
    println!("\n📦 Test 3: Math with Variable");
    println!("{}", "-".repeat(70));

    let source = r#"
class Test {
    static function main() {
        var angle = 3.14;
        var sine = Math.sin(angle);
        trace(sine);
    }
}
"#;

    compile_source(source);
}

fn compile_source(source: &str) {
    let mut pipeline = HaxeCompilationPipeline::new();

    println!("Compiling...");
    let result = pipeline.compile_file("test.hx", source);

    println!("\nResults:");
    println!("  Errors: {}", result.errors.len());
    println!("  Warnings: {}", result.warnings.len());
    println!("  HIR modules: {}", result.hir_modules.len());
    println!("  MIR modules: {}", result.mir_modules.len());

    if !result.errors.is_empty() {
        println!("\n❌ Errors:");
        for (i, error) in result.errors.iter().enumerate().take(3) {
            println!("  {}. {}", i + 1, error.message);
        }
        if result.errors.len() > 3 {
            println!("  ... and {} more", result.errors.len() - 3);
        }
    } else {
        println!("\n✅ Compilation successful!");
    }

    println!("\n(Check stderr for 'Stdlib method call detected' messages)");
}
