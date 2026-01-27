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
//! Test for enum constructor symbol resolution

use compiler::compilation::{CompilationConfig, CompilationUnit};

fn main() {
    println!("=== Enum Constructor Resolution Test ===\n");

    // Test 1: Simple enum without generics
    test_simple_enum();

    // Test 2: Generic enum (Option)
    test_generic_enum();

    println!("\n=== Tests completed ===");
}

fn test_simple_enum() {
    println!("TEST 1: Simple enum with pattern matching");
    println!("{}", "-".repeat(50));

    let source = r#"
enum Color {
    Red;
    Green;
    Blue;
}

class Main {
    static function main() {
        var c: Color = Color.Red;

        // Pattern matching on simple enum
        switch (c) {
            case Red: trace("It is Red");
            case Green: trace("It is Green");
            case Blue: trace("It is Blue");
        }
    }
}
"#;

    compile_and_report(source, "simple_enum");
}

fn test_generic_enum() {
    println!("\nTEST 2: Generic enum (Option<T>)");
    println!("{}", "-".repeat(50));

    let source = r#"
import haxe.ds.Option;

class Main {
    static function main() {
        var opt: Option<Int> = Option.Some(42);
        trace("Created Option.Some(42)");

        // Pattern matching (simplified - not using bound variable)
        switch (opt) {
            case Some(_): trace("Has a value");
            case None: trace("No value");
        }
    }
}
"#;

    compile_and_report_with_deps(source, "generic_enum", vec!["haxe.ds.Option"]);
}

fn compile_and_report(source: &str, name: &str) {
    compile_and_report_with_deps(source, name, vec![]);
}

fn compile_and_report_with_deps(source: &str, name: &str, deps: Vec<&str>) {
    let mut unit = CompilationUnit::new(CompilationConfig::default());

    if let Err(e) = unit.load_stdlib() {
        println!("  ❌ Failed to load stdlib: {}", e);
        return;
    }

    // Load any explicitly required dependencies BEFORE user file
    for dep in deps {
        if let Err(e) = unit.load_import_file(dep) {
            println!("  ⚠️ Warning: Failed to load {}: {}", dep, e);
        } else {
            println!("  ✓ Pre-loaded dependency: {}", dep);
        }
    }

    if let Err(e) = unit.add_file(source, &format!("{}.hx", name)) {
        println!("  ❌ Failed to add file: {}", e);
        return;
    }

    match unit.lower_to_tast() {
        Ok(typed_files) => {
            println!("  ✅ TAST lowering succeeded ({} files)", typed_files.len());

            // Check MIR
            let mir_modules = unit.get_mir_modules();
            if mir_modules.is_empty() {
                println!("  ❌ No MIR modules generated");
            } else {
                println!(
                    "  ✅ MIR lowering succeeded ({} modules)",
                    mir_modules.len()
                );
            }
        }
        Err(errors) => {
            println!("  ❌ TAST errors ({} errors):", errors.len());
            for (i, err) in errors.iter().enumerate().take(5) {
                println!("     {}: {:?}", i + 1, err);
            }
        }
    }
}
