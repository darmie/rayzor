//! Simplified test for operator overloading - just test the detection and rewriting

use compiler::compilation::{CompilationUnit, CompilationConfig};
use compiler::ir::tast_to_hir::lower_tast_to_hir;
use std::cell::RefCell;
use std::rc::Rc;

fn main() -> Result<(), String> {
    println!("\n=== Testing Operator Overloading Detection ===\n");

    let source = r#"
        package test;

        abstract Counter(Int) from Int to Int {
            @:op(A + B)
            public inline function add(rhs:Counter):Counter {
                return this + rhs;  // This uses raw Int addition
            }

            public inline function toInt():Int {
                return this;
            }
        }

        class Main {
            public static function main():Int {
                var a:Counter = 5;
                var b:Counter = 10;
                var sum = a + b;  // THIS should trigger operator overloading
                return 15;  // Just return a constant to avoid MIR issues
            }
        }
    "#;

    // Step 1: Create compilation unit
    let mut unit = CompilationUnit::new(CompilationConfig {
        load_stdlib: false,
        ..Default::default()
    });

    unit.add_file(source, "test.hx")?;

    // Step 2: Lower to TAST
    let typed_files = unit.lower_to_tast()?;
    println!("✓ TAST generated ({} files)\n", typed_files.len());

    // Step 3: Lower to HIR (where operator overloading should be detected)
    println!("Lowering to HIR (this is where operator overloading should be detected)...\n");

    let string_interner_rc = Rc::new(RefCell::new(std::mem::replace(
        &mut unit.string_interner,
        compiler::tast::StringInterner::new(),
    )));

    for typed_file in &typed_files {
        let mut interner = string_interner_rc.borrow_mut();
        let _hir = lower_tast_to_hir(typed_file, &unit.symbol_table, &unit.type_table, &mut *interner, None)
            .map_err(|e| format!("HIR error: {:?}", e))?;
    }

    println!("\n✓ HIR generation complete");
    println!("\nCheck the DEBUG output above to see if operator method was found!");

    Ok(())
}
