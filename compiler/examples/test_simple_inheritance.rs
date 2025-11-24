use compiler::compilation::{CompilationUnit, CompilationConfig};
use compiler::codegen::CraneliftBackend;
use compiler::ir::hir_to_mir::lower_hir_to_mir;
use compiler::ir::tast_to_hir::lower_tast_to_hir;
use std::cell::RefCell;
use std::rc::Rc;

fn main() -> Result<(), String> {
    println!("\n=== Testing Simple Class Inheritance (without super) ===\n");

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

            public function useEnergy(amount:Int):Void {
                this.energy = this.energy - amount;
            }
        }

        class Dog extends Animal {
            var barkCount:Int;

            public function new() {
                // Initialize parent field directly (no super yet)
                this.energy = 100;
                this.barkCount = 0;
            }

            public function bark():Void {
                this.barkCount = this.barkCount + 1;
                this.useEnergy(5);  // Call inherited method
            }

            public function getBarkCount():Int {
                return this.barkCount;
            }
        }

        class Main {
            static public function main():Int {
                var dog = new Dog();
                dog.bark();
                dog.bark();
                dog.bark();

                // Should have barked 3 times and used 15 energy
                // energy = 100 - 15 = 85
                // barkCount = 3
                // Return: 85 + 3 = 88
                return dog.getEnergy() + dog.getBarkCount();
            }
        }
    "#;

    println!("Test Code:\n{}\n", source);
    println!("--- Compilation ---\n");

    // Parse and type check
    let mut unit = CompilationUnit::new(CompilationConfig::default());

    unit.load_stdlib()?;
    unit.add_file(source, "SimpleInheritanceTest.hx")?;
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
            mir_modules.push(mir);
        }
    }
    println!("✓ HIR and MIR generated");

    // Find main function
    let test_module = mir_modules.into_iter()
        .find(|m| m.functions.iter().any(|(_, f)| f.name.contains("main")))
        .ok_or("No module with main function found")?;

    let (main_func_id, _main_func) = test_module.functions.iter()
        .find(|(_, f)| f.name.contains("main"))
        .ok_or("Main function not found")?;

    println!("--- JIT Compilation ---\n");

    // Compile with Cranelift
    let mut backend = CraneliftBackend::new()
        .map_err(|e| format!("Failed to create backend: {}", e))?;

    backend.compile_module(&test_module)?;

    println!("✓ Cranelift compilation complete");

    // Execute
    let func_ptr = backend.get_function_ptr(*main_func_id)?;

    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(func_ptr) };
    let result = main_fn();

    println!("✓ Execution successful!");
    println!("\n=== Test Result ===");
    println!("Returned: {}", result);

    if result == 88 {
        println!("✅ TEST PASSED! Inheritance works!");
        println!("   - Dog extends Animal compiles");
        println!("   - Inherited method useEnergy() works");
        println!("   - Inherited method getEnergy() works");
        println!("   - Inherited field energy accessible");
        println!("   - Subclass fields and methods work");
    } else {
        println!("❌ TEST FAILED!");
        println!("   Expected: 88 (energy=85 + barkCount=3)");
        println!("   Got: {}", result);
    }

    Ok(())
}
