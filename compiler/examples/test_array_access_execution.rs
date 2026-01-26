//! Test that array access operator overloading executes correctly at runtime

use compiler::compilation::{CompilationUnit, CompilationConfig};
use compiler::codegen::CraneliftBackend;
use compiler::ir::hir_to_mir::lower_hir_to_mir;
use compiler::ir::tast_to_hir::lower_tast_to_hir;
use std::cell::RefCell;
use std::rc::Rc;

fn main() -> Result<(), String> {
    println!("\n=== Testing Array Access Operator Overloading Execution ===\n");

    let source = r#"
        package test;

        abstract Vec2(Array<Int>) {
            @:arrayAccess
            public inline function get(index:Int):Int {
                // Simple array access - just return index value for testing
                return index * 10;
            }

            @:arrayAccess
            public inline function set(index:Int, value:Int):Int {
                // Simple array write - just return the value for testing
                return value + index;
            }
        }

        class Main {
            public static function main():Int {
                var v:Vec2 = null;
                var setResult = v[2] = 5;  // Should call set(2, 5) which returns 7
                var getResult = v[3];  // Should call get(3) which returns 30
                return setResult + getResult;  // Should return 7 + 30 = 37
            }
        }
    "#;

    let mut unit = CompilationUnit::new(CompilationConfig {
        load_stdlib: false,
        ..Default::default()
    });

    unit.add_file(source, "test.hx")?;

    // Lower to TAST
    let typed_files = unit.lower_to_tast().expect("expected to lower");
    println!("✓ TAST generated\n");

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
    println!("✓ HIR and MIR generated\n");

    // Find main function
    let test_module = mir_modules.into_iter()
        .find(|m| m.functions.iter().any(|(_, f)| f.name.contains("main")))
        .ok_or("No module with main function found")?;

    let (main_func_id, _main_func) = test_module.functions.iter()
        .find(|(_, f)| f.name.contains("main"))
        .ok_or("Main function not found")?;

    // Compile with Cranelift
    let mut backend = CraneliftBackend::new()
        .map_err(|e| format!("Failed to create backend: {}", e))?;

    backend.compile_module(&test_module)?;
    println!("✓ Cranelift compilation complete\n");

    // Execute
    let func_ptr = backend.get_function_ptr(*main_func_id)?;
    let main_fn: fn() -> i32 = unsafe { std::mem::transmute(func_ptr) };
    let result = main_fn();

    println!("Result: {}", result);

    if result == 37 {
        println!("\n✅ TEST PASSED: Array access operator overloading works correctly at runtime!");
        println!("   set(2, 5) = 7 ✓");
        println!("   get(3) = 30 ✓");
        println!("   Total: 7 + 30 = 37 ✓\n");
        Ok(())
    } else {
        println!("\n❌ FAILED: Expected 37, got {}\n", result);
        Err(format!("Array access operator overloading returned wrong value: {}", result))
    }
}
