/// Test arrow function vs function() capture semantics
///
/// This isolates the capture analysis issue to understand the difference
/// between arrow functions and traditional function expressions.

use compiler::compilation::{CompilationUnit, CompilationConfig};

fn main() -> Result<(), String> {
    println!("=== Arrow Function Capture Analysis Test ===\n");

    // Test 1: Arrow function with capture
    println!("TEST 1: Arrow function capturing outer variable");
    println!("─".repeat(70));
    test_source("arrow_capture", r#"
package test;

class Main {
    static function main() {
        var x = 42;
        var f = () -> {
            return x;
        };
        var result = f();
    }
}
"#)?;

    println!("\n");

    // Test 2: Traditional function with capture
    println!("TEST 2: Traditional function() capturing outer variable");
    println!("─".repeat(70));
    test_source("function_capture", r#"
package test;

class Main {
    static function main() {
        var x = 42;
        var f = function():Int {
            return x;
        };
        var result = f();
    }
}
"#)?;

    println!("\n");

    // Test 3: Arrow function with complex capture (like channel test)
    println!("TEST 3: Arrow function with multiple captures (channel-like)");
    println!("─".repeat(70));
    test_source("arrow_multi_capture", r#"
package test;

class Counter {
    public var value: Int;
    public function new() { this.value = 0; }
    public function increment():Void { this.value++; }
}

class Main {
    static function main() {
        var counter = new Counter();
        var i = 0;

        var f = () -> {
            counter.increment();
            return i * 2;
        };

        var result = f();
    }
}
"#)?;

    println!("\n");

    // Test 4: Traditional function with complex capture
    println!("TEST 4: Traditional function() with multiple captures");
    println!("─".repeat(70));
    test_source("function_multi_capture", r#"
package test;

class Counter {
    public var value: Int;
    public function new() { this.value = 0; }
    public function increment():Void { this.value++; }
}

class Main {
    static function main() {
        var counter = new Counter();
        var i = 0;

        var f = function():Int {
            counter.increment();
            return i * 2;
        };

        var result = f();
    }
}
"#)?;

    println!("\n=== Test Complete ===");
    Ok(())
}

fn test_source(name: &str, source: &str) -> Result<(), String> {
    let mut config = CompilationConfig::default();
    config.load_stdlib = false;

    let mut unit = CompilationUnit::new(config);

    // Add source
    let filename = format!("{}.hx", name);
    unit.add_file(source, &filename)?;

    // Try to lower to TAST (this includes MIR lowering via pipeline)
    match unit.lower_to_tast() {
        Ok(files) => {
            println!("  ✅ Compilation succeeded");
            println!("  Files compiled: {}", files.len());

            // Check MIR modules
            let mir_modules = unit.get_mir_modules();
            println!("  MIR modules: {}", mir_modules.len());

            if !mir_modules.is_empty() {
                let mir = mir_modules.last().unwrap();
                println!("  Functions: {}", mir.functions.len());
                println!("  Extern functions: {}", mir.extern_functions.len());
            }

            Ok(())
        }
        Err(errors) => {
            println!("  ❌ Compilation failed with {} errors:", errors.len());
            for (i, error) in errors.iter().enumerate() {
                println!("    Error {}: {}", i + 1, error.message);
            }
            Err(format!("Test '{}' failed", name))
        }
    }
}
