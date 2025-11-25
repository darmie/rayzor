/// Simple test for String.charAt
use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};

fn main() -> Result<(), String> {
    println!("=== Testing Simple String.charAt ===\n");

    let haxe_source = r#"
package test;

class Main {
    static function main() {
        var s:String = "hello";
        trace(s.charAt(0));  // Should print 'h'
    }
}
"#;

    let mut unit = CompilationUnit::new(CompilationConfig::default());

    println!("Loading stdlib...");
    unit.load_stdlib()
        .map_err(|e| format!("Failed to load stdlib: {}", e))?;

    println!("Adding test file...");
    unit.add_file(haxe_source, "test_string_simple.hx")
        .map_err(|e| format!("Failed to add file: {}", e))?;

    println!("Compiling to TAST...");
    unit.lower_to_tast()
        .map_err(|errors| format!("TAST errors: {:?}", errors))?;

    println!("Getting MIR modules...");
    let mir_modules = unit.get_mir_modules();
    if mir_modules.is_empty() {
        return Err("No MIR modules generated".to_string());
    }

    println!("MIR modules: {}", mir_modules.len());

    // Debug print the extern functions
    for module in &mir_modules {
        println!("\n=== MIR Module Functions ===");
        for (id, func) in &module.functions {
            if func.name.contains("char_at") || func.name.contains("charAt") {
                println!("  Function {:?}: {} (blocks: {})", id, func.name, func.cfg.blocks.len());
                for param in &func.signature.parameters {
                    println!("    param {}: {:?}", param.name, param.ty);
                }
                println!("    returns: {:?}", func.signature.return_type);
            }
        }
        println!("\n=== MIR Module Extern Functions ===");
        for (id, ef) in &module.extern_functions {
            if ef.name.contains("char_at") || ef.name.contains("charAt") {
                println!("  ExternFunc {:?}: {}", id, ef.name);
                for param in &ef.signature.parameters {
                    println!("    param {}: {:?}", param.name, param.ty);
                }
                println!("    returns: {:?}", ef.signature.return_type);
            }
        }
    }

    println!("\nCompiling to native code...");
    let plugin = rayzor_runtime::plugin_impl::get_plugin();
    let symbols = plugin.runtime_symbols();
    let symbols_ref: Vec<(&str, *const u8)> = symbols.iter().map(|(n, p)| (*n, *p)).collect();

    let mut backend = CraneliftBackend::with_symbols(&symbols_ref)?;

    for module in &mir_modules {
        backend.compile_module(module)?;
    }

    println!("Codegen complete!\n");

    println!("=== Expected Output ===");
    println!("h");
    println!("\n=== Actual Output ===\n");

    for module in mir_modules.iter().rev() {
        if backend.call_main(module).is_ok() {
            println!("\n=== Test Complete ===");
            return Ok(());
        }
    }

    Err("Failed to execute main".to_string())
}
