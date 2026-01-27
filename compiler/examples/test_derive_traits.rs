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
use compiler::pipeline::compile_haxe_file;

fn main() {
    println!("=== Testing @:derive Trait Metadata ===\n");

    // Test 1: @:derive(Clone)
    test_derive_clone();

    // Test 2: @:derive([Clone, Copy])
    test_derive_clone_and_copy();

    // Test 3: @:derive(Eq) without PartialEq (should warn)
    test_derive_missing_dependency();

    println!("\n=== Tests Complete ===");
}

fn test_derive_clone() {
    println!("Test 1: @:derive(Clone)\n");

    let haxe_code = r#"
@:derive(Clone)
class Point {
    public var x: Int;
    public var y: Int;

    public function new(x: Int, y: Int) {
        this.x = x;
        this.y = y;
    }
}

class Main {
    static function main() {
        var p1 = new Point(10, 20);
        var p2 = p1.clone();
        trace(p1.x);
        trace(p2.x);
    }
}
"#;

    let result = compile_haxe_file("test_derive_clone.hx", haxe_code);

    if result.errors.is_empty() && !result.typed_files.is_empty() {
        println!("✓ Compilation successful");

        // Check if Point class has Clone trait
        let typed_file = &result.typed_files[0];
        for class in &typed_file.classes {
            let class_name = typed_file
                .string_interner
                .borrow()
                .get(class.name)
                .unwrap_or("?")
                .to_string();
            if class_name == "Point" {
                println!("  Class: {}", class_name);
                println!("  Derived traits: {:?}", class.get_derived_traits());
                println!("  is_clone(): {}", class.is_clone());
                println!("  is_copy(): {}", class.is_copy());
            }
        }
    } else {
        println!("✗ Compilation failed with {} errors:", result.errors.len());
        for err in result.errors.iter().take(3) {
            println!("  - {}", err.message);
        }
    }

    println!();
}

fn test_derive_clone_and_copy() {
    println!("Test 2: @:derive([Clone, Copy])\n");

    let haxe_code = r#"
@:derive([Clone, Copy])
class Color {
    public var r: Int;
    public var g: Int;
    public var b: Int;

    public function new(r: Int, g: Int, b: Int) {
        this.r = r;
        this.g = g;
        this.b = b;
    }
}

class Main {
    static function main() {
        var c1 = new Color(255, 0, 0);
        var c2 = c1;  // Copy semantics
        trace(c1.r);
        trace(c2.r);
    }
}
"#;

    let result = compile_haxe_file("test_derive_copy.hx", haxe_code);

    if result.errors.is_empty() && !result.typed_files.is_empty() {
        println!("✓ Compilation successful");

        let typed_file = &result.typed_files[0];
        for class in &typed_file.classes {
            let class_name = typed_file
                .string_interner
                .borrow()
                .get(class.name)
                .unwrap_or("?")
                .to_string();
            if class_name == "Color" {
                println!("  Class: {}", class_name);
                println!("  Derived traits: {:?}", class.get_derived_traits());
                println!("  is_clone(): {}", class.is_clone());
                println!("  is_copy(): {}", class.is_copy());
            }
        }
    } else {
        println!("✗ Compilation failed with {} errors:", result.errors.len());
        for err in result.errors.iter().take(3) {
            println!("  - {}", err.message);
        }
    }

    println!();
}

fn test_derive_missing_dependency() {
    println!("Test 3: @:derive(Eq) without PartialEq (should warn)\n");

    let haxe_code = r#"
@:derive(Eq)
class Value {
    public var data: Int;

    public function new(data: Int) {
        this.data = data;
    }
}

class Main {
    static function main() {
        var v1 = new Value(42);
        var v2 = new Value(42);
    }
}
"#;

    let result = compile_haxe_file("test_derive_deps.hx", haxe_code);

    if result.errors.is_empty() && !result.typed_files.is_empty() {
        println!("✓ Compilation successful (warnings expected in stderr)");

        let typed_file = &result.typed_files[0];
        for class in &typed_file.classes {
            let class_name = typed_file
                .string_interner
                .borrow()
                .get(class.name)
                .unwrap_or("?")
                .to_string();
            if class_name == "Value" {
                println!("  Class: {}", class_name);
                println!("  Derived traits: {:?}", class.get_derived_traits());
            }
        }
    } else {
        println!("✗ Compilation failed with {} errors:", result.errors.len());
        for err in result.errors.iter().take(3) {
            println!("  - {}", err.message);
        }
    }

    println!();
}
