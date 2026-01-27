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
//! End-to-end tests for rayzor.Vec<T>
//!
//! Tests:
//! - Vec<Int> basic operations (push, pop, get, set, length)
//! - Vec<Float> operations
//! - Vec sorting (sort, sortBy)

use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};
use compiler::ir::IrModule;
use std::sync::Arc;

fn main() {
    println!("=== Vec<T> End-to-End Tests ===\n");

    let mut passed = 0;
    let mut failed = 0;

    // Test 1: Vec<Int> basic operations
    if run_test(
        "vec_int_basic",
        "Vec<Int> basic operations",
        test_vec_int_basic(),
    ) {
        passed += 1;
    } else {
        failed += 1;
    }

    // Test 2: Vec<Float> operations
    if run_test(
        "vec_float_basic",
        "Vec<Float> basic operations",
        test_vec_float_basic(),
    ) {
        passed += 1;
    } else {
        failed += 1;
    }

    // Test 3: Vec<Int> sorting
    if run_test(
        "vec_int_sort",
        "Vec<Int> sort (ascending)",
        test_vec_int_sort(),
    ) {
        passed += 1;
    } else {
        failed += 1;
    }

    // Test 4: Vec<Float> sorting
    if run_test(
        "vec_float_sort",
        "Vec<Float> sort (ascending)",
        test_vec_float_sort(),
    ) {
        passed += 1;
    } else {
        failed += 1;
    }

    // Test 5: Vec capacity and growth
    if run_test("vec_growth", "Vec capacity and growth", test_vec_growth()) {
        passed += 1;
    } else {
        failed += 1;
    }

    println!("\n{}", "=".repeat(50));
    println!("=== Vec Tests Summary ===");
    println!("Passed: {}", passed);
    println!("Failed: {}", failed);
    println!("{}", "=".repeat(50));

    if failed > 0 {
        std::process::exit(1);
    }
}

fn run_test(name: &str, description: &str, source: &str) -> bool {
    println!("\n{}", "-".repeat(50));
    println!("TEST: {} - {}", name, description);
    println!("{}", "-".repeat(50));

    match compile_and_run(source, name) {
        Ok(()) => {
            println!("✅ {} PASSED", name);
            true
        }
        Err(e) => {
            println!("❌ {} FAILED: {}", name, e);
            false
        }
    }
}

fn compile_and_run(source: &str, name: &str) -> Result<(), String> {
    // Create compilation unit
    let mut unit = CompilationUnit::new(CompilationConfig::default());

    // Load stdlib
    unit.load_stdlib()?;

    // Add source file
    unit.add_file(source, &format!("{}.hx", name))?;

    // Compile to TAST
    let _typed_files = unit
        .lower_to_tast()
        .map_err(|errors| format!("TAST lowering failed: {:?}", errors))?;

    // Get MIR modules
    let mir_modules = unit.get_mir_modules();
    if mir_modules.is_empty() {
        return Err("No MIR modules generated".to_string());
    }

    // Compile to native code
    let mut backend = compile_to_native(&mir_modules)?;

    // Execute
    execute_main(&mut backend, &mir_modules)?;

    Ok(())
}

fn compile_to_native(modules: &[Arc<IrModule>]) -> Result<CraneliftBackend, String> {
    let plugin = rayzor_runtime::plugin_impl::get_plugin();
    let symbols = plugin.runtime_symbols();
    let symbols_ref: Vec<(&str, *const u8)> = symbols.iter().map(|(n, p)| (*n, *p)).collect();

    let mut backend = CraneliftBackend::with_symbols(&symbols_ref)?;

    for module in modules {
        backend.compile_module(module)?;
    }

    Ok(backend)
}

fn execute_main(backend: &mut CraneliftBackend, modules: &[Arc<IrModule>]) -> Result<(), String> {
    for module in modules.iter().rev() {
        if backend.call_main(module).is_ok() {
            return Ok(());
        }
    }
    Err("Failed to execute main".to_string())
}

fn test_vec_int_basic() -> &'static str {
    r#"
import rayzor.Vec;

class Main {
    static function main() {
        var v = new Vec<Int>();

        // Push elements
        v.push(10);
        v.push(20);
        v.push(30);

        // Test length
        trace("Length: " + v.length());

        // Test get
        trace("v[0]: " + v.get(0));
        trace("v[1]: " + v.get(1));
        trace("v[2]: " + v.get(2));

        // Test first/last
        trace("First: " + v.first());
        trace("Last: " + v.last());

        // Test set
        v.set(1, 42);
        trace("After set v[1]=42: " + v.get(1));

        // Test pop
        var popped = v.pop();
        trace("Popped: " + popped);
        trace("Length after pop: " + v.length());

        // Test clear
        v.clear();
        trace("Length after clear: " + v.length());
    }
}
"#
}

fn test_vec_float_basic() -> &'static str {
    r#"
import rayzor.Vec;

class Main {
    static function main() {
        var v = new Vec<Float>();

        v.push(1.5);
        v.push(2.5);
        v.push(3.5);

        trace("Length: " + v.length());
        trace("v[0]: " + v.get(0));
        trace("v[1]: " + v.get(1));
        trace("v[2]: " + v.get(2));

        trace("First: " + v.first());
        trace("Last: " + v.last());
    }
}
"#
}

fn test_vec_int_sort() -> &'static str {
    r#"
import rayzor.Vec;

class Main {
    static function main() {
        var v = new Vec<Int>();

        // Push in random order
        v.push(5);
        v.push(2);
        v.push(8);
        v.push(1);
        v.push(9);
        v.push(3);

        trace("Before sort:");
        trace("  v[0]: " + v.get(0));
        trace("  v[1]: " + v.get(1));
        trace("  v[2]: " + v.get(2));
        trace("  v[3]: " + v.get(3));
        trace("  v[4]: " + v.get(4));
        trace("  v[5]: " + v.get(5));

        // Sort ascending
        v.sort();

        trace("After sort:");
        trace("  v[0]: " + v.get(0));
        trace("  v[1]: " + v.get(1));
        trace("  v[2]: " + v.get(2));
        trace("  v[3]: " + v.get(3));
        trace("  v[4]: " + v.get(4));
        trace("  v[5]: " + v.get(5));
    }
}
"#
}

fn test_vec_float_sort() -> &'static str {
    r#"
import rayzor.Vec;

class Main {
    static function main() {
        var v = new Vec<Float>();

        v.push(3.14);
        v.push(1.41);
        v.push(2.71);
        v.push(0.5);

        trace("Before sort:");
        trace("  v[0]: " + v.get(0));
        trace("  v[1]: " + v.get(1));
        trace("  v[2]: " + v.get(2));
        trace("  v[3]: " + v.get(3));

        v.sort();

        trace("After sort:");
        trace("  v[0]: " + v.get(0));
        trace("  v[1]: " + v.get(1));
        trace("  v[2]: " + v.get(2));
        trace("  v[3]: " + v.get(3));
    }
}
"#
}

fn test_vec_growth() -> &'static str {
    r#"
import rayzor.Vec;

class Main {
    static function main() {
        var v = new Vec<Int>();

        trace("Initial capacity: " + v.capacity());

        // Push many elements to trigger growth
        var i = 0;
        while (i < 20) {
            v.push(i * 10);
            i = i + 1;
        }

        trace("After 20 pushes:");
        trace("  Length: " + v.length());
        trace("  Capacity: " + v.capacity());

        // Verify values
        trace("  First: " + v.first());
        trace("  Last: " + v.last());
        trace("  v[10]: " + v.get(10));
    }
}
"#
}
