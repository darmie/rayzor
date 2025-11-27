//! Test tracing of enum values

use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};
use compiler::ir::IrModule;
use rayzor_runtime;
use std::sync::Arc;

fn main() {
    println!("=== Enum Tracing Test ===\n");

    // Test 1: Direct enum variant trace (should print variant name)
    println!("Test 1: Direct enum variant trace");
    let source1 = r#"
enum Color {
    Red;
    Green;
    Blue;
}

class Main {
    static function main() {
        trace(Color.Red);  // Should print "Red"
        trace(Color.Blue); // Should print "Blue"
    }
}
"#;
    run_test(source1, "direct_enum_trace");

    // Test 2: Enum variable trace (prints discriminant for now)
    println!("\nTest 2: Enum variable trace");
    let source2 = r#"
enum Color {
    Red;
    Green;
    Blue;
}

class Main {
    static function main() {
        var c = Color.Green;
        trace(c);  // Prints discriminant (1) until RTTI is implemented
    }
}
"#;
    run_test(source2, "enum_var_trace");

    // Test 3: Enum with parameters
    println!("\nTest 3: Enum with parameters");
    let source3 = r#"
enum Result {
    Ok(value:Int);
    Error(msg:String);
}

class Main {
    static function main() {
        var r = Result.Ok(42);
        trace(r);  // Should print "Ok(42)" or similar
    }
}
"#;
    run_test(source3, "enum_with_params");
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
