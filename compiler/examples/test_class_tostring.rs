//! Test tracing of classes with toString() method

use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};
use compiler::ir::IrModule;
use rayzor_runtime;
use std::sync::Arc;

fn main() {
    println!("=== Class toString() Test ===\n");

    // Test 1: Simple class with toString()
    println!("Test 1: Class with toString()");
    let source1 = r#"
class Point {
    public var x:Int;
    public var y:Int;

    public function new(x:Int, y:Int) {
        this.x = x;
        this.y = y;
    }

    public function toString():String {
        return "Point(" + x + ", " + y + ")";
    }
}

class Main {
    static function main() {
        var p = new Point(10, 20);
        trace(p);  // Should print "Point(10, 20)"
    }
}
"#;
    run_test(source1, "class_tostring");

    // Test 2: Class without toString() (should use default representation)
    println!("\nTest 2: Class without toString()");
    let source2 = r#"
class NoToString {
    public var value:Int;
    public function new(v:Int) { this.value = v; }
}

class Main {
    static function main() {
        var obj = new NoToString(42);
        trace(obj);  // Default representation
    }
}
"#;
    run_test(source2, "class_no_tostring");
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
