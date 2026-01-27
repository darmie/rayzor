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
/// Test global trace() function with type-specific routing
use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};

fn main() -> Result<(), String> {
    println!("=== Testing Global trace() Function ===\n");

    let haxe_source = r#"
package test;

class Main {
    static function main() {
        // Test global trace() with different types
        trace(42);           // Int literal
        trace(3.14159);      // Float literal
        trace(true);         // Bool literal
        trace(false);        // Bool literal

        // Test with variables
        var x = 100;
        trace(x);            // Int variable

        var y = 2.718;
        trace(y);            // Float variable

        var z = true;
        trace(z);            // Bool variable

        // Test with int expression
        trace(5 + 10);       // Int arithmetic
    }
}
"#;

    // Create compilation unit
    let mut unit = CompilationUnit::new(CompilationConfig::default());

    // Load stdlib
    println!("Loading stdlib...");
    unit.load_stdlib()
        .map_err(|e| format!("Failed to load stdlib: {}", e))?;

    // Add test file
    println!("Adding test file...");
    unit.add_file(haxe_source, "test_trace_global.hx")
        .map_err(|e| format!("Failed to add file: {}", e))?;

    // Compile to TAST
    println!("Compiling to TAST...");
    unit.lower_to_tast()
        .map_err(|errors| format!("TAST errors: {:?}", errors))?;

    // Get MIR modules
    println!("Getting MIR modules...");
    let mir_modules = unit.get_mir_modules();
    if mir_modules.is_empty() {
        return Err("No MIR modules generated".to_string());
    }

    println!("MIR modules: {}", mir_modules.len());

    // Compile to native code
    println!("\nCompiling to native code...");
    let plugin = rayzor_runtime::plugin_impl::get_plugin();
    let symbols = plugin.runtime_symbols();
    let symbols_ref: Vec<(&str, *const u8)> = symbols.iter().map(|(n, p)| (*n, *p)).collect();

    let mut backend = CraneliftBackend::with_symbols(&symbols_ref)?;

    for module in &mir_modules {
        backend.compile_module(module)?;
    }

    println!("Codegen complete!\n");

    // Execute
    println!("=== Expected Output ===");
    println!("42");
    println!("3.14159");
    println!("true");
    println!("false");
    println!("100");
    println!("2.718");
    println!("true");
    println!("15");
    println!("\n=== Actual Output ===\n");

    for module in mir_modules.iter().rev() {
        if backend.call_main(module).is_ok() {
            println!("\n=== Test Complete ==");
            return Ok(());
        }
    }

    Err("Failed to execute main".to_string())
}
