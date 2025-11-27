//! Test StringMap and IntMap from haxe.ds

use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};
use compiler::ir::IrModule;
use rayzor_runtime;
use std::sync::Arc;

fn main() {
    println!("=== StringMap/IntMap Test ===\n");

    // Test 1: Minimal StringMap - just new and exists
    println!("Test 1: StringMap new + exists");
    let source1 = r#"
import haxe.ds.StringMap;

class Main {
    static function main() {
        var map = new StringMap<Int>();
        trace("map created");

        // Check exists on empty map
        var exists = map.exists("key");
        trace(exists);  // false
    }
}
"#;
    run_test(source1, "stringmap_exists");

    // Test 2: StringMap set
    println!("\nTest 2: StringMap set");
    let source2 = r#"
import haxe.ds.StringMap;

class Main {
    static function main() {
        var map = new StringMap<Int>();

        // Set a value
        map.set("one", 1);
        trace("set done");

        // Check exists
        trace(map.exists("one"));  // true
    }
}
"#;
    run_test(source2, "stringmap_set");
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
