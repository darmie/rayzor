/// Isolated test for for-in iterator functionality
///
/// This test is separate from the main e2e test suite because for-in loops
/// are not yet fully implemented in MIR lowering.
///
/// Known Issue: The lower_for_in_loop function in hir_to_mir.rs is a stub that:
/// 1. Never calls .iterator() on the collection
/// 2. Never calls .hasNext() in the condition (register has garbage value)
/// 3. Never calls .next() in the body (loop variable never assigned)
///
/// This causes infinite loops because the uninitialized condition register
/// has random garbage values that happen to be truthy.

use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};
use compiler::ir::IrModule;
use std::sync::Arc;

fn main() -> Result<(), String> {
    println!("=== For-In Iterator Test ===\n");

    // Test 1: Basic for-in over Array<Int>
    test_forin_basic()?;

    println!("\n✅ All for-in tests passed!");
    Ok(())
}

fn compile_to_native(modules: &[Arc<IrModule>]) -> Result<CraneliftBackend, String> {
    // Get runtime symbols from the plugin system
    let plugin = rayzor_runtime::plugin_impl::get_plugin();
    let symbols = plugin.runtime_symbols();
    let symbols_ref: Vec<(&str, *const u8)> = symbols.iter().map(|(n, p)| (*n, *p)).collect();

    // Create Cranelift backend with runtime symbols
    let mut backend = CraneliftBackend::with_symbols(&symbols_ref)?;

    // Compile all MIR modules
    for module in modules {
        backend.compile_module(module)?;
    }

    Ok(backend)
}

fn test_forin_basic() -> Result<(), String> {
    println!("TEST: forin_basic");
    println!("Basic for-in iteration over Array<Int>");
    println!("{}", "=".repeat(50));

    let source = r#"
package test;

class Main {
    static function main() {
        var arr = new Array<Int>();
        arr.push(10);
        arr.push(20);
        arr.push(30);

        var sum = 0;
        for (x in arr) {
            sum += x;
        }
        // sum should be 60
    }
}
"#;

    // Create compilation unit
    let mut unit = CompilationUnit::new(CompilationConfig::default());

    // Load stdlib
    if let Err(e) = unit.load_stdlib() {
        return Err(format!("Failed to load stdlib: {}", e));
    }

    // Add test file
    if let Err(e) = unit.add_file(source, "forin_basic.hx") {
        return Err(format!("Failed to add file: {}", e));
    }

    // Compile to TAST
    println!("L1: Compiling to TAST...");
    let _typed_files = match unit.lower_to_tast() {
        Ok(files) => {
            println!("  ✅ TAST lowering succeeded ({} files)", files.len());
            files
        }
        Err(errors) => {
            return Err(format!("TAST lowering failed: {:?}", errors));
        }
    };

    // Get MIR modules
    println!("L2-L3: HIR/MIR lowering...");
    let mir_modules = unit.get_mir_modules();
    if mir_modules.is_empty() {
        return Err("No MIR modules generated".to_string());
    }
    println!("  ✅ MIR lowering succeeded ({} modules)", mir_modules.len());

    // Codegen
    println!("L5: Compiling to native code...");
    let mut backend = compile_to_native(&mir_modules)?;
    println!("  ✅ Codegen succeeded");

    // Execute with timeout
    println!("L6: Executing...");
    println!("  🚀 Executing main()...");
    println!("  ⏱️  (5 second timeout - for-in is expected to hang)");

    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    let (tx, rx) = mpsc::channel();

    // Clone what we need for the thread
    let user_module = mir_modules.last().cloned();

    let handle = thread::spawn(move || {
        if let Some(module) = user_module {
            let _ = backend.call_main(&module);
        }
        let _ = tx.send(());
    });

    // Wait max 5 seconds
    match rx.recv_timeout(Duration::from_secs(5)) {
        Ok(_) => {
            handle.join().ok();
            println!("  ✅ Execution completed!");
            Ok(())
        }
        Err(_) => {
            println!("  ❌ Execution timed out (for-in loop hung as expected)");
            println!();
            println!("  This is the expected behavior because lower_for_in_loop");
            println!("  in hir_to_mir.rs is a stub that never calls:");
            println!("    - .iterator() on the collection");
            println!("    - .hasNext() in the condition check");
            println!("    - .next() to get loop variable values");
            // Note: We can't easily kill the thread, but the process will exit
            Err("For-in loop caused infinite loop - implementation incomplete".to_string())
        }
    }
}
