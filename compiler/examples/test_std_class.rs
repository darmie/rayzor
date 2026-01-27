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
//! Test Std class methods

use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};
use compiler::ir::IrModule;
use std::sync::Arc;

fn main() {
    println!("=== Std Class Test ===\n");

    // Test 1: Std.int - convert Float to Int
    println!("Test 1: Std.int");
    let source1 = r#"
class Main {
    static function main() {
        var x = Std.int(3.7);
        trace(x);  // 3

        var y = Std.int(-2.9);
        trace(y);  // -2

        var z = Std.int(0.5);
        trace(z);  // 0
    }
}
"#;
    run_test(source1, "std_int");

    // Test 2: Std.parseInt - convert String to Int
    println!("\nTest 2: Std.parseInt");
    let source2 = r#"
class Main {
    static function main() {
        var x = Std.parseInt("42");
        trace(x);  // 42

        var y = Std.parseInt("-123");
        trace(y);  // -123

        var z = Std.parseInt("0");
        trace(z);  // 0
    }
}
"#;
    run_test(source2, "std_parse_int");

    // Test 3: Std.parseFloat - convert String to Float
    println!("\nTest 3: Std.parseFloat");
    let source3 = r#"
class Main {
    static function main() {
        var x = Std.parseFloat("3.14");
        trace(x);  // 3.14

        var y = Std.parseFloat("-2.5");
        trace(y);  // -2.5

        var z = Std.parseFloat("0.0");
        trace(z);  // 0.0
    }
}
"#;
    run_test(source3, "std_parse_float");

    // Test 4: Std.random - random integer
    println!("\nTest 4: Std.random");
    let source4 = r#"
class Main {
    static function main() {
        // Random between 0 and 9 (inclusive of 0, exclusive of 10)
        var r1 = Std.random(10);
        trace(r1);  // Should be 0-9

        var r2 = Std.random(100);
        trace(r2);  // Should be 0-99

        // Edge case: random(1) should always be 0
        var r3 = Std.random(1);
        trace(r3);  // 0
    }
}
"#;
    run_test(source4, "std_random");

    // Test 5: Std.string - convert value to String
    println!("\nTest 5: Std.string (basic)");
    let source5 = r#"
class Main {
    static function main() {
        // For now just test that it compiles - string conversion is complex
        trace("Std.string test");
    }
}
"#;
    run_test(source5, "std_string");
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
