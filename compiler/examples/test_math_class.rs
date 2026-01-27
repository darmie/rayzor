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
/// Test for Math class methods
use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};

fn main() -> Result<(), String> {
    println!("=== Testing Math Class Methods ===\n");

    let haxe_source = r#"
package test;

class Main {
    static function main() {
        // Test 1: Math.abs
        trace(Math.abs(-5.5));   // Should print 5.5 (or close)
        trace(Math.abs(3.0));    // Should print 3.0

        // Test 2: Math.floor
        trace(Math.floor(3.7));  // Should print 3.0
        trace(Math.floor(-2.3)); // Should print -3.0

        // Test 3: Math.ceil
        trace(Math.ceil(3.2));   // Should print 4.0
        trace(Math.ceil(-2.8));  // Should print -2.0

        // Test 4: Math.sqrt
        trace(Math.sqrt(4.0));   // Should print 2.0
        trace(Math.sqrt(9.0));   // Should print 3.0

        // Test 5: Math.sin/cos
        trace(Math.sin(0.0));    // Should print 0.0
        trace(Math.cos(0.0));    // Should print 1.0
    }
}
"#;

    let mut unit = CompilationUnit::new(CompilationConfig::default());

    println!("Loading stdlib...");
    unit.load_stdlib()
        .map_err(|e| format!("Failed to load stdlib: {}", e))?;

    println!("Adding test file...");
    unit.add_file(haxe_source, "test_math_class.hx")
        .map_err(|e| format!("Failed to add file: {}", e))?;

    println!("Compiling to TAST...");
    unit.lower_to_tast()
        .map_err(|errors| format!("TAST errors: {:?}", errors))?;

    println!("Getting MIR modules...");
    let mir_modules = unit.get_mir_modules();
    if mir_modules.is_empty() {
        return Err("No MIR modules generated".to_string());
    }

    println!("MIR modules: {}", mir_modules.len());

    println!("\nCompiling to native code...");
    let plugin = rayzor_runtime::plugin_impl::get_plugin();
    let symbols = plugin.runtime_symbols();
    let symbols_ref: Vec<(&str, *const u8)> = symbols.iter().map(|(n, p)| (*n, *p)).collect();

    let mut backend = CraneliftBackend::with_symbols(&symbols_ref)?;

    for module in &mir_modules {
        backend.compile_module(module)?;
    }

    println!("Codegen complete!\n");

    println!("=== Expected Output (approximate) ===");
    println!("5.5");
    println!("3.0");
    println!("3.0");
    println!("-3.0");
    println!("4.0");
    println!("-2.0");
    println!("2.0");
    println!("3.0");
    println!("0.0");
    println!("1.0");
    println!("\n=== Actual Output ===\n");

    for module in mir_modules.iter().rev() {
        if backend.call_main(module).is_ok() {
            println!("\n=== Test Complete ===");
            return Ok(());
        }
    }

    Err("Failed to execute main".to_string())
}
