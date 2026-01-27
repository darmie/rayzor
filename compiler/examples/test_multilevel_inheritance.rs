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
use compiler::compilation::{CompilationConfig, CompilationUnit};
use compiler::ir::hir_to_mir::lower_hir_to_mir;
use compiler::ir::tast_to_hir::lower_tast_to_hir;
use std::cell::RefCell;
use std::rc::Rc;

fn main() -> Result<(), String> {
    println!("\n=== Testing Multi-Level Class Inheritance ===\n");

    let source = r#"
        package test;

        class Animal {
            var energy:Int;

            public function new() {
                this.energy = 100;
            }

            public function getEnergy():Int {
                return this.energy;
            }

            public function makeSound():Int {
                return 0;  // Default: no sound
            }
        }

        class Mammal extends Animal {
            var warmBlooded:Int;

            public function new() {
                this.energy = 150;
                this.warmBlooded = 1;
            }

            public function isWarmBlooded():Int {
                return this.warmBlooded;
            }
        }

        class Dog extends Mammal {
            var barkCount:Int;

            public function new() {
                this.energy = 120;
                this.warmBlooded = 1;
                this.barkCount = 0;
            }

            public function bark():Void {
                this.barkCount = this.barkCount + 1;
                this.energy = this.energy - 5;
            }

            public function makeSound():Int {
                return 1;  // Override: dogs make sound
            }
        }

        class Main {
            static public function main():Int {
                var dog = new Dog();
                dog.bark();
                dog.bark();

                // Test multi-level inheritance:
                // - getEnergy() from Animal (2 levels up)
                // - isWarmBlooded() from Mammal (1 level up)
                // - makeSound() overridden in Dog
                // - barkCount from Dog itself

                var energy = dog.getEnergy();          // 120 - 10 = 110
                var warmBlooded = dog.isWarmBlooded(); // 1
                var sound = dog.makeSound();           // 1 (overridden)

                // Return: 110 + 1 + 1 = 112
                return energy + warmBlooded + sound;
            }
        }
    "#;

    println!("Test Code:\n{}\n", source);
    println!("--- Compilation ---\n");

    // Parse and type check
    let mut unit = CompilationUnit::new(CompilationConfig::default());

    unit.load_stdlib()?;
    unit.add_file(source, "MultiLevelInheritanceTest.hx")?;
    let typed_files = unit.lower_to_tast().map_err(|e| format!("{:?}", e))?;
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
    println!("✓ HIR and MIR generated");

    // Find main function
    let test_module = mir_modules
        .into_iter()
        .find(|m| m.functions.iter().any(|(_, f)| f.name.contains("main")))
        .ok_or("No module with main function found")?;

    let (main_func_id, _main_func) = test_module
        .functions
        .iter()
        .find(|(_, f)| f.name.contains("main"))
        .ok_or("Main function not found")?;

    println!("--- JIT Compilation ---\n");

    // Compile with Cranelift
    let mut backend =
        CraneliftBackend::new().map_err(|e| format!("Failed to create backend: {}", e))?;

    backend.compile_module(&test_module)?;

    println!("✓ Cranelift compilation complete");

    // Execute
    let func_ptr = backend.get_function_ptr(*main_func_id)?;

    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(func_ptr) };
    let result = main_fn();

    println!("✓ Execution successful!");
    println!("\n=== Test Result ===");
    println!("Returned: {}", result);

    if result == 112 {
        println!("✅ TEST PASSED! Multi-level inheritance works!");
        println!("   - Dog extends Mammal extends Animal compiles");
        println!("   - Inherited method from grandparent (getEnergy) works");
        println!("   - Inherited method from parent (isWarmBlooded) works");
        println!("   - Method overriding (makeSound) works");
        println!("   - All inherited fields accessible");
    } else {
        println!("❌ TEST FAILED!");
        println!("   Expected: 112 (energy=110 + warmBlooded=1 + sound=1)");
        println!("   Got: {}", result);
    }

    Ok(())
}
