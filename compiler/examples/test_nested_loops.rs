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
//! Test nested loops with Complex class

use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};

fn test_case(name: &str, source: &str, symbols: &[(&str, *const u8)]) -> Result<(), String> {
    println!("\n=== Test: {} ===", name);

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
        match backend.call_main(module) {
            Ok(()) => {
                println!("  SUCCESS!");
                return Ok(());
            }
            Err(e) => {
                println!("  Module '{}': {}", module.name, e);
            }
        }
    }

    Err("No main function executed".to_string())
}

fn main() {
    let plugin = rayzor_runtime::plugin_impl::get_plugin();
    let symbols: Vec<(&str, *const u8)> = plugin
        .runtime_symbols()
        .iter()
        .map(|(n, p)| (*n, *p))
        .collect();

    // Test 1: Nested for loops without objects
    let _ = test_case(
        "nested_loops_simple",
        r#"
package test;
class Main {
    public static function main() {
        var sum = 0;
        for (y in 0...5) {
            for (x in 0...5) {
                sum = sum + x + y;
            }
        }
        trace(sum);
    }
}
"#,
        &symbols,
    );

    // Test 2: Nested for loops with object creation
    let _ = test_case(
        "nested_loops_objects",
        r#"
package test;
class Point {
    public var x:Float;
    public var y:Float;
    public function new(x:Float, y:Float) {
        this.x = x;
        this.y = y;
    }
}
class Main {
    public static function main() {
        for (y in 0...5) {
            for (x in 0...5) {
                var p = new Point(x * 1.0, y * 1.0);
            }
        }
        trace(25);
    }
}
"#,
        &symbols,
    );

    // Test 3: Nested loops with static function call
    let _ = test_case(
        "nested_loops_static_call",
        r#"
package test;
class Helper {
    static function compute(x:Int, y:Int):Int {
        return x * y;
    }
}
class Main {
    public static function main() {
        var sum = 0;
        for (y in 0...5) {
            for (x in 0...5) {
                sum = sum + Helper.compute(x, y);
            }
        }
        trace(sum);
    }
}
"#,
        &symbols,
    );

    // Test 4: Nested loops with instance method call returning object
    let _ = test_case(
        "nested_loops_instance_method",
        r#"
package test;
class Complex {
    public var re:Float;
    public var im:Float;
    public function new(re:Float, im:Float) {
        this.re = re;
        this.im = im;
    }
    public function add(c:Complex):Complex {
        return new Complex(re + c.re, im + c.im);
    }
}
class Main {
    public static function main() {
        var sum = 0;
        for (y in 0...5) {
            for (x in 0...5) {
                var c1 = new Complex(x * 1.0, y * 1.0);
                var c2 = new Complex(1.0, 1.0);
                var c3 = c1.add(c2);
                sum = sum + 1;
            }
        }
        trace(sum);
    }
}
"#,
        &symbols,
    );

    // Test 5: Mandelbrot-style iterate function
    let _ = test_case(
        "iterate_function",
        r#"
package test;
class Complex {
    public var re:Float;
    public var im:Float;
    public function new(re:Float, im:Float) {
        this.re = re;
        this.im = im;
    }
    public function add(c:Complex):Complex {
        return new Complex(re + c.re, im + c.im);
    }
    public function mul(c:Complex):Complex {
        return new Complex(re * c.re - im * c.im, re * c.im + im * c.re);
    }
    public function abs():Float {
        return Math.sqrt(re * re + im * im);
    }
}
class Main {
    public static function main() {
        var c = new Complex(0.5, 0.5);
        var result = iterate(c);
        trace(result);
    }

    static function iterate(c:Complex):Int {
        var z = new Complex(0.0, 0.0);
        for (i in 0...50) {
            z = z.mul(z).add(c);
            if (z.abs() > 2.0) return i;
        }
        return 50;
    }
}
"#,
        &symbols,
    );

    // Test 6: Nested loops calling iterate (like mandelbrot)
    let _ = test_case(
        "nested_with_iterate",
        r#"
package test;
class Complex {
    public var re:Float;
    public var im:Float;
    public function new(re:Float, im:Float) {
        this.re = re;
        this.im = im;
    }
    public function add(c:Complex):Complex {
        return new Complex(re + c.re, im + c.im);
    }
    public function mul(c:Complex):Complex {
        return new Complex(re * c.re - im * c.im, re * c.im + im * c.re);
    }
    public function abs():Float {
        return Math.sqrt(re * re + im * im);
    }
}
class Main {
    public static function main() {
        var checksum = 0;
        for (y in 0...3) {
            for (x in 0...3) {
                var c = new Complex(
                    (x - 1) * 2.0,
                    (y - 1) * 2.0
                );
                checksum = checksum + iterate(c);
            }
        }
        trace(checksum);
    }

    static function iterate(c:Complex):Int {
        var z = new Complex(0.0, 0.0);
        for (i in 0...50) {
            z = z.mul(z).add(c);
            if (z.abs() > 2.0) return i;
        }
        return 50;
    }
}
"#,
        &symbols,
    );

    println!("\n=== All tests completed ===");
}
