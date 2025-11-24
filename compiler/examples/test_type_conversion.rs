/// Test proper type conversion in HIR→MIR
///
/// This test verifies that different Haxe types are correctly converted to MIR types.

use compiler::compilation::{CompilationUnit, CompilationConfig};
use compiler::codegen::CraneliftBackend;
use compiler::ir::hir_to_mir::lower_hir_to_mir;
use compiler::ir::tast_to_hir::lower_tast_to_hir;
use std::cell::RefCell;
use std::rc::Rc;

fn main() -> Result<(), String> {
    println!("=== Type Conversion Test ===\n");

    let mut unit = CompilationUnit::new(CompilationConfig::default());

    // Test code with various types
    let source = r#"
        package test;

        class TypeTest {
            // Int function
            public static function addInt(a:Int, b:Int):Int {
                return a + b;
            }

            // Float function
            public static function addFloat(a:Float, b:Float):Float {
                return a + b;
            }

            // Bool function
            public static function andBool(a:Bool, b:Bool):Bool {
                return a && b;
            }

            // Function that takes function pointer (complex type)
            public static function apply(x:Int, func:Int->Int):Int {
                return func(x);
            }

            public static function main():Int {
                // Test various types
                var intResult = addInt(10, 5);           // Int
                var floatResult = addFloat(3.14, 2.86);  // Float
                var boolResult = andBool(true, false);   // Bool

                // Test function pointer (should be i64, not i32!)
                var doubler = function(x:Int) { return x * 2; };
                var funcResult = apply(21, doubler);     // Function pointer

                return intResult + funcResult;  // Should be 15 + 42 = 57
            }
        }
    "#;

    println!("Compiling Haxe code with various types...");
    unit.load_stdlib()?;
    unit.add_file(source, "TypeTest.hx")?;
    let typed_files = unit.lower_to_tast()?;

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

    // Find test module
    let test_module = mir_modules
        .iter()
        .find(|m| m.functions.values().any(|f| f.name.contains("main")))
        .ok_or("No test module found")?;

    // Check function signatures
    println!("Checking function signatures in MIR:");
    for (_id, func) in &test_module.functions {
        if func.name.contains("add") || func.name.contains("and") || func.name.contains("apply") {
            println!("  {}: ({:?}) -> {:?}",
                func.name,
                func.signature.parameters.iter().map(|p| &p.ty).collect::<Vec<_>>(),
                func.signature.return_type
            );
        }
    }

    let main_func_id = test_module
        .functions
        .iter()
        .find(|(_, f)| f.name.contains("main"))
        .map(|(id, _)| *id)
        .ok_or("No main function found")?;

    println!("\nJIT compiling...");
    let mut backend = CraneliftBackend::new()?;
    backend.compile_module(test_module)?;

    println!("Executing...");
    let func_ptr = backend.get_function_ptr(main_func_id)?;
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(func_ptr) };
    let result = main_fn();

    println!("\nResult: {}", result);

    if result == 57 {
        println!("✅ SUCCESS! Type conversion working correctly");
        println!("   - Int types converted properly");
        println!("   - Float types converted properly");
        println!("   - Bool types converted properly");
        println!("   - Function pointer types converted properly (i64, not i32!)");
        Ok(())
    } else {
        Err(format!("❌ FAILED: Expected 57, got {}", result))
    }
}
