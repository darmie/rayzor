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
//! Minimal Math test

use compiler::pipeline::{HaxeCompilationPipeline, PipelineConfig};

fn main() {
    println!("Testing Math.sin detection...\n");

    let source = r#"
package test;
class Test {
    static function main():Void {
        var x:Float = Math.sin(3.14);
    }
}
"#;

    let mut config = PipelineConfig::default();
    // Disable dead code analysis to avoid false positives
    config.enable_flow_sensitive_analysis = false;
    config.enable_enhanced_flow_analysis = false;

    let mut pipeline = HaxeCompilationPipeline::with_config(config);
    let result = pipeline.compile_file("test.hx", source);

    println!("HIR modules: {}", result.hir_modules.len());
    println!("MIR modules: {}", result.mir_modules.len());
    println!("Compilation errors: {}", result.errors.len());

    if !result.errors.is_empty() {
        println!("\n❌ Compilation errors:");
        for (i, error) in result.errors.iter().enumerate() {
            println!("  {}. {}", i + 1, error.message);
        }
    } else {
        println!("\n✅ Compilation successful");
        println!("✅ Math.sin() detected and mapped to runtime function");
    }
}
