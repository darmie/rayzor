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
//! Test Std class functions

use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};
use compiler::ir::IrModule;
use std::sync::Arc;

fn main() {
    println!("=== Std Class Functions Test ===\n");

    // Test 1: Std.int()
    println!("Test 1: Std.int() - Float to Int conversion");
    let source1 = r#"
class Main {
    static function main() {
        trace(Std.int(3.7));    // Should print 3
        trace(Std.int(-3.7));   // Should print -3
        trace(Std.int(0.0));    // Should print 0
        trace(Std.int(10.99));  // Should print 10
    }
}
"#;
    run_test(source1, "std_int");

    // Test 2: Std.parseInt()
    println!("\nTest 2: Std.parseInt() - String to Int parsing");
    let source2 = r#"
class Main {
    static function main() {
        trace(Std.parseInt("42"));      // Should print 42
        trace(Std.parseInt("-123"));    // Should print -123
        trace(Std.parseInt("0xFF"));    // Should print 255 (hex)
        trace(Std.parseInt("  100"));   // Should print 100 (trim whitespace)
        trace(Std.parseInt("10abc"));   // Should print 10 (stops at invalid char)
    }
}
"#;
    run_test(source2, "std_parse_int");

    // Test 3: Std.parseFloat()
    println!("\nTest 3: Std.parseFloat() - String to Float parsing");
    let source3 = r#"
class Main {
    static function main() {
        trace(Std.parseFloat("3.14"));    // Should print 3.14
        trace(Std.parseFloat("-2.5"));    // Should print -2.5
        trace(Std.parseFloat("1e3"));     // Should print 1000
        trace(Std.parseFloat("  42.0"));  // Should print 42 (trim whitespace)
    }
}
"#;
    run_test(source3, "std_parse_float");

    // Test 4: Std.random()
    println!("\nTest 4: Std.random() - Random number generation");
    let source4 = r#"
class Main {
    static function main() {
        // Simple test - just one call
        var r = Std.random(10);
        trace(r);
    }
}
"#;
    run_test(source4, "std_random");

    // Test 5: Combined usage
    println!("\nTest 5: Combined Std functions");
    let source5 = r#"
class Main {
    static function main() {
        // Parse a float, then convert to int
        var f = Std.parseFloat("3.99");
        var i = Std.int(f);
        trace(i);  // Should print 3

        // Parse an int from string
        var parsed = Std.parseInt("100");
        trace(parsed);  // Should print 100
    }
}
"#;
    run_test(source5, "std_combined");
}

fn run_test(source: &str, name: &str) {
    match compile_and_run(source, name) {
        Ok(()) => {
            println!("✅ {} PASSED", name);
        }
        Err(e) => {
            println!("❌ {} FAILED: {}", name, e);
        }
    }
}

fn compile_and_run(source: &str, name: &str) -> Result<(), String> {
    let mut unit = CompilationUnit::new(CompilationConfig::default());
    unit.load_stdlib()?;
    unit.add_file(source, &format!("{}.hx", name))?;

    let _typed_files = unit
        .lower_to_tast()
        .map_err(|errors| format!("TAST lowering failed: {:?}", errors))?;

    let mir_modules = unit.get_mir_modules();
    if mir_modules.is_empty() {
        return Err("No MIR modules generated".to_string());
    }

    let mut backend = compile_to_native(&mir_modules)?;
    execute_main(&mut backend, &mir_modules)?;

    Ok(())
}

fn compile_to_native(modules: &[Arc<IrModule>]) -> Result<CraneliftBackend, String> {
    let plugin = rayzor_runtime::plugin_impl::get_plugin();
    let symbols = plugin.runtime_symbols();
    let symbols_ref: Vec<(&str, *const u8)> = symbols.iter().map(|(n, p)| (*n, *p)).collect();

    let mut backend = CraneliftBackend::with_symbols(&symbols_ref)?;

    for module in modules {
        backend.compile_module(module)?;
    }

    Ok(backend)
}

fn execute_main(backend: &mut CraneliftBackend, modules: &[Arc<IrModule>]) -> Result<(), String> {
    for module in modules.iter().rev() {
        if backend.call_main(module).is_ok() {
            return Ok(());
        }
    }
    Err("Failed to execute main".to_string())
}
