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
//! Test Option<T> (from haxe.ds) and Result<T,E> (from StdTypes) enum usage

use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};
use compiler::ir::IrModule;
use std::sync::Arc;

fn main() {
    println!("=== Option & Result Enum Test ===\n");

    // Test 1: Result<T,E> from rayzor.core (requires import)
    println!("Test 1: Result enum from rayzor.core");
    let source1 = r#"
import rayzor.core.Result;

class Main {
    static function main() {
        var ok = Result.Ok(42);
        trace(ok);
        var err = Result.Error("something went wrong");
        trace(err);
    }
}
"#;
    run_test(source1, "result_import");

    // Test 2: Simple Result usage with Int error type
    println!("\nTest 2: Result with Int error");
    let source2 = r#"
import rayzor.core.Result;

class Main {
    static function main() {
        var r = Result.Ok(100);
        trace(r);
        var e = Result.Error(404);
        trace(e);
    }
}
"#;
    run_test(source2, "result_int_error");

    // Test 3: Inline enum (baseline - should still work)
    println!("\nTest 3: Inline enum baseline");
    let source3 = r#"
enum Color {
    Red;
    Green;
    Blue;
}

class Main {
    static function main() {
        trace(Color.Red);
        trace(Color.Blue);
    }
}
"#;
    run_test(source3, "inline_enum_baseline");
}

fn run_test(source: &str, name: &str) {
    match compile_and_run(source, name) {
        Ok(()) => {
            println!("  {} PASSED", name);
        }
        Err(e) => {
            println!("  {} FAILED: {}", name, e);
        }
    }
}

fn compile_and_run(source: &str, name: &str) -> Result<(), String> {
    let mut unit = CompilationUnit::new(CompilationConfig::fast());
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
    // Initialize ALL modules (calls __init__ which registers enum RTTI, etc.)
    backend.initialize_modules(modules)?;

    // Now find and call main
    for module in modules.iter().rev() {
        if backend.call_main(module).is_ok() {
            return Ok(());
        }
    }
    Err("Failed to execute main".to_string())
}
