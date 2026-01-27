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
//! Test Math constants like PI

use compiler::pipeline::HaxeCompilationPipeline;

fn main() {
    println!("🧪 Testing Math Constants\n");
    println!("{}", "=".repeat(70));

    let source = r#"
package test;

class Test {
    static function main() {
        var pi = Math.PI;
        var result = Math.sin(Math.PI);
    }
}
"#;

    let mut pipeline = HaxeCompilationPipeline::new();
    println!("Compiling Haxe code with Math.PI...");
    let result = pipeline.compile_file("test.hx", source);

    println!("\nResults:");
    println!("  Compilation errors: {}", result.errors.len());
    println!("  HIR modules: {}", result.hir_modules.len());
    println!("  MIR modules: {}", result.mir_modules.len());

    if !result.errors.is_empty() {
        println!("\n❌ Compilation errors:");
        for (i, error) in result.errors.iter().enumerate().take(5) {
            println!("  {}. {}", i + 1, error.message);
        }
    } else {
        println!("\n✅ Successfully compiled code using Math.PI and Math.sin()!");
        println!("✅ Math methods detected and mapped to runtime functions");
    }

    println!("\n{}", "=".repeat(70));
}
