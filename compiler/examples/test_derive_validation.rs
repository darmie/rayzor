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
    println!("=== Testing @:derive Trait Validation ===\n");

    // Test 1: @:rc without Clone (should error and auto-add)
    test_rc_without_clone();

    // Test 2: @:derive(Copy) with non-Copy field (should error)
    test_copy_with_string_field();

    // Test 3: @:derive(Clone) with non-Clone field (conservative - allows for now)
    test_clone_with_array();

    // Test 4: Valid @:rc with Clone
    test_rc_with_clone();

    println!("\n=== Tests Complete ===");
}

fn test_rc_with_clone() {
    println!("Test 1: @:rc with @:derive(Clone) - should succeed\n");

    let haxe_code = r#"
@:rc
@:derive(Clone)
class SharedResource {
    public var data: Int;
}

class Main {
    static function main() {
        var r1 = new SharedResource();
        r1.data = 42;
        var r2 = r1;  // RC increment
        trace(r1.data);
        trace(r2.data);
    }
}
"#;

    let result = compile_haxe_file("test_rc_clone.hx", haxe_code);

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
            if class_name == "SharedResource" {
                println!("  Class: {}", class_name);
                println!(
                    "  Has @:rc: {}",
                    class
                        .memory_annotations
                        .iter()
                        .any(|a| matches!(a, compiler::tast::MemoryAnnotation::Rc))
                );
                println!("  Derived traits: {:?}", class.get_derived_traits());
                println!("  is_clone(): {}", class.is_clone());
            }
        }
    } else {
        println!("✗ Compilation failed with {} errors:", result.errors.len());
        for (i, err) in result.errors.iter().take(3).enumerate() {
            println!("  Error {}: {}", i + 1, err.message);
        }
    }

    println!();
}

fn test_rc_without_clone() {
    println!("Test 2: @:rc without @:derive(Clone) - should error but auto-add\n");

    let haxe_code = r#"
@:rc
class SharedResource {
    public var data: Int;
}

class Main {
    static function main() {
        var r1 = new SharedResource();
        r1.data = 42;
    }
}
"#;

    let result = compile_haxe_file("test_rc_no_clone.hx", haxe_code);

    if result.errors.is_empty() && !result.typed_files.is_empty() {
        println!("✓ Compilation successful (Clone auto-added)");

        let typed_file = &result.typed_files[0];
        for class in &typed_file.classes {
            let class_name = typed_file
                .string_interner
                .borrow()
                .get(class.name)
                .unwrap_or("?")
                .to_string();
            if class_name == "SharedResource" {
                println!("  Class: {}", class_name);
                println!(
                    "  Has @:rc: {}",
                    class
                        .memory_annotations
                        .iter()
                        .any(|a| matches!(a, compiler::tast::MemoryAnnotation::Rc))
                );
                println!("  Derived traits: {:?}", class.get_derived_traits());
                println!("  is_clone(): {} (auto-added)", class.is_clone());
            }
        }
    } else {
        println!("✗ Compilation failed with {} errors:", result.errors.len());
        for (i, err) in result.errors.iter().take(3).enumerate() {
            println!("  Error {}: {}", i + 1, err.message);
        }
    }

    println!();
}

fn test_copy_with_string_field() {
    println!("Test 3: @:derive(Copy) with String field - should error\n");

    let haxe_code = r#"
@:derive([Clone, Copy])
class Person {
    public var name: String;  // String is NOT Copy
    public var age: Int;
}

class Main {
    static function main() {
        var p = new Person();
    }
}
"#;

    let result = compile_haxe_file("test_copy_string.hx", haxe_code);

    if result.errors.is_empty() && !result.typed_files.is_empty() {
        println!("✓ Compilation successful (but Copy should be removed)");

        let typed_file = &result.typed_files[0];
        for class in &typed_file.classes {
            let class_name = typed_file
                .string_interner
                .borrow()
                .get(class.name)
                .unwrap_or("?")
                .to_string();
            if class_name == "Person" {
                println!("  Class: {}", class_name);
                println!("  Derived traits: {:?}", class.get_derived_traits());
                println!("  is_copy(): {} (should be false)", class.is_copy());
                println!("  is_clone(): {}", class.is_clone());
            }
        }
    } else {
        println!("✗ Compilation failed with {} errors:", result.errors.len());
        for (i, err) in result.errors.iter().take(3).enumerate() {
            println!("  Error {}: {}", i + 1, err.message);
        }
    }

    println!();
}

fn test_clone_with_array() {
    println!("Test 4: @:derive(Clone) with Array field - should succeed\n");

    let haxe_code = r#"
@:derive(Clone)
class Buffer {
    public var data: Array<Int>;
}

class Main {
    static function main() {
        var b = new Buffer();
    }
}
"#;

    let result = compile_haxe_file("test_clone_array.hx", haxe_code);

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
            if class_name == "Buffer" {
                println!("  Class: {}", class_name);
                println!("  Derived traits: {:?}", class.get_derived_traits());
                println!("  is_clone(): {}", class.is_clone());
            }
        }
    } else {
        println!("✗ Compilation failed with {} errors:", result.errors.len());
        for (i, err) in result.errors.iter().take(3).enumerate() {
            println!("  Error {}: {}", i + 1, err.message);
        }
    }

    println!();
}
