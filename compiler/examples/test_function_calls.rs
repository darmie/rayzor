/// Test: Regular function calls
///
/// Tests:
/// - Static function definitions
/// - Function calls with parameters
/// - Return values from functions
/// - Multiple function calls

use compiler::compilation::{CompilationUnit, CompilationConfig};
use compiler::codegen::CraneliftBackend;
use compiler::ir::hir_to_mir::lower_hir_to_mir;
use compiler::ir::tast_to_hir::lower_tast_to_hir;
use std::cell::RefCell;
use std::rc::Rc;

fn main() -> Result<(), String> {
    println!("\n=== Testing Function Calls ===\n");

    let mut unit = CompilationUnit::new(CompilationConfig::default());

    // Test multiple functions calling each other
    let source = r#"
        package test;

        class FunctionTest {
            public static function add(x:Int, y:Int):Int {
                return x + y;
            }

            public static function multiply(x:Int, y:Int):Int {
                return x * y;
            }

            public static function compute():Int {
                var a = add(10, 5);        // 15
                var b = multiply(a, 2);    // 30
                var c = add(b, 12);        // 42
                return c;
            }

            public static function main():Int {
                return compute();
            }
        }
    "#;

    println!("Test Code:");
    println!("{}", source);
    println!("\n--- Compilation ---\n");

    // Compile
    unit.load_stdlib()?;
    unit.add_file(source, "FunctionTest.hx")?;
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

    println!("Functions in module:");
    for (_, func) in &test_module.functions {
        println!("  - {}", func.name);
    }
    println!();

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
    println!("\nExpected: 42");
    println!("  add(10, 5) = 15");
    println!("  multiply(15, 2) = 30");
    println!("  add(30, 12) = 42");

    if result == 42 {
        println!("\n✅ TEST PASSED! Function calls work correctly!");
        Ok(())
    } else {
        println!("\n❌ TEST FAILED: Expected 42, got {}", result);
        Err(format!("Expected 42, got {}", result))
    }
}
