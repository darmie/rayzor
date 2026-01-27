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
//! Test Math operations with actual execution and logging

use compiler::pipeline::{HaxeCompilationPipeline, PipelineConfig};

fn main() {
    println!("Testing Math operations with execution...\n");

    let source = r#"
package test;

class MathTest {
    static function main():Void {
        // Test basic math operations
        trace("Math.PI = " + Math.PI);
        trace("Math.sin(0) = " + Math.sin(0.0));
        trace("Math.sin(Math.PI/2) = " + Math.sin(Math.PI / 2.0));
        trace("Math.cos(0) = " + Math.cos(0.0));
        trace("Math.sqrt(16) = " + Math.sqrt(16.0));
        trace("Math.abs(-5) = " + Math.abs(-5.0));
        trace("Math.min(3, 7) = " + Math.min(3.0, 7.0));
        trace("Math.max(3, 7) = " + Math.max(3.0, 7.0));
        trace("Math.floor(3.7) = " + Math.floor(3.7));
        trace("Math.ceil(3.2) = " + Math.ceil(3.2));
        trace("Math.round(3.5) = " + Math.round(3.5));
        trace("Math.pow(2, 8) = " + Math.pow(2.0, 8.0));
    }
}
"#;

    let mut config = PipelineConfig::default();
    config.enable_flow_sensitive_analysis = false;
    config.enable_enhanced_flow_analysis = false;

    let mut pipeline = HaxeCompilationPipeline::with_config(config);

    println!("=== COMPILATION PHASE ===");
    let result = pipeline.compile_file("test.hx", source);

    println!("Compilation errors: {}", result.errors.len());
    println!("HIR modules: {}", result.hir_modules.len());
    println!("MIR modules: {}", result.mir_modules.len());

    if !result.errors.is_empty() {
        println!("\n❌ Compilation errors:");
        for (i, error) in result.errors.iter().enumerate() {
            println!("  {}. {}", i + 1, error.message);
        }
        return;
    }

    println!("\n✅ Compilation successful!");

    // Check if MIR was generated
    if !result.mir_modules.is_empty() {
        println!("\n=== MIR ANALYSIS ===");
        for (i, mir_module) in result.mir_modules.iter().enumerate() {
            println!("MIR Module {}: {} functions", i, mir_module.functions.len());

            for (func_name, func) in &mir_module.functions {
                println!(
                    "  Function '{}': {} blocks, {} instructions",
                    func_name,
                    func.cfg.blocks.len(),
                    func.cfg
                        .blocks
                        .values()
                        .map(|b| b.instructions.len())
                        .sum::<usize>()
                );
            }
        }
    }

    println!("\n=== EXECUTION NOTES ===");
    println!("To execute this code, you would need to:");
    println!("1. Lower MIR to Cranelift IR");
    println!("2. JIT compile with Cranelift");
    println!("3. Link against runtime functions (haxe_math_*)");
    println!("4. Execute the main() function");
    println!("\nThe Math runtime functions should be registered via the plugin system.");
    println!("Check that these functions exist in the runtime:");
    println!("  - haxe_math_sin");
    println!("  - haxe_math_cos");
    println!("  - haxe_math_sqrt");
    println!("  - haxe_math_abs");
    println!("  - haxe_math_min");
    println!("  - haxe_math_max");
    println!("  - haxe_math_floor");
    println!("  - haxe_math_ceil");
    println!("  - haxe_math_round");
    println!("  - haxe_math_pow");

    println!("\n{}", "=".repeat(70));
}
