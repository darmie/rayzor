/// Simple test for String.charCodeAt
use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};

fn main() -> Result<(), String> {
    println!("=== Testing String.charCodeAt ===\n");

    let haxe_source = r#"
package test;

class Main {
    static function main() {
        var s:String = "hello";
        trace(s.charCodeAt(0));  // Should print 104 (ASCII for 'h')
        trace(s.charCodeAt(1));  // Should print 101 (ASCII for 'e')
    }
}
"#;

    let mut unit = CompilationUnit::new(CompilationConfig::default());

    println!("Loading stdlib...");
    unit.load_stdlib()
        .map_err(|e| format!("Failed to load stdlib: {}", e))?;

    println!("Adding test file...");
    unit.add_file(haxe_source, "test_charcodeat.hx")
        .map_err(|e| format!("Failed to add file: {}", e))?;

    println!("Compiling to TAST...");
    unit.lower_to_tast()
        .map_err(|errors| format!("TAST errors: {:?}", errors))?;

    println!("Getting MIR modules...");
    let mir_modules = unit.get_mir_modules();
    if mir_modules.is_empty() {
        return Err("No MIR modules generated".to_string());
    }

    println!("Compiling to native code...");
    let plugin = rayzor_runtime::plugin_impl::get_plugin();
    let symbols = plugin.runtime_symbols();
    let symbols_ref: Vec<(&str, *const u8)> = symbols.iter().map(|(n, p)| (*n, *p)).collect();

    let mut backend = CraneliftBackend::with_symbols(&symbols_ref)?;

    for module in &mir_modules {
        backend.compile_module(module)?;
    }

    println!("Codegen complete!\n");

    println!("=== Expected Output ===");
    println!("104");
    println!("101");
    println!("\n=== Actual Output ===\n");

    for module in mir_modules.iter().rev() {
        if backend.call_main(module).is_ok() {
            println!("\n=== Test Complete ===");
            return Ok(());
        }
    }

    Err("Failed to execute main".to_string())
}
