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
//! Basic tests for sys.thread.Deque and Condition
//!
//! Tests basic functionality without complex threading scenarios

use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};

fn main() -> Result<(), String> {
    println!("=== Testing sys.thread.Deque and Condition ===\n");

    // Test 1: Deque basic operations
    test_deque_basic()?;

    // Test 2: Condition basic operations
    test_condition_basic()?;

    // Test 3: Mutex tryAcquire with trace
    test_mutex_try_acquire()?;

    println!("\n=== All Deque/Condition tests passed! ===");
    Ok(())
}

fn test_deque_basic() -> Result<(), String> {
    println!("TEST: sys.thread.Deque basic operations");

    let code = r#"
import sys.thread.Deque;

class Main {
    static function main() {
        trace("Testing Deque basic operations");

        var deque = new Deque<String>();

        // Add to end
        deque.add("first");
        deque.add("second");
        deque.add("third");

        // Push to front
        deque.push("zero");

        // Pop should get "zero" (front)
        var first = deque.pop(false);
        trace("First pop: " + first);

        // Pop should get "first"
        var second = deque.pop(false);
        trace("Second pop: " + second);

        // Pop non-blocking on non-empty should succeed
        var third = deque.pop(false);
        trace("Third pop: " + third);

        trace("Deque basic operations work!");
    }
}
"#;

    run_code(code, "test_deque_basic")
}

fn test_condition_basic() -> Result<(), String> {
    println!("\nTEST: sys.thread.Condition basic operations");

    let code = r#"
import sys.thread.Condition;

class Main {
    static function main() {
        trace("Testing Condition basic operations");

        var condition = new Condition();

        // Acquire mutex
        condition.acquire();
        trace("Acquired lock");

        // Release mutex
        condition.release();
        trace("Released lock");

        // Try acquire and trace the boolean value directly
        var acquired = condition.tryAcquire();
        trace("tryAcquire result:");
        trace(acquired);
        if (acquired) {
            trace("tryAcquire succeeded");
            condition.release();
        } else {
            trace("tryAcquire failed");
        }

        trace("Condition basic operations work!");
    }
}
"#;

    run_code(code, "test_condition_basic")
}

fn test_mutex_try_acquire() -> Result<(), String> {
    println!("\nTEST: sys.thread.Mutex tryAcquire with trace");

    let code = r#"
import sys.thread.Mutex;

class Main {
    static function main() {
        trace("Testing Mutex tryAcquire with trace");

        var mutex = new Mutex();

        // Try acquire (should succeed on unlocked mutex)
        var acquired = mutex.tryAcquire();
        trace("First tryAcquire result:");
        trace(acquired);

        if (acquired) {
            trace("First tryAcquire succeeded");

            // Try acquire again (should fail - already locked)
            var acquired2 = mutex.tryAcquire();
            trace("Second tryAcquire result (should be false):");
            trace(acquired2);

            mutex.release();
            trace("Mutex released");
        } else {
            trace("First tryAcquire failed (unexpected)");
        }

        trace("Mutex tryAcquire test works!");
    }
}
"#;

    run_code(code, "test_mutex_try_acquire")
}

fn run_code(code: &str, test_name: &str) -> Result<(), String> {
    // Create compilation unit
    let mut unit = CompilationUnit::new(CompilationConfig::default());

    // Load stdlib
    unit.load_stdlib()
        .map_err(|e| format!("Failed to load stdlib: {}", e))?;

    // Add test code
    unit.add_file(code, &format!("{}.hx", test_name))
        .map_err(|e| format!("Failed to add file: {}", e))?;

    // Compile to TAST
    unit.lower_to_tast()
        .map_err(|errors| format!("TAST errors: {:?}", errors))?;

    // Get MIR modules
    let mir_modules = unit.get_mir_modules();
    if mir_modules.is_empty() {
        return Err("No MIR modules generated".to_string());
    }

    // Create Cranelift backend with runtime symbols
    let plugin = rayzor_runtime::plugin_impl::get_plugin();
    let symbols = plugin.runtime_symbols();
    let symbols_ref: Vec<(&str, *const u8)> = symbols.iter().map(|(n, p)| (*n, *p)).collect();

    let mut backend = CraneliftBackend::with_symbols(&symbols_ref)?;

    // Compile modules
    for module in &mir_modules {
        backend.compile_module(module)?;
    }

    // Execute
    println!("  Executing {}...", test_name);
    for module in mir_modules.iter().rev() {
        if backend.call_main(module).is_ok() {
            println!("  {} PASSED", test_name);
            return Ok(());
        }
    }

    Err(format!("{}: No main found", test_name))
}
