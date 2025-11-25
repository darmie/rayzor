/// Test method calls through Dynamic
use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};

fn main() -> Result<(), String> {
    println!("=== Testing Dynamic Method Call ===\n");

    let haxe_source = r#"
package test;

class Point {
    public var x:Int;
    public var y:Int;

    public function new(x:Int, y:Int) {
        this.x = x;
        this.y = y;
    }

    public function add(other:Point):Point {
        return new Point(this.x + other.x, this.y + other.y);
    }

    public function getX():Int {
        return this.x;
    }

    public function getY():Int {
        return this.y;
    }
}

class Main {
    static function main() {
        var p1 = new Point(10, 20);
        var p2 = new Point(5, 3);

        // Test through Dynamic method calls
        var d:Dynamic = p1;
        trace(d.getX());  // Should print 10
        trace(d.getY());  // Should print 20

        var d2:Dynamic = p2;
        trace(d2.getX());  // Should print 5
        trace(d2.getY());  // Should print 3
    }
}
"#;

    let mut unit = CompilationUnit::new(CompilationConfig::default());

    println!("Loading stdlib...");
    unit.load_stdlib()
        .map_err(|e| format!("Failed to load stdlib: {}", e))?;

    println!("Adding test file...");
    unit.add_file(haxe_source, "test_dynamic_method_call.hx")
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
    println!("10");
    println!("20");
    println!("5");
    println!("3");
    println!("\n=== Actual Output ===\n");

    for module in mir_modules.iter().rev() {
        if backend.call_main(module).is_ok() {
            println!("\n=== Test Complete ===");
            return Ok(());
        }
    }

    Err("Failed to execute main".to_string())
}
