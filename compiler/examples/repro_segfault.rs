use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};
use compiler::ir::IrModule;
use rayzor_runtime;
use std::thread::sleep;
use std::time::Duration;

fn main() -> Result<(), String> {
    println!("=== Segfault Reproduction Test ===\n");

    let haxe_source = r#"
package test;

class Main {
    // Function that takes a callback and calls it
    static function run(callback: () -> Void) {
        callback();
    }

    static function main() {
        var x = 10;
        // Pass a closure that captures x
        run(() -> {
            var y = x; 
        });
    }
}
"#;

    // Create compilation unit with stdlib
    let mut unit = CompilationUnit::new(CompilationConfig::default());

    // Load stdlib
    if let Err(e) = unit.load_stdlib() {
        return Err(format!("Failed to load stdlib: {}", e));
    }

    // Add the test file
    if let Err(e) = unit.add_file(haxe_source, "repro.hx") {
        return Err(format!("Failed to add file: {}", e));
    }

    // Compile to TAST (which triggers lowering to HIR and MIR)
    println!("Compiling to TAST...");
    match unit.lower_to_tast() {
        Ok(_) => {}
        Err(e) => return Err(format!("Compilation failed: {:?}", e)),
    };

    let mir_modules = unit.get_mir_modules();
    if mir_modules.is_empty() {
        return Err("No MIR modules generated".to_string());
    }

    // Compile to native code
    println!("Compiling to native code...");
    let plugin = rayzor_runtime::plugin_impl::get_plugin();
    let symbols = plugin.runtime_symbols();
    let symbols_ref: Vec<(&str, *const u8)> = symbols.iter().map(|(n, p)| (*n, *p)).collect();

    let mut backend = CraneliftBackend::with_symbols(&symbols_ref)?;

    for module in &mir_modules {
        backend.compile_module(module)?;
    }

    // Execute
    println!("Executing...");
    for module in mir_modules.iter().rev() {
        if let Ok(()) = backend.call_main(module) {
            println!("Success!");
            return Ok(());
        }
    }

    Err("Failed to execute main".to_string())
}
