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
//! Mandelbrot with workaround - pass values as function params

use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};

fn main() {
    // Use completely inlined literals to avoid static var issue
    let source = r#"
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
    public static function main() {
        var checksum = 0;
        // Use literal values directly instead of static vars
        // WIDTH = 10, HEIGHT = 10, MAX_ITER = 50
        for (y in 0...10) {
            for (x in 0...10) {
                var c = new Complex(
                    (x - 5) * 4.0 / 10,
                    (y - 5) * 4.0 / 10
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
"#;

    println!("=== Mandelbrot Fixed Test ===\n");

    let plugin = rayzor_runtime::plugin_impl::get_plugin();
    let symbols: Vec<(&str, *const u8)> = plugin
        .runtime_symbols()
        .iter()
        .map(|(n, p)| (*n, *p))
        .collect();

    let mut unit = CompilationUnit::new(CompilationConfig::fast());
    unit.load_stdlib().expect("stdlib");
    unit.add_file(source, "mandelbrot_fixed.hx").expect("parse");
    unit.lower_to_tast().expect("tast");

    let mir_modules = unit.get_mir_modules();

    let mut backend = CraneliftBackend::with_symbols(&symbols).expect("backend");

    for module in &mir_modules {
        backend.compile_module(module).expect("compile");
    }

    println!("Executing...");
    for module in mir_modules.iter().rev() {
        if backend.call_main(module).is_ok() {
            println!("SUCCESS!");
            return;
        }
    }
    println!("FAILED: No main executed");
}
