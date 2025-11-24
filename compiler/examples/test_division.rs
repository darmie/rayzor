/// Test: Division with proper float handling
///
/// In Haxe, division always returns Float.
/// This test properly handles float division and converts back to int.

use compiler::compilation::{CompilationUnit, CompilationConfig};
use compiler::codegen::CraneliftBackend;
use compiler::ir::hir_to_mir::lower_hir_to_mir;
use compiler::ir::tast_to_hir::lower_tast_to_hir;
use std::cell::RefCell;
use std::rc::Rc;

fn main() -> Result<(), String> {
    println!("\n=== Testing Division ===\n");

    let mut unit = CompilationUnit::new(CompilationConfig::default());

    // Test division with explicit cast: (30 - Std.int(8.0 / 2.0)) = 30 - 4 = 26
    // Using simpler approach: just integer division via cast
    let source = r#"
        package test;

        class DivisionTest {
            public static function main():Int {
                var a = 10;
                var b = 5;
                var c = a + b;           // 15 (Int)
                var d = c * 2;           // 30 (Int)
                var e = 8.0 / 2.0;       // 4.0 (Float division)
                var f:Int = cast(e, Int); // 4 (cast to Int)
                var result = d - f;      // 26 (Int)
                return result;
            }
        }
    "#;

    println!("Test Code:");
    println!("{}", source);
    println!("\n--- Compilation ---\n");

    // Compile
    unit.load_stdlib()?;
    unit.add_file(source, "DivisionTest.hx")?;
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
    println!("\nExpected: 26 = 30 - cast(8.0/2.0, Int) = 30 - 4");

    if result == 26 {
        println!("\n✅ TEST PASSED! Division with casting works!");
        Ok(())
    } else {
        println!("\n❌ TEST FAILED: Expected 26, got {}", result);
        Err(format!("Expected 26, got {}", result))
    }
}
