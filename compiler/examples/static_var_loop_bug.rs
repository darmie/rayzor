//! Test suite for investigating static var + loop bound SIGILL bug
//!
//! BUG: When a static variable (either `static inline var` or `static var`) is read
//! and the value is used as a loop bound (directly or through a local variable),
//! execution crashes with SIGILL (illegal instruction).
//!
//! PASSES:
//! - Literal values in loop bounds: `for (i in 0...5)`
//! - Local var with literal in loop: `var n = 5; for (i in 0...n)`
//! - Reading static var without loop: `var n = SIZE; trace(n);`
//! - Static var in arithmetic: `var r = SIZE * 2;`
//!
//! FAILS (SIGILL):
//! - Static var in loop bound: `for (i in 0...SIZE)`
//! - Static var to local, local in loop: `var n = SIZE; for (i in 0...n)`
//! - Static inline var in loop bound: `static inline var SIZE = 5; for (i in 0...SIZE)`
//!
//! Root cause investigation needed:
//! - Check how static var reads are lowered to MIR
//! - Check how IntIterator (0...n) generates code when n comes from static var
//! - Compare MIR/Cranelift IR between working and failing cases

use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};

fn test_case(name: &str, source: &str, symbols: &[(&str, *const u8)]) -> bool {
    println!("\n=== Test: {} ===", name);

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

    match result {
        Ok(()) => true,
        Err(e) => {
            println!("  FAIL: {}", e);
            false
        }
    }
}

fn main() {
    let plugin = rayzor_runtime::plugin_impl::get_plugin();
    let symbols: Vec<(&str, *const u8)> = plugin.runtime_symbols()
        .iter()
        .map(|(n, p)| (*n, *p))
        .collect();

    println!("Static Var + Loop Bound Bug Investigation");
    println!("==========================================\n");

    let mut passed = 0;
    let mut failed = 0;

    // === WORKING CASES (should all pass) ===
    println!("--- WORKING CASES (expected to pass) ---");

    if test_case("literal_loop_bound", r#"
package test;
class Main {
    public static function main() {
        var sum = 0;
        for (i in 0...5) { sum = sum + i; }
        trace(sum);
    }
}
"#, &symbols) { passed += 1; } else { failed += 1; }

    if test_case("local_var_literal_loop", r#"
package test;
class Main {
    public static function main() {
        var n = 5;
        var sum = 0;
        for (i in 0...n) { sum = sum + i; }
        trace(sum);
    }
}
"#, &symbols) { passed += 1; } else { failed += 1; }

    if test_case("static_var_read_only", r#"
package test;
class Main {
    static var SIZE = 5;
    public static function main() {
        var n = SIZE;
        trace(n);
    }
}
"#, &symbols) { passed += 1; } else { failed += 1; }

    if test_case("static_var_arithmetic", r#"
package test;
class Main {
    static var SIZE = 5;
    public static function main() {
        var r = SIZE * 2 + 3;
        trace(r);
    }
}
"#, &symbols) { passed += 1; } else { failed += 1; }

    if test_case("static_var_read_and_separate_loop", r#"
package test;
class Main {
    static var SIZE = 5;
    public static function main() {
        var n = SIZE;
        trace(n);
        for (i in 0...3) { }  // Loop with different bound
        trace(1);
    }
}
"#, &symbols) { passed += 1; } else { failed += 1; }

    // === FAILING CASES (expected to crash) ===
    println!("\n--- FAILING CASES (expected to crash - SIGILL) ---");

    if test_case("static_var_direct_loop_bound", r#"
package test;
class Main {
    static var SIZE = 5;
    public static function main() {
        var sum = 0;
        for (i in 0...SIZE) { sum = sum + i; }
        trace(sum);
    }
}
"#, &symbols) { passed += 1; } else { failed += 1; }

    if test_case("static_var_to_local_loop_bound", r#"
package test;
class Main {
    static var SIZE = 5;
    public static function main() {
        var n = SIZE;
        var sum = 0;
        for (i in 0...n) { sum = sum + i; }
        trace(sum);
    }
}
"#, &symbols) { passed += 1; } else { failed += 1; }

    if test_case("static_inline_var_loop_bound", r#"
package test;
class Main {
    static inline var SIZE = 5;
    public static function main() {
        var sum = 0;
        for (i in 0...SIZE) { sum = sum + i; }
        trace(sum);
    }
}
"#, &symbols) { passed += 1; } else { failed += 1; }

    if test_case("static_inline_to_local_loop", r#"
package test;
class Main {
    static inline var SIZE = 5;
    public static function main() {
        var n = SIZE;
        var sum = 0;
        for (i in 0...n) { sum = sum + i; }
        trace(sum);
    }
}
"#, &symbols) { passed += 1; } else { failed += 1; }

    if test_case("nested_loops_static_bound", r#"
package test;
class Main {
    static var WIDTH = 3;
    static var HEIGHT = 3;
    public static function main() {
        var count = 0;
        var w = WIDTH;
        var h = HEIGHT;
        for (y in 0...h) {
            for (x in 0...w) {
                count = count + 1;
            }
        }
        trace(count);
    }
}
"#, &symbols) { passed += 1; } else { failed += 1; }

    // === SUMMARY ===
    println!("\n==========================================");
    println!("Results: {} passed, {} failed", passed, failed);
    println!("==========================================");

    if failed > 0 {
        println!("\nNOTE: Failed tests are expected - they demonstrate the bug.");
        println!("The bug occurs when static var values are used as loop bounds.");
        println!("\nTo investigate, compare MIR output between working and failing cases:");
        println!("  cargo run --example dump_mir  (with appropriate source)");
    }
}
