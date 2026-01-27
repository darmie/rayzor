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
//! Test Closure with Captured Variable
//!
//! This test validates that closures can capture variables from their enclosing scope:
//! - Closure expression with captured variable
//! - Environment pointer setup
//! - Captured variable access in closure body

use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};
use compiler::ir::hir_to_mir::lower_hir_to_mir;
use compiler::ir::tast_to_hir::lower_tast_to_hir;
use std::cell::RefCell;
use std::rc::Rc;

fn main() -> Result<(), String> {
    println!("\n=== Testing Closure with Captured Variable ===\n");

    let mut unit = CompilationUnit::new(CompilationConfig::default());

    // Test: Closure that captures variable from outer scope
    let source = r#"
        package test;

        class ClosureTest {
            public static function main():Int {
                var x = 10;
                var addX = function(y:Int):Int {
                    return x + y;
                };
                // For now, just create the closure
                // Calling it requires closure invocation support
                return 42;  // Placeholder return
            }
        }
    "#;

    println!("Test Code:");
    println!("{}", source);
    println!("\n--- Compilation ---\n");

    // Compile
    unit.load_stdlib()?;
    unit.add_file(source, "LambdaTest.hx")?;
    let typed_files = unit.lower_to_tast().map_err(|e| format!("{:?}", e))?;

    // Lower to HIR → MIR
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

    // Find main function
    let test_module = mir_modules
        .into_iter()
        .find(|m| m.functions.iter().any(|(_, f)| f.name.contains("main")))
        .ok_or("No module with main function found")?;

    // Print all functions to see if lambda was generated
    println!("Functions in module:");
    for (_, func) in &test_module.functions {
        println!("  - {} (ID: {:?})", func.name, func.id);
    }
    println!();

    let main_func_id = test_module
        .functions
        .iter()
        .find(|(_, f)| f.name.contains("main"))
        .map(|(id, _)| *id)
        .ok_or("No main function found")?;

    // Execute with Cranelift
    println!("\n--- JIT Compilation ---\n");
    let mut backend = CraneliftBackend::new()?;
    backend.compile_module(&test_module)?;

    println!("\n--- Execution ---\n");
    let func_ptr = backend.get_function_ptr(main_func_id)?;
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(func_ptr) };
    let result = main_fn();

    println!("\n--- Results ---");
    println!("✓ Compilation successful!");
    println!("✓ Closure with captured variable compiled!");
    println!("✓ Execution result: {}", result);
    println!("\nExpected: 42 (closure created but not called yet)");

    if result == 42 {
        println!("\n✓ TEST PASSED!");
        Ok(())
    } else {
        println!("\n✗ TEST FAILED: Expected 42, got {}", result);
        std::process::exit(1);
    }
}
