/// Test: If/Else statements
///
/// Tests:
/// - Simple if statement
/// - If/else branches
/// - Nested conditionals
/// - Comparison operators

use compiler::compilation::{CompilationUnit, CompilationConfig};
use compiler::codegen::CraneliftBackend;
use compiler::ir::hir_to_mir::lower_hir_to_mir;
use compiler::ir::tast_to_hir::lower_tast_to_hir;
use std::cell::RefCell;
use std::rc::Rc;

fn main() -> Result<(), String> {
    println!("\n=== Testing If/Else Statements ===\n");

    let mut unit = CompilationUnit::new(CompilationConfig::default());

    // Test if/else with comparisons
    let source = r#"
        package test;

        class IfElseTest {
            public static function main():Int {
                var x = 10;
                var y = 5;
                var result = 0;

                if (x > y) {
                    result = 100;
                } else {
                    result = 200;
                }

                // Should take the if branch (x > y is true)
                // result = 100

                var bonus = 0;
                if (x == 10) {
                    bonus = 20;
                }

                // Total: 100 + 20 = 120
                return result + bonus;
            }
        }
    "#;

    println!("Test Code:");
    println!("{}", source);
    println!("\n--- Compilation ---\n");

    // Compile
    unit.load_stdlib()?;
    unit.add_file(source, "IfElseTest.hx")?;
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
            // Debug: Print MIR structure
            for (func_id, func) in &mir.functions {
                println!("\nMIR Function {:?}: {}", func_id, func.name);
                println!("  Blocks: {}", func.cfg.blocks.len());
                for (block_id, block) in &func.cfg.blocks {
                    println!("    Block {:?}: {} phis, {} instrs, terminator: {:?}",
                           block_id, block.phi_nodes.len(), block.instructions.len(),
                           std::mem::discriminant(&block.terminator));
                    for phi in &block.phi_nodes {
                        println!("      Phi {:?}: {} incoming", phi.dest, phi.incoming.len());
                        for (from, val) in &phi.incoming {
                            println!("        {:?} -> {:?}", from, val);
                        }
                    }
                }
            }
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
    println!("\nExpected: 120");
    println!("  x=10, y=5");
    println!("  if (x > y) => result = 100");
    println!("  if (x == 10) => bonus = 20");
    println!("  Total: 100 + 20 = 120");

    if result == 120 {
        println!("\n✅ TEST PASSED! If/else statements work correctly!");
        Ok(())
    } else {
        println!("\n❌ TEST FAILED: Expected 120, got {}", result);
        Err(format!("Expected 120, got {}", result))
    }
}
