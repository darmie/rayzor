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
//! Simplified runtime test for abstract types
//! Just testing if abstract types work at all with Cranelift

use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};
use compiler::ir::hir_to_mir::lower_hir_to_mir;
use compiler::ir::tast_to_hir::lower_tast_to_hir;
use std::cell::RefCell;
use std::rc::Rc;

fn main() -> Result<(), String> {
    println!("\n=== Testing Abstract Types at Runtime (Simplified) ===\n");

    // Test 1: Abstract type with implicit from/to conversion
    test_abstract_conversion()?;

    Ok(())
}

fn test_abstract_conversion() -> Result<(), String> {
    println!("Test 1: Abstract Type with Implicit Conversions\n");

    let source = r#"
        package test;

        abstract Counter(Int) from Int to Int {
        }

        class Main {
            public static function main():Int {
                var x:Counter = 5;  // from Int
                var y:Int = x;      // to Int
                return y;           // Should return 5
            }
        }
    "#;

    let result = compile_and_execute(source)?;

    if result == 5 {
        println!("✓ TEST PASSED: Abstract type with implicit conversions works!\n");
        Ok(())
    } else {
        println!("❌ FAILED: Expected 5, got {}\n", result);
        Err(format!("Test failed: expected 5, got {}", result))
    }
}

fn compile_and_execute(source: &str) -> Result<i32, String> {
    let mut unit = CompilationUnit::new(CompilationConfig {
        load_stdlib: false,
        ..Default::default()
    });

    unit.add_file(source, "test.hx")?;
    let typed_files = unit.lower_to_tast().map_err(|e| format!("{:?}", e))?;
    println!("  ✓ TAST generated");

    let string_interner_rc = Rc::new(RefCell::new(std::mem::replace(
        &mut unit.string_interner,
        compiler::tast::StringInterner::new(),
    )));

    let mut mir_modules = Vec::new();
    for typed_file in &typed_files {
        let hir = {
            let mut interner = string_interner_rc.borrow_mut();
            lower_tast_to_hir(
                typed_file,
                &unit.symbol_table,
                &unit.type_table,
                &mut *interner,
                None,
            )
            .map_err(|e| format!("HIR error: {:?}", e))?
        };

        let mir = lower_hir_to_mir(
            &hir,
            &*string_interner_rc.borrow(),
            &unit.type_table,
            &unit.symbol_table,
        )
        .map_err(|e| format!("MIR error: {:?}", e))?;
        if !mir.functions.is_empty() {
            mir_modules.push(mir);
        }
    }
    println!("  ✓ HIR and MIR generated");

    let test_module = mir_modules
        .into_iter()
        .find(|m| m.functions.iter().any(|(_, f)| f.name.contains("main")))
        .ok_or("No module with main function found")?;

    let (main_func_id, _) = test_module
        .functions
        .iter()
        .find(|(_, f)| f.name.contains("main"))
        .ok_or("Main function not found")?;

    let mut backend =
        CraneliftBackend::new().map_err(|e| format!("Failed to create backend: {}", e))?;

    backend.compile_module(&test_module)?;
    println!("  ✓ Cranelift compilation complete");

    let func_ptr = backend.get_function_ptr(*main_func_id)?;
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(func_ptr) };
    let result = main_fn();
    println!("  ✓ Execution successful: returned {}", result);

    Ok(result as i32)
}
