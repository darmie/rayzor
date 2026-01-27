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
//! Debug test for Mandelbrot benchmark with Cranelift
//!
//! Test to reproduce the "illegal hardware instruction" crash

use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};

fn main() {
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
    // Using static var with local copies for loop bounds
    static var WIDTH = 5;
    static var HEIGHT = 5;
    static var MAX_ITER = 10;

    public static function main() {
        var checksum = 0;
        var width = WIDTH;
        var height = HEIGHT;

        for (y in 0...height) {
            for (x in 0...width) {
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
        var maxIter = MAX_ITER;
        for (i in 0...maxIter) {
            z = z.mul(z).add(c);
            if (z.abs() > 2.0) return i;
        }
        return MAX_ITER;
    }
}
"#;

    println!("=== Mandelbrot Cranelift Debug Test ===\n");

    // Get runtime symbols
    let plugin = rayzor_runtime::plugin_impl::get_plugin();
    let symbols: Vec<(&str, *const u8)> = plugin
        .runtime_symbols()
        .iter()
        .map(|(n, p)| (*n, *p))
        .collect();

    println!("Runtime symbols loaded: {}", symbols.len());

    // Compile to MIR
    println!("\n1. Compiling to MIR...");
    let mut unit = CompilationUnit::new(CompilationConfig::fast());
    unit.load_stdlib().expect("stdlib");
    unit.add_file(source, "mandelbrot_test.hx").expect("parse");
    unit.lower_to_tast().expect("tast");

    let mir_modules = unit.get_mir_modules();
    println!("   MIR modules: {}", mir_modules.len());

    // List functions in the user module
    for module in &mir_modules {
        if module.name.contains("benchmark") || module.name.contains("test") {
            println!("\n   Module '{}' functions:", module.name);
            for (id, func) in &module.functions {
                if !func.cfg.blocks.is_empty() {
                    println!(
                        "     {:?}: {} (qualified: {:?})",
                        id, func.name, func.qualified_name
                    );
                }
            }
        }
    }

    // Create Cranelift backend
    println!("\n2. Creating Cranelift backend...");
    let mut backend = CraneliftBackend::with_symbols(&symbols).expect("backend");

    // Compile all modules
    println!("\n3. Compiling modules to Cranelift...");
    for module in &mir_modules {
        println!("   Compiling module: {}", module.name);
        if let Err(e) = backend.compile_module(module) {
            println!("   ERROR compiling: {}", e);
            return;
        }
    }
    println!("   All modules compiled successfully!");

    // Execute
    println!("\n4. Executing main()...");
    for module in mir_modules.iter().rev() {
        match backend.call_main(module) {
            Ok(()) => {
                println!("\n   Execution completed successfully!");
                return;
            }
            Err(e) => {
                println!("   Module '{}': {}", module.name, e);
            }
        }
    }

    println!("\n   ERROR: No main function executed successfully!");
}
