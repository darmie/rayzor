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
//! Test with explicit types

use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};
use compiler::ir::IrModule;
use std::sync::Arc;

fn main() {
    println!("=== Explicit Type Test ===\n");

    let source = r#"
import sys.io.File;
import sys.io.FileOutput;
import sys.io.FileInput;
import sys.FileSystem;

class Main {
    static function main() {
        trace("=== Test with EXPLICIT types ===");
        var output:FileOutput = File.write("/tmp/rayzor_explicit_test.txt", true);
        output.writeByte(72);  // 'H'
        output.close();
        trace("Wrote H!");
        
        var input:FileInput = File.read("/tmp/rayzor_explicit_test.txt", true);
        var b1 = input.readByte();
        input.close();
        trace(b1);
        
        FileSystem.deleteFile("/tmp/rayzor_explicit_test.txt");
        trace("Done!");
    }
}
"#;

    match compile_and_run(source, "explicit_test") {
        Ok(()) => println!("✅ Explicit type test PASSED"),
        Err(e) => println!("❌ FAILED: {:?}", e),
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
