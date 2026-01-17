//! Isolate static var + loop issue

use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};

fn run_test(name: &str, source: &str, symbols: &[(&str, *const u8)]) -> bool {
    println!("Testing: {}", name);

    let result: Result<(), String> = (|| {
        let mut unit = CompilationUnit::new(CompilationConfig::fast());
        unit.load_stdlib().map_err(|e| format!("stdlib: {}", e))?;
        unit.add_file(source, &format!("{}.hx", name)).map_err(|e| format!("parse: {}", e))?;
        unit.lower_to_tast().map_err(|e| format!("tast: {:?}", e))?;

        let mir_modules = unit.get_mir_modules();

        let mut backend = CraneliftBackend::with_symbols(symbols)
            .map_err(|e| format!("backend: {}", e))?;

        for module in &mir_modules {
            backend.compile_module(module).map_err(|e| format!("compile: {}", e))?;
        }

        for module in mir_modules.iter().rev() {
            if backend.call_main(module).is_ok() {
                println!("  PASS");
                return Ok(());
            }
        }
        Err("No main executed".to_string())
    })();

    if let Err(e) = result {
        println!("  FAIL: {}", e);
        false
    } else {
        true
    }
}

fn main() {
    let plugin = rayzor_runtime::plugin_impl::get_plugin();
    let symbols: Vec<(&str, *const u8)> = plugin.runtime_symbols()
        .iter()
        .map(|(n, p)| (*n, *p))
        .collect();

    // Run each test in isolation
    println!("\n=== Running isolated tests ===\n");

    // Test A: Works - local var with literal, used in loop
    run_test("literal_to_local_loop", r#"
package test;
class Main {
    public static function main() {
        var n = 5;
        for (i in 0...n) { }
        trace(1);
    }
}
"#, &symbols);

    println!("");

    // Test B: Check - static var read only (no loop)
    run_test("static_read_no_loop", r#"
package test;
class Main {
    static var SIZE = 5;
    public static function main() {
        var n = SIZE;
        trace(n);
    }
}
"#, &symbols);

    println!("");

    // Test C: Check - static var and separate loop
    run_test("static_read_separate_loop", r#"
package test;
class Main {
    static var SIZE = 5;
    public static function main() {
        var n = SIZE;
        trace(n);
        for (i in 0...3) { }
        trace(2);
    }
}
"#, &symbols);

    println!("");

    // Test D: The problematic case - static var to local, used in loop
    run_test("static_to_local_used_in_loop", r#"
package test;
class Main {
    static var SIZE = 5;
    public static function main() {
        var n = SIZE;
        for (i in 0...n) { }
        trace(3);
    }
}
"#, &symbols);

    println!("\n=== Done ===");
}
