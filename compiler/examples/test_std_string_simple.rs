/// Test Std.string() with compile-time known types
use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};

fn main() -> Result<(), String> {
    println!("=== Testing Std.string() with Known Types ===\n");

    let haxe_source = r#"
package test;

class Main {
    static function main() {
        // Test Std.string() with compile-time known types

        var s1 = Std.string(42);
        trace(s1);  // Should print "42"

        var s2 = Std.string(3.14159);
        trace(s2);  // Should print "3.14159"

        var s3 = Std.string(true);
        trace(s3);  // Should print "true"

        var s4 = Std.string(false);
        trace(s4);  // Should print "false"

        // Test with variables
        var x = 100;
        var sx = Std.string(x);
        trace(sx);  // Should print "100"

        var y = 2.718;
        var sy = Std.string(y);
        trace(sy);  // Should print "2.718"

        var z = true;
        var sz = Std.string(z);
        trace(sz);  // Should print "true"
    }
}
"#;

    // Create compilation unit
    let mut unit = CompilationUnit::new(CompilationConfig::default());

    // Load stdlib
    println!("Loading stdlib...");
    unit.load_stdlib()
        .map_err(|e| format!("Failed to load stdlib: {}", e))?;

    // Add test file
    println!("Adding test file...");
    unit.add_file(haxe_source, "test_std_string_simple.hx")
        .map_err(|e| format!("Failed to add file: {}", e))?;

    // Compile to TAST
    println!("Compiling to TAST...");
    unit.lower_to_tast()
        .map_err(|errors| format!("TAST errors: {:?}", errors))?;

    // Get MIR modules
    println!("Getting MIR modules...");
    let mir_modules = unit.get_mir_modules();
    if mir_modules.is_empty() {
        return Err("No MIR modules generated".to_string());
    }

    println!("MIR modules: {}", mir_modules.len());

    // Compile to native code
    println!("\nCompiling to native code...");
    let plugin = rayzor_runtime::plugin_impl::get_plugin();
    let symbols = plugin.runtime_symbols();
    let symbols_ref: Vec<(&str, *const u8)> = symbols.iter().map(|(n, p)| (*n, *p)).collect();

    let mut backend = CraneliftBackend::with_symbols(&symbols_ref)?;

    for module in &mir_modules {
        backend.compile_module(module)?;
    }

    println!("Codegen complete!\n");

    // Execute
    println!("=== Expected Output ===");
    println!("42");
    println!("3.14159");
    println!("true");
    println!("false");
    println!("100");
    println!("2.718");
    println!("true");
    println!("\n=== Actual Output ===\n");

    for module in mir_modules.iter().rev() {
        if backend.call_main(module).is_ok() {
            println!("\n=== Test Complete ===");
            return Ok(());
        }
    }

    Err("Failed to execute main".to_string())
}
