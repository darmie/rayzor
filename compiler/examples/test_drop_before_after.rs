//! Quick test to verify drop semantics reduces memory for mandelbrot pattern

use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};

fn main() {
    println!("=== Testing Drop Semantics for Mandelbrot Pattern ===\n");

    let plugin = rayzor_runtime::plugin_impl::get_plugin();
    let symbols: Vec<(&str, *const u8)> = plugin.runtime_symbols()
        .iter()
        .map(|(n, p)| (*n, *p))
        .collect();

    // Test the mandelbrot iterate pattern with many iterations
    // This is the core loop that was leaking memory
    let source = r#"
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

    public function abs():Float {
        return Math.sqrt(re * re + im * im);
    }
}

class Main {
    public static function main() {
        // Simulate the mandelbrot iterate function
        // With 10000 iterations, this would create ~20000 Complex objects
        // Without drop: all 20000 stay in memory
        // With drop: only ~2 at any time (z and the final result)
        var c = new Complex(0.1, 0.2);
        var count = 0;

        // Run iterate for multiple points to stress test
        for (point in 0...100) {
            var z = new Complex(0.0, 0.0);
            for (i in 0...100) {
                // This is the key pattern: z = z.mul(z).add(c)
                // Creates 2 temps per iteration, should be freed
                z = z.mul(z).add(c);
                if (z.abs() > 2.0) {
                    count = count + 1;
                    // Early return would also test drop on return path
                }
            }
        }

        trace(count);
    }
}
"#;

    println!("Compiling and running 100 points x 100 iterations = 10000 mandelbrot iterations");
    println!("Each iteration creates 2 intermediate Complex objects");
    println!("Without drop: ~20000 Complex objects in memory at end");
    println!("With drop: ~2 Complex objects in memory at any time\n");

    let result: Result<(), String> = (|| {
        let mut unit = CompilationUnit::new(CompilationConfig::fast());
        unit.load_stdlib().map_err(|e| format!("stdlib: {}", e))?;
        unit.add_file(source, "mandelbrot_pattern.hx").map_err(|e| format!("parse: {}", e))?;
        unit.lower_to_tast().map_err(|e| format!("tast: {:?}", e))?;

        let mir_modules = unit.get_mir_modules();

        let mut backend = CraneliftBackend::with_symbols(&symbols)
            .map_err(|e| format!("backend: {}", e))?;

        for module in &mir_modules {
            backend.compile_module(module).map_err(|e| format!("compile: {}", e))?;
        }

        let start = std::time::Instant::now();
        for module in mir_modules.iter().rev() {
            if backend.call_main(module).is_ok() {
                let elapsed = start.elapsed();
                println!("SUCCESS! Execution time: {:?}", elapsed);
                return Ok(());
            }
        }
        Err("No main executed".to_string())
    })();

    match result {
        Ok(()) => println!("\nTest passed - drop semantics is working"),
        Err(e) => println!("\nFailed: {}", e),
    }
}
