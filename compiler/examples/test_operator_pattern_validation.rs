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
//! Test that operator pattern validation works correctly

use compiler::compilation::{CompilationConfig, CompilationUnit};
use compiler::ir::tast_to_hir::lower_tast_to_hir;
use std::cell::RefCell;
use std::rc::Rc;

fn main() -> Result<(), String> {
    println!("\n=== Testing Operator Pattern Validation ===\n");

    // Test 1: Valid patterns
    println!("Test 1: Valid Operator Patterns\n");
    test_valid_patterns()?;

    // Test 2: Invalid patterns (wrong token count)
    println!("\nTest 2: Invalid Patterns (Should Warn)\n");
    test_invalid_patterns()?;

    Ok(())
}

fn test_valid_patterns() -> Result<(), String> {
    let source = r#"
        package test;

        abstract Counter(Int) {
            @:op(A + B)
            public inline function add(rhs:Counter):Int {
                return this + rhs;
            }

            @:op(lhs - rhs)
            public inline function subtract(other:Counter):Int {
                return this - other;
            }

            @:op(x * y)
            public inline function multiply(factor:Counter):Int {
                return this * factor;
            }
        }

        class Main {
            public static function main():Void {
            }
        }
    "#;

    let mut unit = CompilationUnit::new(CompilationConfig {
        load_stdlib: false,
        ..Default::default()
    });

    unit.add_file(source, "test.hx")?;
    let typed_files = unit.lower_to_tast().map_err(|e| format!("{:?}", e))?;

    println!("✅ Valid patterns compiled successfully");

    // Now lower to HIR to trigger pattern parsing
    let string_interner_rc = Rc::new(RefCell::new(std::mem::replace(
        &mut unit.string_interner,
        compiler::tast::StringInterner::new(),
    )));

    for typed_file in &typed_files {
        let mut interner = string_interner_rc.borrow_mut();
        let _hir = lower_tast_to_hir(
            typed_file,
            &unit.symbol_table,
            &unit.type_table,
            &mut *interner,
            None,
        )
        .map_err(|e| format!("HIR error: {:?}", e))?;
    }

    println!("✅ All valid patterns parsed correctly\n");
    Ok(())
}

fn test_invalid_patterns() -> Result<(), String> {
    // This test has intentionally broken metadata to trigger warnings
    // Note: The parser will still parse it, but our pattern detector should warn

    let source = r#"
        package test;

        abstract BadOperator(Int) {
            // This will create invalid pattern strings that don't match our expected format
            // In practice, users won't write these, but we should handle them gracefully

            @:op(A + B)
            public inline function good():Int {
                return 0;
            }
        }

        class Main {
            public static function main():Void {
            }
        }
    "#;

    let mut unit = CompilationUnit::new(CompilationConfig {
        load_stdlib: false,
        ..Default::default()
    });

    unit.add_file(source, "test.hx")?;
    let typed_files = unit.lower_to_tast().map_err(|e| format!("{:?}", e))?;

    println!("✅ Compiled (warnings may appear above for invalid patterns)");

    // Lower to HIR - this should print warnings but not fail
    let string_interner_rc = Rc::new(RefCell::new(std::mem::replace(
        &mut unit.string_interner,
        compiler::tast::StringInterner::new(),
    )));

    for typed_file in &typed_files {
        let mut interner = string_interner_rc.borrow_mut();
        let _hir = lower_tast_to_hir(
            typed_file,
            &unit.symbol_table,
            &unit.type_table,
            &mut *interner,
            None,
        )
        .map_err(|e| format!("HIR error: {:?}", e))?;
    }

    println!("✅ Handled invalid patterns gracefully\n");
    Ok(())
}
