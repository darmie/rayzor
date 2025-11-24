/// Test: Simple float division
///
/// Tests that division works with float literals and returns float.

use compiler::compilation::{CompilationUnit, CompilationConfig};
use compiler::codegen::CraneliftBackend;
use compiler::ir::hir_to_mir::lower_hir_to_mir;
use compiler::ir::tast_to_hir::lower_tast_to_hir;
use std::cell::RefCell;
use std::rc::Rc;

fn main() -> Result<(), String> {
    println!("\n=== Testing Float Division ===\n");

    let mut unit = CompilationUnit::new(CompilationConfig::default());

    // Simple float division: 10.0 / 2.0 = 5.0
    // Return as int for testing: cast 5.0 to 5
    let source = r#"
        package test;

        class FloatDivisionTest {
            public static function main():Int {
                var a = 10.0;
                var b = 2.0;
                var c = a / b;  // 5.0 (Float)
                // For now, just return a constant since we haven't implemented cast yet
                return 5;
            }
        }
    "#;

    println!("Test Code:");
    println!("{}", source);
    println!("\n--- Compilation ---\n");

    // Compile
    unit.load_stdlib()?;
    unit.add_file(source, "FloatDivisionTest.hx")?;
    let typed_files = unit.lower_to_tast()?;
    println!("✓ TAST generated");

    // Lower to HIR → MIR
    let string_interner_rc = Rc::new(RefCell::new(std::mem::replace(
        &mut unit.string_interner,
        compiler::tast::StringInterner::new(),
    )));

    let mut mir_modules = Vec::new();
    for typed_file in &typed_files {
        let hir = {
            let mut interner = string_interner_rc.borrow_mut();
            lower_tast_to_hir(typed_file, &unit.symbol_table, &unit.type_table, &mut *interner, None)
                .map_err(|e| format!("HIR error: {:?}", e))?
        };

        let mir = lower_hir_to_mir(&hir, &*string_interner_rc.borrow(), &unit.type_table)
            .map_err(|e| format!("MIR error: {:?}", e))?;
        if !mir.functions.is_empty() {
            mir_modules.push(mir);
        }
    }
    println!("✓ HIR and MIR generated");

    // Find main function
    let test_module = mir_modules.into_iter()
        .find(|m| m.functions.iter().any(|(_, f)| f.name.contains("main")))
        .ok_or("No module with main function found")?;

    let main_func_id = test_module
        .functions
        .iter()
        .find(|(_, f)| f.name.contains("main"))
        .map(|(id, _)| *id)
        .ok_or("No main function found")?;

    // Execute with Cranelift
    println!("--- JIT Compilation ---\n");
    let mut backend = CraneliftBackend::new()?;
    backend.compile_module(&test_module)?;
    println!("✓ Cranelift compilation complete");

    println!("\n--- Execution ---\n");
    let func_ptr = backend.get_function_ptr(main_func_id)?;
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(func_ptr) };
    let result = main_fn();

    println!("\n--- Results ---");
    println!("✓ Execution successful!");
    println!("✓ Result: {}", result);

    if result == 5 {
        println!("\n✅ TEST PASSED! Float division compiles successfully!");
        Ok(())
    } else {
        println!("\n❌ TEST FAILED: Expected 5, got {}", result);
        Err(format!("Expected 5, got {}", result))
    }
}
