//! Simple Vec test without trace

use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};
use compiler::ir::IrModule;
use rayzor_runtime;
use std::sync::Arc;

fn main() {
    println!("=== Vec Simple Test ===\n");

    let source = r#"
import rayzor.Vec;

class Main {
    static function main() {
        var v = new Vec<Int>();
        v.push(10);
        v.push(20);
        v.push(30);

        // Just get values without trace
        var len = v.length();
        var first = v.get(0);
        var second = v.get(1);
        var third = v.get(2);

        // Test set
        v.set(1, 42);
        var updated = v.get(1);

        // Test first/last
        var firstVal = v.first();
        var lastVal = v.last();

        // Test pop
        var popped = v.pop();

        // Test clear
        v.clear();
        var lenAfterClear = v.length();

        // Return success if we got here
        return;
    }
}
"#;

    match compile_and_run(source, "vec_simple") {
        Ok(()) => {
            println!("✅ Vec simple test PASSED");
        }
        Err(e) => {
            println!("❌ Vec simple test FAILED: {}", e);
            std::process::exit(1);
        }
    }
}

fn compile_and_run(source: &str, name: &str) -> Result<(), String> {
    let mut unit = CompilationUnit::new(CompilationConfig::default());
    unit.load_stdlib()?;
    unit.add_file(source, &format!("{}.hx", name))?;

    let _typed_files = unit.lower_to_tast().map_err(|errors| {
        format!("TAST lowering failed: {:?}", errors)
    })?;

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
