//! Direct test of abstract type method inlining without intermediate variables

use compiler::compilation::{CompilationUnit, CompilationConfig};
use compiler::codegen::CraneliftBackend;
use compiler::ir::hir_to_mir::lower_hir_to_mir;
use compiler::ir::tast_to_hir::lower_tast_to_hir;
use std::cell::RefCell;
use std::rc::Rc;

fn main() -> Result<(), String> {
    println!("\n=== Testing Abstract Type Direct Method Calls ===\n");

    test_direct_method_chain()?;

    Ok(())
}

fn test_direct_method_chain() -> Result<(), String> {
    println!("Test: Direct method chain without intermediate variables\n");

    let source = r#"
        package test;

        abstract Counter(Int) from Int to Int {
            public inline function toInt():Int {
                return this;
            }
        }

        class Main {
            public static function main():Int {
                var a:Counter = 15;
                return a.toInt();  // Direct call, should return 15
            }
        }
    "#;

    let result = compile_and_execute(source)?;

    if result == 15 {
        println!("✅ TEST PASSED: Direct method call works!");
        println!("  Result: {}\n", result);
        Ok(())
    } else {
        println!("❌ FAILED: Expected 15, got {}\n", result);
        Err(format!("Test failed: expected 15, got {}", result))
    }
}

fn compile_and_execute(source: &str) -> Result<i32, String> {
    let mut unit = CompilationUnit::new(CompilationConfig {
        load_stdlib: false,
        ..Default::default()
    });

    unit.add_file(source, "test.hx")?;
    let typed_files = unit.lower_to_tast()?;
    println!("  ✓ TAST generated");

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
    println!("  ✓ HIR and MIR generated");

    let test_module = mir_modules.into_iter()
        .find(|m| m.functions.iter().any(|(_, f)| f.name.contains("main")))
        .ok_or("No module with main function found")?;

    let (main_func_id, _) = test_module.functions.iter()
        .find(|(_, f)| f.name.contains("main"))
        .ok_or("Main function not found")?;

    let mut backend = CraneliftBackend::new()
        .map_err(|e| format!("Failed to create backend: {}", e))?;

    backend.compile_module(&test_module)?;
    println!("  ✓ Cranelift compilation complete");

    let func_ptr = backend.get_function_ptr(*main_func_id)?;
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(func_ptr) };
    let result = main_fn();
    println!("  ✓ Execution successful: returned {}", result);

    Ok(result as i32)
}
