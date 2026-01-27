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
/// Test for Array class methods
use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};

fn main() -> Result<(), String> {
    println!("=== Testing Array Class Methods ===\n");

    let haxe_source = r#"
package test;

class Main {
    static function main() {
        // Test 1: Array creation and length
        var arr = new Array<Int>();
        trace(arr.length);  // Should print 0

        // Test 2: push and length
        arr.push(10);
        arr.push(20);
        arr.push(30);
        trace(arr.length);  // Should print 3

        // Test 3: pop
        var last = arr.pop();
        trace(last);        // Should print 30
        trace(arr.length);  // Should print 2

        // Test 4: index access (get)
        arr.push(40);
        trace(arr[0]);  // Should print 10
        trace(arr[1]);  // Should print 20
        trace(arr[2]);  // Should print 40

        // Test 5: reverse
        var arr2 = new Array<Int>();
        arr2.push(1);
        arr2.push(2);
        arr2.push(3);
        arr2.reverse();
        trace(arr2[0]);  // Should print 3
        trace(arr2[1]);  // Should print 2
        trace(arr2[2]);  // Should print 1

        // Test 6: insert
        var arr3 = new Array<Int>();
        arr3.push(1);
        arr3.push(3);
        arr3.insert(1, 2);  // Insert 2 at position 1: [1, 2, 3]
        trace(arr3.length);  // Should print 3
        trace(arr3[0]);  // Should print 1
        trace(arr3[1]);  // Should print 2
        trace(arr3[2]);  // Should print 3
    }
}
"#;

    let mut unit = CompilationUnit::new(CompilationConfig::default());

    println!("Loading stdlib...");
    unit.load_stdlib()
        .map_err(|e| format!("Failed to load stdlib: {}", e))?;

    println!("Adding test file...");
    unit.add_file(haxe_source, "test_array_class.hx")
        .map_err(|e| format!("Failed to add file: {}", e))?;

    println!("Compiling to TAST...");
    unit.lower_to_tast()
        .map_err(|errors| format!("TAST errors: {:?}", errors))?;

    println!("Getting MIR modules...");
    let mir_modules = unit.get_mir_modules();
    if mir_modules.is_empty() {
        return Err("No MIR modules generated".to_string());
    }

    println!("MIR modules: {}", mir_modules.len());

    println!("\nCompiling to native code...");
    let plugin = rayzor_runtime::plugin_impl::get_plugin();
    let symbols = plugin.runtime_symbols();
    let symbols_ref: Vec<(&str, *const u8)> = symbols.iter().map(|(n, p)| (*n, *p)).collect();

    let mut backend = CraneliftBackend::with_symbols(&symbols_ref)?;

    for module in &mir_modules {
        backend.compile_module(module)?;
    }

    println!("Codegen complete!\n");

    println!("=== Expected Output ===");
    println!("0");
    println!("3");
    println!("30");
    println!("2");
    println!("10");
    println!("20");
    println!("40");
    println!("3"); // reverse arr2[0]
    println!("2"); // reverse arr2[1]
    println!("1"); // reverse arr2[2]
    println!("3"); // insert length
    println!("1"); // insert arr3[0]
    println!("2"); // insert arr3[1]
    println!("3"); // insert arr3[2]
    println!("\n=== Actual Output ===\n");

    for module in mir_modules.iter().rev() {
        if backend.call_main(module).is_ok() {
            println!("\n=== Test Complete ===");
            return Ok(());
        }
    }

    Err("Failed to execute main".to_string())
}
