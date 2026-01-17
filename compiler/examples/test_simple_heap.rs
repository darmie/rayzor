//! Simple heap allocation test - isolate where crash occurs

use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};

fn test_case(name: &str, source: &str, symbols: &[(&str, *const u8)]) -> Result<(), String> {
    println!("\n=== Test: {} ===", name);

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
    let symbols: Vec<(&str, *const u8)> = plugin.runtime_symbols()
        .iter()
        .map(|(n, p)| (*n, *p))
        .collect();

    // Test 1: Simple constructor without returning
    let _ = test_case("simple_construct", r#"
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
        var p = new Point(1.0, 2.0);
        trace(p.x);
    }
}
"#, &symbols);

    // Test 2: Instance method call without return
    let _ = test_case("method_call", r#"
package test;
class Point {
    public var x:Float;
    public var y:Float;
    public function new(x:Float, y:Float) {
        this.x = x;
        this.y = y;
    }
    public function getSum():Float {
        return x + y;
    }
}
class Main {
    public static function main() {
        var p = new Point(1.0, 2.0);
        var sum = p.getSum();
        trace(sum);
    }
}
"#, &symbols);

    // Test 3: Instance method returning new object
    let _ = test_case("method_return_object", r#"
package test;
class Point {
    public var x:Float;
    public var y:Float;
    public function new(x:Float, y:Float) {
        this.x = x;
        this.y = y;
    }
    public function add(p:Point):Point {
        return new Point(x + p.x, y + p.y);
    }
}
class Main {
    public static function main() {
        var p1 = new Point(1.0, 2.0);
        var p2 = new Point(3.0, 4.0);
        var p3 = p1.add(p2);
        trace(p3.x);
    }
}
"#, &symbols);

    // Test 4: Loop with object allocation
    let _ = test_case("loop_allocation", r#"
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
        for (i in 0...10) {
            var p = new Point(i * 1.0, i * 2.0);
        }
        trace(999);
    }
}
"#, &symbols);

    // Test 5: Math.sqrt call (relevant to mandelbrot)
    let _ = test_case("math_sqrt", r#"
package test;
class Main {
    public static function main() {
        var x = 4.0;
        var y = Math.sqrt(x);
        trace(y);
    }
}
"#, &symbols);

    // Test 6: Complex-like class with mul/add/abs
    let _ = test_case("complex_like", r#"
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
        var c1 = new Complex(3.0, 4.0);
        var a = c1.abs();
        trace(a);  // Should be 5.0
    }
}
"#, &symbols);

    // Test 7: Loop with Complex operations (like mandelbrot)
    let _ = test_case("complex_loop", r#"
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
        var z = new Complex(0.0, 0.0);
        var c = new Complex(0.5, 0.5);
        for (i in 0...10) {
            z = z.mul(z).add(c);
            if (z.abs() > 2.0) {
                trace(i);
                return;
            }
        }
        trace(10);
    }
}
"#, &symbols);

    println!("\n=== All tests completed ===");
}
