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
use compiler::codegen::CraneliftBackend;
/// Simple test for indirect function calls
///
/// This is a minimal example demonstrating function pointers in action.
use compiler::compilation::{CompilationConfig, CompilationUnit};
use compiler::ir::hir_to_mir::lower_hir_to_mir;
use compiler::ir::tast_to_hir::lower_tast_to_hir;
use std::cell::RefCell;
use std::rc::Rc;

fn main() -> Result<(), String> {
    println!("=== Simple Indirect Call Example ===\n");

    let mut unit = CompilationUnit::new(CompilationConfig::default());

    // Simple Haxe code with a function pointer
    let source = r#"
        package test;

        class Math {
            public static function double(x:Int):Int {
                return x * 2;
            }

            public static function apply(value:Int, func:Int->Int):Int {
                return func(value);
            }

            public static function main():Int {
                var f = double;
                return apply(21, f);  // Should return 42
            }
        }
    "#;

    println!("Compiling Haxe code...");
    unit.load_stdlib()?;
    unit.add_file(source, "Math.hx")?;
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

    // Find test module
    let test_module = mir_modules
        .iter()
        .find(|m| m.functions.values().any(|f| f.name.contains("main")))
        .ok_or("No test module found")?;

    let main_func_id = test_module
        .functions
        .iter()
        .find(|(_, f)| f.name.contains("main"))
        .map(|(id, _)| *id)
        .ok_or("No main function found")?;

    println!("JIT compiling...");
    let mut backend = CraneliftBackend::new()?;
    backend.compile_module(test_module)?;

    println!("Executing...");
    let func_ptr = backend.get_function_ptr(main_func_id)?;
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(func_ptr) };
    let result = main_fn();

    println!("\nResult: {}", result);

    if result == 42 {
        println!("✅ SUCCESS! Indirect call returned correct value (42)");
        Ok(())
    } else {
        Err(format!("❌ FAILED: Expected 42, got {}", result))
    }
}
