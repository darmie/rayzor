//! Test for recursive generic instantiation
//!
//! Tests cases like:
//! - Option<Option<Int>> - nested generic enum
//! - Box<Box<Int>> - nested generic class
//! - Container<Container<Int>> - user-defined nested generics
//! - List<List<Int>> - recursive data structures

use compiler::compilation::{CompilationConfig, CompilationUnit};

fn main() {
    println!("=== Recursive Generic Instantiation Test ===\n");

    // Test 1: Nested Vec (our native implementation)
    test_nested_vec();

    // Test 2: Nested Option
    test_nested_option();

    // Test 3: Nested generic class
    test_nested_generic_class();

    // Test 4: Triple nesting
    test_triple_nesting();

    println!("\n=== All recursive generic tests completed ===");
}

fn test_nested_vec() {
    println!("TEST 1: Nested Vec<Vec<Int>> (native runtime)");
    println!("{}", "-".repeat(50));

    let source = r#"
import rayzor.Vec;

class Main {
    static function main() {
        // Create inner vectors
        var row1 = new Vec<Int>();
        row1.push(1);
        row1.push(2);
        row1.push(3);

        var row2 = new Vec<Int>();
        row2.push(4);
        row2.push(5);
        row2.push(6);

        // Create outer vector (Vec of Vecs - a 2D matrix)
        var matrix = new Vec<Vec<Int>>();
        matrix.push(row1);
        matrix.push(row2);

        trace("Created 2x3 matrix");
        trace("Matrix length: " + matrix.length());

        // Access nested elements
        var firstRow = matrix.get(0);
        trace("First row length: " + firstRow.length());
        trace("Element [0][1]: " + firstRow.get(1));
    }
}
"#;

    compile_and_report(source, "nested_vec");
    println!();
}

fn test_nested_option() {
    println!("TEST 2: Nested Option<Option<Int>>");
    println!("{}", "-".repeat(50));

    let source = r#"
import haxe.ds.Option;

class Main {
    static function main() {
        // Option<Option<Int>> - nested generic
        // Use enum constructor syntax instead of Option.Some
        var inner: Option<Int> = Some(42);
        var outer: Option<Option<Int>> = Some(inner);

        trace("Created nested Option");

        switch (outer) {
            case Some(innerOpt):
                switch (innerOpt) {
                    case Some(value): trace("Inner value: " + value);
                    case None: trace("Inner is None");
                }
            case None: trace("Outer is None");
        }
    }
}
"#;

    compile_and_report(source, "nested_option");
    println!();
}

fn test_nested_generic_class() {
    println!("TEST 3: Nested generic class Box<Box<Int>>");
    println!("{}", "-".repeat(50));

    let source = r#"
@:generic
class Box<T> {
    var value: T;

    public function new(v: T) {
        this.value = v;
    }

    public function get(): T {
        return this.value;
    }
}

class Main {
    static function main() {
        // Box<Box<Int>> - nested generic class
        var inner = new Box<Int>(42);
        var outer = new Box<Box<Int>>(inner);

        trace("Created nested Box");

        var innerBox = outer.get();
        var value = innerBox.get();
        trace("Inner value: " + value);
    }
}
"#;

    compile_and_report(source, "nested_box");
    println!();
}

fn test_triple_nesting() {
    println!("TEST 4: Triple nesting Wrapper<Wrapper<Wrapper<Int>>>");
    println!("{}", "-".repeat(50));

    let source = r#"
@:generic
class Wrapper<T> {
    var data: T;

    public function new(d: T) {
        this.data = d;
    }

    public function unwrap(): T {
        return this.data;
    }
}

class Main {
    static function main() {
        // Triple nesting: Wrapper<Wrapper<Wrapper<Int>>>
        var level1 = new Wrapper<Int>(100);
        var level2 = new Wrapper<Wrapper<Int>>(level1);
        var level3 = new Wrapper<Wrapper<Wrapper<Int>>>(level2);

        trace("Created triple-nested Wrapper");

        // Unwrap all levels
        var l2 = level3.unwrap();
        var l1 = l2.unwrap();
        var value = l1.unwrap();
        trace("Deeply nested value: " + value);
    }
}
"#;

    compile_and_report(source, "triple_nesting");
    println!();
}

fn compile_and_report(source: &str, name: &str) {
    let mut unit = CompilationUnit::new(CompilationConfig::default());

    // Load stdlib
    if let Err(e) = unit.load_stdlib() {
        println!("  ❌ Failed to load stdlib: {}", e);
        return;
    }

    // Add source file
    if let Err(e) = unit.add_file(source, &format!("{}.hx", name)) {
        println!("  ❌ Failed to add file: {}", e);
        return;
    }

    // Compile to TAST
    match unit.lower_to_tast() {
        Ok(typed_files) => {
            println!("  ✅ TAST lowering succeeded ({} files)", typed_files.len());

            // Check MIR modules
            let mir_modules = unit.get_mir_modules();
            if mir_modules.is_empty() {
                println!("  ❌ No MIR modules generated");
                return;
            }

            println!("  ✅ MIR generated ({} modules)", mir_modules.len());

            // Look for monomorphized functions with nested type names
            let mut monomorphized: Vec<String> = Vec::new();
            let mut generic_count = 0;

            for module in &mir_modules {
                for (_, func) in &module.functions {
                    if !func.signature.type_params.is_empty() {
                        generic_count += 1;
                    }

                    // Find monomorphized versions (contain __ in name)
                    if func.name.contains("__") {
                        monomorphized.push(format!("{} ({:?})", func.name, func.id));
                    }
                }
            }

            if !monomorphized.is_empty() {
                println!("  🎯 Monomorphized functions:");
                for name in &monomorphized {
                    println!("     - {}", name);
                }
            }

            if generic_count > 0 {
                println!("  📋 Generic functions found: {}", generic_count);
            }
        }
        Err(errors) => {
            println!("  ❌ TAST errors ({}):", errors.len());
            for (i, err) in errors.iter().enumerate().take(10) {
                println!("     {}: {:?}", i + 1, err);
            }
        }
    }
}
