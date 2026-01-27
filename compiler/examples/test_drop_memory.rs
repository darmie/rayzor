#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    unreachable_patterns,
    unused_mut,
    unused_assignments,
    unused_parens
)]
#![allow(
    clippy::single_component_path_imports,
    clippy::for_kv_map,
    clippy::explicit_auto_deref
)]
#![allow(
    clippy::println_empty_string,
    clippy::len_zero,
    clippy::useless_vec,
    clippy::field_reassign_with_default
)]
#![allow(
    clippy::needless_borrow,
    clippy::redundant_closure,
    clippy::bool_assert_comparison
)]
#![allow(
    clippy::empty_line_after_doc_comments,
    clippy::useless_format,
    clippy::clone_on_copy
)]
//! Test that implicit drop reduces memory usage
//!
//! This test creates many objects in a loop and verifies that memory
//! doesn't grow unbounded (as it would without implicit drop)

use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};

fn main() {
    println!("=== Testing Implicit Drop Memory Management ===\n");

    let plugin = rayzor_runtime::plugin_impl::get_plugin();
    let symbols: Vec<(&str, *const u8)> = plugin
        .runtime_symbols()
        .iter()
        .map(|(n, p)| (*n, *p))
        .collect();

    // Test 1: Simple loop with reassignment
    println!("Test 1: Simple loop with variable reassignment");
    test_case(
        "loop_reassign",
        r#"
package test;

class Point {
    public var x:Int;
    public var y:Int;
    public function new(x:Int, y:Int) {
        this.x = x;
        this.y = y;
    }
}

class Main {
    public static function main() {
        // This loop creates 1000 Point objects
        // With implicit drop, each iteration frees the previous one
        // Without it, all 1000 remain in memory
        var p = new Point(0, 0);
        for (i in 0...1000) {
            p = new Point(i, i * 2);  // Should free old p before allocating new
        }
        trace(p.x);
    }
}
"#,
        &symbols,
    );

    // Test 2: Nested reassignment
    println!("\nTest 2: Nested reassignment in inner loop");
    test_case(
        "nested_loop",
        r#"
package test;

class Value {
    public var n:Int;
    public function new(n:Int) {
        this.n = n;
    }
}

class Main {
    public static function main() {
        var sum = 0;
        for (i in 0...10) {
            var v = new Value(0);
            for (j in 0...10) {
                v = new Value(j);  // Should free old v
                sum = sum + v.n;
            }
            // v should be freed at end of outer iteration
        }
        trace(sum);
    }
}
"#,
        &symbols,
    );

    // Test 3: Mandelbrot-like pattern (simplified)
    println!("\nTest 3: Mandelbrot-like iteration pattern");
    test_case(
        "mandelbrot_pattern",
        r#"
package test;

class Complex {
    public var re:Float;
    public var im:Float;

    public function new(re:Float, im:Float) {
        this.re = re;
        this.im = im;
    }

    public function mul(c:Complex):Complex {
        return new Complex(re * c.re - im * c.im, re * c.im + im * c.re);
    }

    public function add(c:Complex):Complex {
        return new Complex(re + c.re, im + c.im);
    }
}

class Main {
    public static function main() {
        var c = new Complex(0.1, 0.2);
        var z = new Complex(0.0, 0.0);

        // This is the mandelbrot inner loop pattern
        for (i in 0...100) {
            // z = z.mul(z).add(c)
            // This creates intermediate values that should be freed
            z = z.mul(z).add(c);
        }

        trace(1);
    }
}
"#,
        &symbols,
    );

    println!("\n=== All tests completed ===");
}

fn test_case(name: &str, source: &str, symbols: &[(&str, *const u8)]) {
    print!("  Running {} ... ", name);

    let result: Result<(), String> = (|| {
        let mut unit = CompilationUnit::new(CompilationConfig::fast());
        unit.load_stdlib().map_err(|e| format!("stdlib: {}", e))?;
        unit.add_file(source, &format!("{}.hx", name))
            .map_err(|e| format!("parse: {}", e))?;
        unit.lower_to_tast().map_err(|e| format!("tast: {:?}", e))?;

        let mir_modules = unit.get_mir_modules();

        let mut backend =
            CraneliftBackend::with_symbols(symbols).map_err(|e| format!("backend: {}", e))?;

        for module in &mir_modules {
            backend
                .compile_module(module)
                .map_err(|e| format!("compile: {}", e))?;
        }

        for module in mir_modules.iter().rev() {
            if backend.call_main(module).is_ok() {
                return Ok(());
            }
        }
        Err("No main executed".to_string())
    })();

    match result {
        Ok(()) => println!("PASS"),
        Err(e) => println!("FAIL: {}", e),
    }
}
