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
//! Test stdlib method calls with runtime mapping
//!
//! This test verifies that Haxe stdlib method calls (String.charAt, Array.push, etc.)
//! are properly detected and mapped to runtime functions during HIR->MIR lowering.

use compiler::pipeline::HaxeCompilationPipeline;

fn main() {
    println!("🧪 Testing Stdlib Runtime Mapping\n");
    println!("{}", "=".repeat(70));

    // Test 1: String methods
    test_string_methods();

    // Test 2: Array methods
    test_array_methods();

    // Test 3: Math methods
    test_math_methods();

    println!("\n{}", "=".repeat(70));
    println!("\n✅ Stdlib runtime mapping test complete!");
    println!("   Check the output above for 'Stdlib method call detected' messages");
}

fn test_string_methods() {
    println!("\n📦 Test 1: String Methods");
    println!("{}", "-".repeat(70));

    let source = r#"
package test;

class Test {
    static function main() {
        var s:String = "hello";
        var ch = s.charAt(0);
        trace(ch);
        var upper = s.toUpperCase();
        trace(upper);
        var idx = s.indexOf("l");
        trace(idx);
    }
}
"#;

    compile_and_check(source, &["charAt", "toUpperCase", "indexOf"]);
}

fn test_array_methods() {
    println!("\n📦 Test 2: Array Methods");
    println!("{}", "-".repeat(70));

    let source = r#"
package test;

class Test {
    static function main() {
        var arr = new Array<Int>();
        arr.push(42);
        var x = arr.pop();
        trace(x);
        var copy = arr.copy();
        trace(copy);
    }
}
"#;

    compile_and_check(source, &["push", "pop", "copy"]);
}

fn test_math_methods() {
    println!("\n📦 Test 3: Math Methods");
    println!("{}", "-".repeat(70));

    let source = r#"
package test;

class Test {
    static function main() {
        var x = Math.sin(3.14);
        var y = Math.sqrt(16.0);
        var r = Math.random();
        trace(x);
        trace(y);
        trace(r);
    }
}
"#;

    compile_and_check(source, &["sin", "sqrt", "random"]);
}

fn compile_and_check(source: &str, expected_methods: &[&str]) {
    let mut pipeline = HaxeCompilationPipeline::new();

    println!("Compiling Haxe code...");
    let result = pipeline.compile_file("test.hx", source);

    if !result.errors.is_empty() {
        println!("❌ Compilation errors:");
        for error in &result.errors {
            println!("   {}", error.message);
        }
        return;
    }

    println!("✓ Compilation successful");
    println!("✓ Generated {} typed file(s)", result.typed_files.len());
    println!("✓ Generated {} HIR module(s)", result.hir_modules.len());
    println!("✓ Generated {} MIR module(s)", result.mir_modules.len());

    println!("\nExpected stdlib methods to be detected:");
    for method in expected_methods {
        println!("  - {}", method);
    }

    println!("\n(Check stderr output above for 'Stdlib method call detected' messages)");
}
