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
//! Test static inline variables (like in mandelbrot benchmark)

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

    // Test 1: Simple static inline variable
    let _ = test_case(
        "simple_inline",
        r#"
package test;
class Main {
    static inline var SIZE = 10;

    public static function main() {
        trace(SIZE);
    }
}
"#,
        &symbols,
    );

    // Test 2: Static inline in loop
    let _ = test_case(
        "inline_in_loop",
        r#"
package test;
class Main {
    static inline var SIZE = 5;

    public static function main() {
        var sum = 0;
        for (i in 0...SIZE) {
            sum = sum + i;
        }
        trace(sum);
    }
}
"#,
        &symbols,
    );

    // Test 3: Multiple static inline vars (like WIDTH/HEIGHT)
    let _ = test_case(
        "multi_inline",
        r#"
package test;
class Main {
    static inline var WIDTH = 3;
    static inline var HEIGHT = 3;

    public static function main() {
        var count = 0;
        for (y in 0...HEIGHT) {
            for (x in 0...WIDTH) {
                count = count + 1;
            }
        }
        trace(count);
    }
}
"#,
        &symbols,
    );

    // Test 4: Static inline with calculation (like mandelbrot)
    let _ = test_case(
        "inline_with_calc",
        r#"
package test;
class Main {
    static inline var WIDTH = 5;
    static inline var HEIGHT = 5;

    public static function main() {
        for (y in 0...HEIGHT) {
            for (x in 0...WIDTH) {
                var xn = (x - WIDTH / 2) * 4.0 / WIDTH;
                var yn = (y - HEIGHT / 2) * 4.0 / HEIGHT;
            }
        }
        trace(25);
    }
}
"#,
        &symbols,
    );

    // Test 5: Mandelbrot-like with inline vars and Complex
    let _ = test_case(
        "mandelbrot_inline_simple",
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

class Mandelbrot {
    static inline var WIDTH = 3;
    static inline var HEIGHT = 3;
    static inline var MAX_ITER = 50;

    public static function main() {
        var checksum = 0;
        for (y in 0...HEIGHT) {
            for (x in 0...WIDTH) {
                var c = new Complex(
                    (x - WIDTH / 2) * 4.0 / WIDTH,
                    (y - HEIGHT / 2) * 4.0 / HEIGHT
                );
                checksum = checksum + iterate(c);
            }
        }
        trace(checksum);
    }

    static function iterate(c:Complex):Int {
        var z = new Complex(0.0, 0.0);
        for (i in 0...MAX_ITER) {
            z = z.mul(z).add(c);
            if (z.abs() > 2.0) return i;
        }
        return MAX_ITER;
    }
}
"#,
        &symbols,
    );

    // Test 6: Exact copy of mandelbrot benchmark structure with small values
    let _ = test_case(
        "mandelbrot_exact_structure",
        r#"
package benchmarks;

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

class Mandelbrot {
    static inline var WIDTH = 5;
    static inline var HEIGHT = 5;
    static inline var MAX_ITER = 50;

    public static function main() {
        var checksum = 0;

        for (y in 0...HEIGHT) {
            for (x in 0...WIDTH) {
                var c = new Complex(
                    (x - WIDTH / 2) * 4.0 / WIDTH,
                    (y - HEIGHT / 2) * 4.0 / HEIGHT
                );
                checksum = checksum + iterate(c);
            }
        }

        trace(checksum);
    }

    static function iterate(c:Complex):Int {
        var z = new Complex(0.0, 0.0);
        for (i in 0...MAX_ITER) {
            z = z.mul(z).add(c);
            if (z.abs() > 2.0) return i;
        }
        return MAX_ITER;
    }
}
"#,
        &symbols,
    );

    println!("\n=== All tests completed ===");
}
