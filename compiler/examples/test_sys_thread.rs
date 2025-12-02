//! Test sys.thread.* API (standard Haxe threading)
//!
//! Tests the sys.thread.Thread and sys.thread.Mutex types which are
//! backed by rayzor's concurrency primitives.

use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};

fn main() -> Result<(), String> {
    println!("=== Testing sys.thread API ===\n");

    // Test 1: Basic thread creation using sys.thread.Thread
    test_basic_thread()?;

    // Test 2: Thread with mutex
    test_thread_with_mutex()?;

    println!("\n=== All sys.thread tests passed! ===");
    Ok(())
}

fn test_basic_thread() -> Result<(), String> {
    println!("TEST: sys.thread.Thread.sleep and yield");

    // Test basic Thread static methods (not create, which requires lambda captures)
    let code = r#"
import sys.thread.Thread;

class Main {
    static function main() {
        trace("Testing Thread static methods");

        // Test sleep (0.05 seconds = 50ms)
        Thread.sleep(0.05);
        trace("Sleep complete");

        // Test yield
        Thread.yield();
        trace("Yield complete");

        // Test currentId
        var id = Thread.currentId();
        trace(id > 0 || id == 0);  // Any valid ID

        trace("Thread static methods work!");
    }
}
"#;

    run_code(code, "test_basic_thread")
}

fn test_thread_with_mutex() -> Result<(), String> {
    println!("\nTEST: sys.thread.Mutex");

    let code = r#"
import sys.thread.Thread;
import sys.thread.Mutex;

class Main {
    static function main() {
        trace("Testing sys.thread.Mutex");

        // Create a mutex
        var mutex = new Mutex();

        // Acquire it
        mutex.acquire();
        trace("Lock acquired");

        // Release it
        mutex.release();
        trace("Lock released");

        // Try to acquire (should succeed)
        var acquired = mutex.tryAcquire();
        if (acquired) {
            trace("tryAcquire succeeded");
            mutex.release();
        } else {
            trace("tryAcquire failed (unexpected)");
        }
        trace("Mutex test complete");
    }
}
"#;

    run_code(code, "test_thread_with_mutex")
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
