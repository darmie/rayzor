/// Test Std.string() with runtime type dispatch via Dynamic boxing
use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};

fn main() -> Result<(), String> {
    println!("=== Testing Std.string() with Dynamic Boxing ===\n");

    let haxe_source = r#"
package test;

// Direct extern declarations for testing the runtime type system
@:native("haxe_box_int")
extern function boxInt(value:Int):Dynamic;

@:native("haxe_box_float")
extern function boxFloat(value:Float):Dynamic;

@:native("haxe_box_bool")
extern function boxBool(value:Bool):Dynamic;

@:native("haxe_std_string")
extern function stdString(dynamic:Dynamic):String;

class Main {
    static function main() {
        // Test boxing and Std.string() with runtime type dispatch

        // Box an Int and convert to string
        var dynInt = boxInt(42);
        var strInt = stdString(dynInt);
        trace(strInt);  // Should print "42"

        // Box a Float and convert to string
        var dynFloat = boxFloat(3.14159);
        var strFloat = stdString(dynFloat);
        trace(strFloat);  // Should print "3.14159"

        // Box a Bool and convert to string
        var dynBool = boxBool(true);
        var strBool = stdString(dynBool);
        trace(strBool);  // Should print "true"

        var dynBool2 = boxBool(false);
        var strBool2 = stdString(dynBool2);
        trace(strBool2);  // Should print "false"
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
    unit.add_file(haxe_source, "test_std_string.hx")
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
    println!("\n=== Actual Output ===\n");

    for module in mir_modules.iter().rev() {
        if backend.call_main(module).is_ok() {
            println!("\n=== Test Complete ===");
            return Ok(());
        }
    }

    Err("Failed to execute main".to_string())
}
